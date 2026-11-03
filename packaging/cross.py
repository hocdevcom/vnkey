"""Cross-compile Debian binary packages using Zig + a verified Debian SDK, no VM."""
import hashlib
import io
import lzma
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import gzip

def execute(command, root, **kwargs):
    print("+ " + " ".join(str(arg) for arg in command),flush=True)
    subprocess.run([str(arg) for arg in command],cwd=root,check=True,**kwargs)

def dependencies(root):
    tools=root / "target/cross-tools";sys.path.insert(0,str(tools))
    if not (tools / "ziglang/zig.exe").exists() and not (tools / "ziglang/zig").exists():
        execute([sys.executable,"-m","pip","install","--disable-pip-version-check","--target",tools,
            "ziglang==0.13.0","pyelftools==0.33","python-debian==1.1.1"],root)
    from debian import deb822,debfile
    from elftools.elf.elffile import ELFFile
    zig=tools / ("ziglang/zig.exe" if os.name=="nt" else "ziglang/zig")
    return zig,deb822,debfile,ELFFile

def download(url, path, expected=None):
    if path.exists() and (expected is None or hashlib.sha256(path.read_bytes()).hexdigest()==expected):return
    path.parent.mkdir(parents=True,exist_ok=True)
    temporary=path.with_suffix(path.suffix+".download")
    print("Download: "+url,flush=True)
    with urllib.request.urlopen(url,timeout=60) as response,temporary.open("wb") as output:
        shutil.copyfileobj(response,output)
    if expected and hashlib.sha256(temporary.read_bytes()).hexdigest()!=expected:
        temporary.unlink();raise RuntimeError("SDK checksum mismatch: "+path.name)
    os.replace(temporary,path)

def sdk(root, arch, framework, deb822, debfile, distro="debian12"):
    base="https://community-packages.deepin.com/deepin/beige/" if distro=="deepin25" else "https://deb.debian.org/debian/"
    suite="crimson" if distro=="deepin25" else "bookworm"
    cache=root / "target/cross-sdk" / ("deepin25-"+arch if distro=="deepin25" else arch)
    cache.mkdir(parents=True,exist_ok=True)
    index=cache / "Packages.xz"
    if not index.exists():
        release=urllib.request.urlopen(base+"dists/"+suite+"/Release",timeout=60).read().decode()
        key="main/binary-"+arch+"/Packages.xz"
        sha_section=release.split("SHA256:\n",1)[1].split("\n\n",1)[0]
        checksum=next(line.split()[0] for line in sha_section.splitlines() if len(line.split())==3 and line.split()[-1]==key)
        download(base+"dists/"+suite+"/"+key,index,checksum)
    print("Reading "+distro+" SDK index",flush=True)
    packages={str(item["Package"]):item for item in deb822.Deb822.iter_paragraphs(io.StringIO(lzma.decompress(index.read_bytes()).decode()))}
    seeds=["libc6-dev","linux-libc-dev","libstdc++-12-dev"]
    if framework in ("ibus","both"):seeds += ["libibus-1.0-dev","libglib2.0-dev"]
    if framework in ("fcitx5","both"):seeds += ["libfcitx5core-dev","libfcitx5utils-dev","libfcitx5config-dev"]
    selected={};pending=seeds[:]
    while pending:
        name=pending.pop()
        if name in selected:continue
        if name not in packages:raise RuntimeError("SDK package missing: "+name)
        item=packages[name];selected[name]=item
        for group in (item.get("Pre-Depends","")+","+item.get("Depends","")).split(","):
            for alternative in group.split("|"):
                match=re.match(r"\s*([a-z0-9][a-z0-9+.-]*)",alternative)
                if match and match[1] in packages:
                    dep=match[1]
                    if dep.startswith("lib") or dep=="linux-libc-dev":pending.append(dep)
                    break
    sysroot=cache / "sysroot";sysroot.mkdir(exist_ok=True)
    links=[];owners={}
    for name,item in sorted(selected.items()):
        filename=str(item["Filename"])
        if not filename.startswith("pool/") or ".." in PurePosixPath(filename).parts:raise RuntimeError("Unsafe SDK filename")
        archive=cache / "debs" / Path(filename).name
        download(base+filename,archive,str(item["SHA256"]))
        package=debfile.DebFile(archive)
        for member in package.data.tgz().getmembers():
            relative=member.name.removeprefix("./")
            if not relative.startswith(("usr/include/","usr/lib/","lib/")):continue
            if relative.startswith("/") or ".." in PurePosixPath(relative).parts:raise RuntimeError("Unsafe SDK entry")
            destination=sysroot / relative;destination.resolve().relative_to(sysroot.resolve())
            if member.isfile():
                destination.parent.mkdir(parents=True,exist_ok=True)
                with package.data.tgz().extractfile(member) as source,destination.open("wb") as output:shutil.copyfileobj(source,output)
                owners[relative]=name
            elif member.issym() or member.islnk():
                target=member.linkname
                logical=posixpath.normpath(target.lstrip("/") if target.startswith("/") else (posixpath.join(posixpath.dirname(relative),target) if member.issym() else target.removeprefix("./")))
                if logical.startswith("../"):raise RuntimeError("SDK link escaped root")
                links.append((relative,logical,name))
        package.close()
    for _ in range(8):
        remaining=[]
        for relative,target,name in links:
            source=sysroot / target;destination=sysroot / relative
            if source.is_file():
                destination.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(source,destination);owners[relative]=name
            else:remaining.append((relative,target,name))
        if len(remaining)==len(links):break
        links=remaining
    return sysroot,selected,owners

def elf_info(path, ELFFile, glibc=(2,36)):
    with path.open("rb") as file:
        elf=ELFFile(file)
        dynamic=elf.get_section_by_name(".dynamic")
        needed=[tag.needed for tag in dynamic.iter_tags() if tag.entry.d_tag=="DT_NEEDED"] if dynamic else []
        symbols=elf.get_section_by_name(".dynsym")
        exports={symbol.name for symbol in symbols.iter_symbols() if symbol.name and symbol.entry.st_shndx!="SHN_UNDEF" and symbol.entry.st_info.bind=="STB_GLOBAL"} if symbols else set()
        version=elf.get_section_by_name(".gnu.version_r")
        if version:
            for _,auxiliary in version.iter_versions():
                for symbol in auxiliary:
                    if symbol.name.startswith("GLIBC_") and re.match(r"GLIBC_\d",symbol.name):
                        if tuple(map(int,symbol.name[6:].split(".")))>glibc:raise RuntimeError("Binary requires glibc newer than target: "+symbol.name)
        return needed,exports,elf.header.e_machine

def archive_tree(root, compressed=True):
    data=io.BytesIO()
    with tarfile.open(fileobj=data,mode="w",format=tarfile.GNU_FORMAT) as tar:
        for path in sorted(root.rglob("*")):
            relative=path.relative_to(root).as_posix()
            info=tarfile.TarInfo("./"+relative);info.mode=0o755 if path.is_dir() or relative.startswith(("usr/bin/","usr/libexec/")) else 0o644
            if path.is_dir():info.type=tarfile.DIRTYPE;tar.addfile(info)
            else:
                content=path.read_bytes();info.size=len(content);tar.addfile(info,io.BytesIO(content))
    return lzma.compress(data.getvalue()) if compressed else gzip.compress(data.getvalue(),mtime=0)

def write_deb(path, control, payload):
    control_dir=payload.parent / ("control-"+payload.name);control_dir.mkdir(exist_ok=True)
    (control_dir / "control").write_bytes(control.encode("utf-8"))
    (control_dir / "md5sums").write_bytes("".join(hashlib.md5(file.read_bytes()).hexdigest()+"  "+file.relative_to(payload).as_posix()+"\n" for file in sorted(payload.rglob("*")) if file.is_file()).encode("utf-8"))
    members=[("debian-binary",b"2.0\n"),("control.tar.gz",archive_tree(control_dir,False)),("data.tar.xz",archive_tree(payload))]
    with path.open("wb") as output:
        output.write(b"!<arch>\n")
        for name,content in members:
            header=f"{name+'/':<16}{0:<12}{0:<6}{0:<6}{'100644':<8}{len(content):<10}`\n".encode("ascii")
            if len(header)!=60:raise RuntimeError("Invalid ar header")
            output.write(header);output.write(content)
            if len(content)%2:output.write(b"\n")

def run(args, root, publish):
    if args.format!="deb":raise RuntimeError("Zig cross-packaging currently supports --format deb; RPM needs a separate RPM builder")
    if sys.version_info<(3,10):raise RuntimeError("Cross-packaging requires Python >= 3.10")
    zig,deb822,debfile,ELFFile=dependencies(root)
    arch="amd64" if args.platform=="linux/amd64" else "arm64"
    machine="x86_64" if arch=="amd64" else "aarch64"
    distro=getattr(args,"distro","debian12")
    glibc=(2,38) if distro=="deepin25" else (2,36)
    multiarch=machine+"-linux-gnu";rust_target=machine+"-unknown-linux-gnu";zig_target=multiarch+"."+".".join(map(str,glibc))
    sysroot,packages,owners=sdk(root,arch,args.framework,deb822,debfile,distro)
    work=root / "target/cross-build" / distro / arch;work.mkdir(parents=True,exist_ok=True)
    linker=work / ("zig-link.cmd" if os.name=="nt" else "zig-link")
    if os.name=="nt":linker.write_text('@echo off\n"'+str(zig)+'" cc -target '+zig_target+' %*\n',encoding="utf-8")
    else:linker.write_text('#!/bin/sh\nexec "'+str(zig)+'" cc -target '+zig_target+' "$@"\n');linker.chmod(0o755)
    execute(["rustup","target","add",rust_target],root)
    env=dict(os.environ)
    env["CARGO_TARGET_"+rust_target.upper().replace("-","_")+"_LINKER"]=str(linker)
    env["CARGO_TARGET_DIR"]=str(root / "target/cross-rust")
    execute(["cargo","build","--release","--locked","--target",rust_target,"--features","config-app"],root,env=env)
    # Zig resolves -l inputs to absolute paths; give the shared core a SONAME so
    # wrappers record a relocatable DT_NEEDED rather than a Windows build path.
    execute(["cargo","rustc","-p","viet-core","--release","--locked","--target",rust_target,"--features","config-app","--lib","--",
        "-C","link-arg=-Wl,-soname,libviet_core.so"],root,env=env)
    rust_output=Path(env["CARGO_TARGET_DIR"]) / rust_target / "release"
    payload=Path(tempfile.mkdtemp(prefix="payload-",dir=work))
    library_dir=payload / "usr/lib" / multiarch / "viet-ime";library_dir.mkdir(parents=True)
    shutil.copy2(rust_output / "libviet_core.so",library_dir / "libviet_core.so")
    binary_dir=payload / "usr/bin";binary_dir.mkdir(parents=True)
    shutil.copy2(rust_output / "viet-config",binary_dir / "viet-config")
    includes=[root / "core-rust/include",sysroot / "usr/include",sysroot / "usr/include" / multiarch]
    library_paths=[sysroot / "usr/lib" / multiarch,sysroot / "lib" / multiarch]
    flags=["-target",zig_target,"-O2","-fno-sanitize=undefined"]
    for directory in includes:flags += ["-isystem",str(directory)]
    for directory in library_paths:flags += ["-L",str(directory)]
    flags += ["-L",str(library_dir)]
    if args.framework in ("ibus","both"):
        destination=payload / "usr/libexec/viet-ime/ibus-viet-plugin";destination.parent.mkdir(parents=True)
        ibus_flags=[]
        for directory in ["usr/include/ibus-1.0","usr/include/glib-2.0","usr/include/gio-unix-2.0","usr/lib/"+multiarch+"/glib-2.0/include"]:ibus_flags += ["-I",str(sysroot / directory)]
        execute([zig,"cc",*flags,*ibus_flags,root / "wrappers/ibus-c/engine.c","-o",destination,
            "-lviet_core","-libus-1.0","-lgio-2.0","-lgobject-2.0","-lglib-2.0","-Wl,-rpath,$ORIGIN/../../lib/"+multiarch+"/viet-ime"],root)
        text=(root / "wrappers/ibus-c/viet.xml.in").read_text().replace("@CMAKE_INSTALL_FULL_LIBEXECDIR@","/usr/libexec").replace("@CMAKE_INSTALL_FULL_BINDIR@","/usr/bin").replace("@PROJECT_VERSION@","0.1.0")
        destination=payload / "usr/share/ibus/component/viet.xml";destination.parent.mkdir(parents=True);destination.write_bytes(text.encode())
    if args.framework in ("fcitx5","both"):
        gcc_runtime=next((directory / "libgcc_s.so.1" for directory in library_paths if (directory / "libgcc_s.so.1").is_file()),None)
        if gcc_runtime is None:raise RuntimeError("SDK missing GCC unwind runtime")
        destination=payload / "usr/lib" / multiarch / "fcitx5/libviet.so";destination.parent.mkdir(parents=True)
        cpp_flags=["-std=c++17","-nostdinc++","-nostdlib++","-D_GLIBCXX_USE_CXX11_ABI=1","-shared","-fPIC","-Wl,-z,defs"]
        for directory in ["usr/include/c++/12","usr/include/"+multiarch+"/c++/12","usr/include/c++/12/backward","usr/include/Fcitx5/Core","usr/include/Fcitx5/Utils","usr/include/Fcitx5/Config"]:cpp_flags += ["-isystem",str(sysroot / directory)]
        execute([zig,"c++",*cpp_flags,*flags,root / "wrappers/fcitx5-cpp/engine.cpp","-o",destination,
            "-L",sysroot / "usr/lib/gcc" / multiarch / "12","-lviet_core","-lFcitx5Core","-lFcitx5Config","-lFcitx5Utils",
            sysroot / "usr/lib" / multiarch / "libstdc++.so.6",gcc_runtime,
            "-Wl,-rpath,$ORIGIN/../viet-ime"],root)
        for source,target in [("wrappers/fcitx5-cpp/viet.conf","usr/share/fcitx5/inputmethod/viet.conf"),("wrappers/fcitx5-cpp/viet-addon.conf","usr/share/fcitx5/addon/viet.conf")]:
            destination=payload / target;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(root / source,destination)
    for source,target in [("packaging/viet-config.desktop","usr/share/applications/viet-config.desktop"),("LICENSE","usr/share/licenses/viet-ime/LICENSE"),("core-rust/config.example.toml","usr/share/viet-ime/config.example.toml")]:
        destination=payload / target;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(root / source,destination)
    runtime=set()
    expected_machine="EM_X86_64" if arch=="amd64" else "EM_AARCH64"
    for binary in [path for path in payload.rglob("*") if path.is_file() and path.read_bytes()[:4]==b"\x7fELF"]:
        needed,exports,elf_machine=elf_info(binary,ELFFile,glibc)
        if elf_machine!=expected_machine:raise RuntimeError("Wrong binary architecture")
        if binary.name=="libviet_core.so" and exports!={"viet_engine_new","viet_engine_free","viet_engine_set_app_id","viet_engine_process_key","viet_engine_reset"}:raise RuntimeError("C ABI export mismatch")
        for dependency in needed:
            if "/" in dependency or "\\" in dependency or ":" in dependency:raise RuntimeError("Build path leaked into ELF")
            if dependency=="libviet_core.so":continue
            owner=next((owners.get(directory+"/"+dependency) for directory in ("usr/lib/"+multiarch,"lib/"+multiarch) if owners.get(directory+"/"+dependency)),None)
            if not owner:raise RuntimeError("Missing SDK runtime library: "+dependency)
            runtime.add(owner)
    requires=[name+" (>= "+str(packages[name]["Version"])+")" for name in sorted(runtime)]
    requires += ["zenity","ibus | fcitx5" if args.framework=="both" else args.framework]
    size=(sum(path.stat().st_size for path in payload.rglob("*") if path.is_file())+1023)//1024
    control="Package: viet-ime\nVersion: 0.1.0\nArchitecture: "+arch+"\nMaintainer: Viet IME contributors <noreply@localhost>\nSection: utils\nPriority: optional\nInstalled-Size: "+str(size)+"\nDepends: "+", ".join(requires)+"\nDescription: Vietnamese input with a shared Rust core\n Precompiled IBus/Fcitx5 adapters and common settings controller.\n"
    output=Path(tempfile.mkdtemp(prefix="release-",dir=work));package=output / ("viet-ime_0.1.0_"+distro+"_"+arch+"_"+args.framework+".deb")
    write_deb(package,control,payload)
    check=debfile.DebFile(package)
    if check.debcontrol()["Architecture"]!=arch:raise RuntimeError("DEB metadata mismatch")
    if not check.data.tgz().getmembers():raise RuntimeError("Empty package")
    check.close()
    (output / "INSTALL.txt").write_bytes(("Viet IME - precompiled "+distro+" / "+arch+"\n\nsudo apt install ./"+package.name+"\n\nNo Rust/compiler is required on the destination machine.\nCross-compiled with Zig; ELF/ABI/dependency metadata checked. Desktop runtime test still required.\n").encode())
    (output / "SHA256SUMS").write_bytes((hashlib.sha256(package.read_bytes()).hexdigest()+"  "+package.name+"\n").encode())
    destination=publish(output,root,work,"viet-ime-binary-v1")
    print("Binary installer: "+str(destination / package.name))
    return 0
