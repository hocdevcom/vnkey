"""Compile, validate and publish binary packages. Never compile on the end-user host."""
import hashlib
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

EXTENSIONS = {"deb": ".deb", "rpm": ".rpm", "tar": ".tar.gz"}
GENERATORS = {"deb": "DEB", "rpm": "RPM", "tar": "TGZ"}

def execute(command, root, **kwargs):
    print("+ " + " ".join(str(arg) for arg in command), flush=True)
    return subprocess.run([str(arg) for arg in command], cwd=root, check=True, **kwargs)

def require_engine(engine):
    if not shutil.which(engine):
        raise RuntimeError("Binary Linux packaging needs a Linux builder. Install/start Docker Desktop in Linux-container mode (or Podman), then rerun build.ps1. No DEB/RPM has been generated.")
    result = subprocess.run([engine, "info", "--format", "{{.OSType}}"], capture_output=True, text=True)
    if result.returncode or "linux" not in result.stdout.lower():
        raise RuntimeError(engine + " must be running in Linux-container mode. No binary package has been generated.")

def check_package_magic(path, kind):
    with path.open("rb") as file:
        magic=file.read(8)
    expected={"deb":b"!<arch>\n", "rpm":b"\xed\xab\xee\xdb", "tar":b"\x1f\x8b"}[kind]
    if not magic.startswith(expected):
        raise RuntimeError("Invalid binary package header: " + str(path))

def verify_payload(package, kind, framework, root):
    check_package_magic(package, kind)
    unpack = Path(tempfile.mkdtemp(prefix="verify-",dir=root / "target/native-packages"))
    # Keep verification artifacts under target/, including on validation failure.
    if kind=="deb":
        execute(["dpkg-deb","--extract",package,unpack],root)
        dependencies=execute(["dpkg-deb","--field",package,"Depends"],root,capture_output=True,text=True).stdout
        forbidden={"cargo","rustc","rust","cmake","gcc","g++","build-essential"}
        for item in re.split(r"[,|]",dependencies):
            if item.strip().split(" ")[0] in forbidden:
                raise RuntimeError("End-user package incorrectly depends on build tools: " + item)
    elif kind=="rpm":
        with subprocess.Popen(["rpm2cpio",str(package)],stdout=subprocess.PIPE) as source:
            subprocess.run(["cpio","-idm","--no-absolute-filenames"],cwd=unpack,stdin=source.stdout,check=True)
            source.stdout.close()
            if source.wait(): raise RuntimeError("rpm2cpio failed")
    else:
        with tarfile.open(package,"r:gz") as archive:
            # CPack's generated files are trusted build outputs; still bound extraction paths.
            for member in archive.getmembers():
                if member.issym() or member.islnk(): raise RuntimeError("Unexpected symlink in package")
                (unpack / member.name).resolve().relative_to(unpack.resolve())
            archive.extractall(unpack)
    cores=list(unpack.rglob("libviet_core.so"))
    controllers=[path for path in unpack.rglob("viet-config") if path.is_file()]
    if len(cores)!=1 or len(controllers)!=1: raise RuntimeError("Package must contain compiled core and viet-config")
    binaries=cores+controllers
    if framework in ("ibus","both"):
        wrappers=list(unpack.rglob("ibus-viet-plugin"))
        if len(wrappers)!=1: raise RuntimeError("Missing IBus binary")
        binaries+=wrappers
    if framework in ("fcitx5","both"):
        wrappers=list(unpack.rglob("libviet.so"))
        if len(wrappers)!=1: raise RuntimeError("Missing Fcitx5 binary")
        binaries+=wrappers
    for binary in binaries:
        with binary.open("rb") as file:
            if file.read(4)!=b"\x7fELF": raise RuntimeError("Package contains non-ELF application: " + str(binary))
        dependencies=execute(["ldd",binary],root,capture_output=True,text=True).stdout
        if "not found" in dependencies: raise RuntimeError("Unresolved staged dependencies: " + dependencies)
        dynamic=execute(["readelf","-d",binary],root,capture_output=True,text=True).stdout
        for needed in re.findall(r"Shared library: \[([^]]+)\]",dynamic):
            if "/" in needed: raise RuntimeError("Build-machine path leaked into DT_NEEDED: " + needed)
    execute([controllers[0],"--help"],root)

def read_distro():
    if hasattr(platform,"freedesktop_os_release"):
        release=platform.freedesktop_os_release()
        return re.sub(r"[^a-zA-Z0-9.-]","",release.get("ID","linux")+release.get("VERSION_ID",""))
    return "linux"

def native_package(args, root, work):
    if sys.platform!="linux": raise RuntimeError("--builder native requires Linux; use Docker/Podman on Windows")
    commands=["cmake","cpack","cargo","ldd","readelf"]
    commands += ["dpkg-deb","dpkg-shlibdeps"] if args.format=="deb" else (["rpmbuild","rpm2cpio","cpio"] if args.format=="rpm" else [])
    for command in commands:
        if not shutil.which(command): raise RuntimeError("Missing Linux build dependency: " + command)
    build=args.from_build.resolve() if args.from_build else work / (args.format+"-"+args.framework)
    if not args.from_build:
        execute(["cmake","-S",root,"-B",build,"-DCMAKE_BUILD_TYPE=Release","-DCMAKE_INSTALL_PREFIX=/usr",
            "-DVIET_EXPORT_DISK=OFF","-DVIET_WARNINGS_AS_ERRORS=ON",
            "-DVIET_BUILD_IBUS="+("ON" if args.framework in ("ibus","both") else "OFF"),
            "-DVIET_BUILD_FCITX5="+("ON" if args.framework in ("fcitx5","both") else "OFF")],root)
        if not args.skip_tests and (root / "tests/parity.rs").exists():
            execute(["cargo","test","--release","--locked","--features","config-app"],root)
        execute(["cmake","--build",build,"--parallel",str(min(4,os.cpu_count() or 1))],root)
        if not args.skip_tests and (root / "tests/ffi_smoke.py").exists():
            execute([sys.executable,root / "tests/ffi_smoke.py"],root)
    version=re.search(r'^version\s*=\s*"([^"]+)"',(root / "core-rust/Cargo.toml").read_text(),re.M).group(1)
    arch=execute(["dpkg","--print-architecture"],root,capture_output=True,text=True).stdout.strip() if args.format=="deb" else platform.machine()
    label="viet-ime_"+version+"_"+read_distro()+"_"+arch+"_"+args.framework
    output=Path(tempfile.mkdtemp(prefix="cpack-",dir=work))
    execute(["cpack","--config",build / "CPackConfig.cmake","-G",GENERATORS[args.format],
        "-D","CPACK_PACKAGE_FILE_NAME="+label,"-B",output],root)
    packages=[path for path in output.iterdir() if path.is_file() and path.name.endswith(EXTENSIONS[args.format])]
    if len(packages)!=1: raise RuntimeError("Expected exactly one compiled package")
    verify_payload(packages[0],args.format,args.framework,root)
    return packages[0]

def run(args, root, publish):
    work=root / "target/native-packages"
    if work.is_symlink() or (root / "target").is_symlink(): raise RuntimeError("Package workspace must stay inside repo")
    builder=args.builder
    if builder=="auto": builder="native" if sys.platform=="linux" else ("docker" if shutil.which("docker") else "podman")
    if builder!="native": require_engine(builder)
    work.mkdir(parents=True,exist_ok=True)
    staging=Path(tempfile.mkdtemp(prefix="release-",dir=work))
    if builder=="native":
        package=native_package(args,root,work)
        shutil.copy2(package,staging / package.name)
    else:
        if args.from_build: raise RuntimeError("--from-build is only supported with --builder native")
        image_kind="rpm" if args.format=="rpm" else "deb"
        image="viet-ime-builder:"+image_kind+"-"+args.platform.split("/")[-1]
        execute([builder,"build","--platform",args.platform,"--file",root / ("packaging/Dockerfile."+image_kind),"--tag",image,root],root)
        command=[builder,"run","--rm","--platform",args.platform,"--mount","type=bind,source="+str(staging)+",target=/out",
            image,"python3","packaging/build.py","--builder","native","--format",args.format,"--framework",args.framework,"--output","/out"]
        if args.skip_tests: command.append("--skip-tests")
        execute(command,root)
        packages=[path for path in staging.iterdir() if path.name.endswith(EXTENSIONS[args.format])]
        if len(packages)!=1: raise RuntimeError("Container did not produce a binary installer")
        check_package_magic(packages[0],args.format)
    package=next(path for path in staging.iterdir() if path.name.endswith(EXTENSIONS[args.format]))
    instruction=("sudo apt install ./"+package.name) if args.format=="deb" else (("sudo dnf install ./"+package.name) if args.format=="rpm" else "Extract the precompiled payload into its configured /usr prefix; framework runtime packages are required.")
    message="Viet IME - precompiled Linux package\n\n"+instruction+"\n\nNo Rust, compiler or source build is required on the destination machine.\nA matching distro/architecture and framework runtime dependencies are required.\nAfter installation, log out/in and add Viet IME in your input method settings.\nSettings: viet-config --gui\n"
    (staging / "INSTALL.txt").write_bytes(message.encode("utf-8"))
    files=sorted(path for path in staging.iterdir() if path.is_file() and path.name!="SHA256SUMS")
    (staging / "SHA256SUMS").write_bytes("".join(hashlib.sha256(path.read_bytes()).hexdigest()+"  "+path.name+"\n" for path in files).encode("utf-8"))
    if args.output:
        destination=args.output.resolve();destination.mkdir(parents=True,exist_ok=True)
        if any(destination.iterdir()): raise RuntimeError("Package output directory must be empty: " + str(destination))
        for path in staging.iterdir(): shutil.copy2(path,destination / path.name)
    else:
        destination=publish(staging,root,work,"viet-ime-binary-v1")
    print("Binary installer: "+str(destination / package.name))
    return 0
