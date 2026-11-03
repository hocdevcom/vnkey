"""Native source installer. No shell interpolation or system mutation on --dry-run."""
import argparse
import glob
import hashlib
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import shutil
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
MIN_RUST = (1, 85)
PACKAGES = {
    "apt": {"base": ["build-essential", "cmake", "pkg-config", "ca-certificates", "cargo", "rustc", "zenity"],
            "ibus": ["ibus", "libibus-1.0-dev"], "fcitx5": ["fcitx5", "libfcitx5core-dev", "libfcitx5utils-dev", "fcitx5-frontend-all", "fcitx5-config-qt"]},
    "dnf": {"base": ["gcc", "gcc-c++", "make", "cmake", "pkgconf-pkg-config", "ca-certificates", "cargo", "rust", "zenity"],
            "ibus": ["ibus", "ibus-devel"], "fcitx5": ["fcitx5", "fcitx5-devel", "fcitx5-gtk", "fcitx5-qt", "fcitx5-configtool"]},
    "pacman": {"base": ["base-devel", "cmake", "pkgconf", "ca-certificates", "rust", "zenity"],
               "ibus": ["ibus"], "fcitx5": ["fcitx5", "fcitx5-gtk", "fcitx5-qt", "fcitx5-configtool"]},
    "zypper": {"base": ["gcc", "gcc-c++", "make", "cmake", "pkg-config", "ca-certificates", "cargo", "rust", "zenity"],
               "ibus": ["ibus", "ibus-devel"], "fcitx5": ["fcitx5", "fcitx5-devel"]},
    "apk": {"base": ["build-base", "cmake", "pkgconf", "ca-certificates", "cargo", "rust", "zenity"],
            "ibus": ["ibus", "ibus-dev"], "fcitx5": ["fcitx5", "fcitx5-dev"]},
}

def os_release(path=Path("/etc/os-release")):
    values = {}
    try:
        for line in path.read_text().splitlines():
            if "=" not in line or line.startswith("#"):
                continue
            key, value = line.split("=", 1)
            tokens = shlex.split(value)
            values[key] = tokens[0] if tokens else ""
    except FileNotFoundError:
        pass
    return values

def manager_for(release):
    for name in (release.get("ID", "") + " " + release.get("ID_LIKE", "")).split():
        if name in ("debian", "ubuntu", "linuxmint", "pop"): return "apt"
        if name in ("fedora", "rhel", "centos", "rocky", "almalinux"): return "dnf"
        if name in ("arch", "manjaro", "endeavouros"): return "pacman"
        if name in ("opensuse", "opensuse-tumbleweed", "opensuse-leap", "suse", "sles"): return "zypper"
        if name == "alpine": return "apk"
    return "none"

def running_processes(proc=Path("/proc")):
    names = set()
    try:
        for entry in proc.iterdir():
            if entry.name.isdigit():
                try: names.add((entry / "comm").read_text().strip())
                except (OSError, UnicodeError): pass
    except OSError:
        pass
    return names

def framework_for(env, running, which=shutil.which):
    # Actual session signals win over a desktop-name heuristic.
    for key in ("GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"):
        value = env.get(key, "").lower()
        if value in ("fcitx", "fcitx5", "@im=fcitx", "@im=fcitx5"): return "fcitx5"
        if value in ("ibus", "@im=ibus"): return "ibus"
    if "fcitx5" in running and "ibus-daemon" not in running: return "fcitx5"
    if "ibus-daemon" in running and "fcitx5" not in running: return "ibus"
    desktop = env.get("XDG_CURRENT_DESKTOP", "").lower()
    if "gnome" in desktop or "unity" in desktop: return "ibus"
    if any(name in desktop for name in ("kde", "plasma", "xfce", "hyprland")): return "fcitx5"
    if which("fcitx5"): return "fcitx5"
    return "ibus"

def dependency_commands(manager, framework, package=None):
    if manager == "none": return []
    names = PACKAGES[manager]["base"][:]
    for part in ("ibus", "fcitx5") if framework == "both" else (framework,):
        names += PACKAGES[manager][part]
    if package == "deb" and manager == "apt": names += ["dpkg-dev"]
    if package == "rpm" and manager in ("dnf", "zypper"): names += ["rpm-build"]
    prefix = {"apt": ["apt-get", "install", "-y"], "dnf": ["dnf", "install", "-y"],
              "pacman": ["pacman", "-S", "--needed", "--noconfirm"],
              "zypper": ["zypper", "--non-interactive", "install"], "apk": ["apk", "add"]}[manager]
    return ([["apt-get", "update"]] if manager == "apt" else []) + [prefix + list(dict.fromkeys(names))]

def rust_version(output):
    match = re.search(r"rustc (\d+)\.(\d+)", output)
    return tuple(map(int, match.groups())) if match else (0, 0)

def root_prefix():
    if os.geteuid() == 0: return []
    for command in ("sudo", "doas"):
        if shutil.which(command): return [command]
    raise RuntimeError("System installation needs sudo/doas, or run in a root-managed build environment.")

def run(args, dry=False, elevated=False, env=None):
    command = (["sudo"] if dry else root_prefix()) + args if elevated else args
    print("+ " + shlex.join(command), flush=True)
    if not dry: subprocess.run(command, check=True, cwd=ROOT, env=env)

def ensure_rust(env, allow_install, dry):
    if dry:
        print("Rust: reuse rustc >= 1.85" + ("; otherwise install official rustup minimal (SHA-256 verified)." if allow_install else "; dependency installation disabled."))
        return
    candidate = shutil.which("rustc", path=env.get("PATH"))
    if candidate:
        found = subprocess.run([candidate, "--version"], capture_output=True, text=True)
        if found.returncode == 0 and rust_version(found.stdout) >= MIN_RUST:
            return
    if not allow_install:
        raise RuntimeError("Rust >= 1.85 and cargo are required. Install a current toolchain or rerun without --no-deps.")
    cargo_home = Path(env.get("CARGO_HOME", str(Path.home() / ".cargo"))).expanduser()
    proxy_dir = cargo_home / "bin"
    rustup = shutil.which("rustup", path=env.get("PATH")) or str(proxy_dir / "rustup")
    if not Path(rustup).is_file():
        arch = {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64", "i686": "i686", "armv7l": "armv7"}.get(platform.machine())
        if not arch: raise RuntimeError("Install Rust >= 1.85 for this architecture, then use --no-deps.")
        musl = bool(glob.glob("/lib/ld-musl-*.so.1"))
        abi = ("musl" if musl else "gnu") + ("eabihf" if arch == "armv7" else "")
        target = arch + "-unknown-linux-" + abi
        url = "https://static.rust-lang.org/rustup/dist/" + target + "/rustup-init"
        bootstrap = ROOT / "target/install-tools/rustup-init"
        bootstrap.parent.mkdir(parents=True, exist_ok=True)
        print("Downloading official Rust bootstrap: " + url, flush=True)
        with urllib.request.urlopen(url, timeout=60) as response:
            bootstrap.write_bytes(response.read())
        with urllib.request.urlopen(url + ".sha256", timeout=60) as response:
            expected = response.read().decode("ascii").split()[0]
        if hashlib.sha256(bootstrap.read_bytes()).hexdigest() != expected:
            bootstrap.unlink()
            raise RuntimeError("rustup-init checksum mismatch")
        bootstrap.chmod(0o700)
        install_env = dict(env, RUSTUP_INIT_SKIP_PATH_CHECK="yes")
        run([str(bootstrap), "-y", "--profile", "minimal", "--default-toolchain", "stable", "--no-modify-path"], env=install_env)
    else:
        run([rustup, "toolchain", "install", "stable", "--profile", "minimal"], env=env)
    env["PATH"] = str(proxy_dir) + os.pathsep + env.get("PATH", "")
    env["RUSTUP_TOOLCHAIN"] = "stable"

def main(argv=None):
    parser = argparse.ArgumentParser(description="Install viet-ime for the active Linux input framework.")
    parser.add_argument("--framework", choices=["auto", "ibus", "fcitx5", "both"], default="auto")
    parser.add_argument("--manager", choices=["auto", "none"] + list(PACKAGES), default="auto")
    parser.add_argument("--no-deps", action="store_true", help="Do not install packages or Rust toolchains")
    parser.add_argument("--dry-run", action="store_true", help="Print the plan without changes or downloads")
    parser.add_argument("--build-only", action="store_true")
    parser.add_argument("--package", choices=["deb", "rpm", "tar"], help="Build an artifact instead of installing")
    parser.add_argument("--prefix", type=PurePosixPath, default=PurePosixPath("/usr"))
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    args = parser.parse_args(argv)
    if sys.platform != "linux" and not args.dry_run: parser.error("Run this installer on Linux")
    if args.jobs < 1: parser.error("--jobs must be positive")
    if not args.prefix.is_absolute(): parser.error("--prefix must be absolute")
    release = os_release()
    framework = framework_for(os.environ, running_processes()) if args.framework == "auto" else args.framework
    manager = manager_for(release) if args.manager == "auto" else args.manager
    if not args.no_deps and (Path("/run/ostree-booted").exists() or release.get("ID") in ("nixos", "guix")):
        raise RuntimeError("This system manages /usr declaratively. Use its native packaging workflow; --no-deps --build-only is available in a prepared development environment.")
    if manager == "none" and not args.no_deps:
        raise RuntimeError("Unknown distro. Install C/C++, CMake, pkg-config, Rust >= 1.85 and framework SDKs, then rerun with --no-deps.")
    print("Framework: " + framework + "; package manager: " + manager, flush=True)
    if not args.no_deps:
        for command in dependency_commands(manager, framework, args.package): run(command, dry=args.dry_run, elevated=True)
    env = dict(os.environ)
    ensure_rust(env, not args.no_deps, args.dry_run)
    if glob.glob("/lib/ld-musl-*.so.1"):
        env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-feature=-crt-static"
    env["CARGO_BUILD_JOBS"] = str(args.jobs)
    build = (args.build_dir or ROOT / "target/linux-install" / framework).resolve()
    if not args.dry_run:
        programs=["cmake", "pkg-config", "cargo", "cc", "c++"]
        if args.package: programs.append("cpack")
        if args.package=="deb": programs.append("dpkg-shlibdeps")
        if args.package=="rpm": programs.append("rpmbuild")
        for program in programs:
            if not shutil.which(program, path=env.get("PATH")): raise RuntimeError("Required command missing: " + program)
    run(["cmake", "-S", str(ROOT), "-B", str(build), "-UVIET_FCITX_ADDON_DIR",
         "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_INSTALL_PREFIX=" + str(args.prefix),
         "-DVIET_BUILD_IBUS=" + ("ON" if framework in ("ibus", "both") else "OFF"),
         "-DVIET_BUILD_FCITX5=" + ("ON" if framework in ("fcitx5", "both") else "OFF")], dry=args.dry_run, env=env)
    run(["cmake", "--build", str(build), "--parallel", str(args.jobs)], dry=args.dry_run, env=env)
    if args.package:
        generator = {"deb": "DEB", "rpm": "RPM", "tar": "TGZ"}[args.package]
        run(["cpack", "--config", str(build / "CPackConfig.cmake"), "-G", generator, "-B", str(build / "packages")], dry=args.dry_run, env=env)
        print("Package output: " + str(build / "packages"))
    elif not args.build_only:
        # Never invoke cargo/cmake configuration as root. Only file installation is elevated.
        needs_root=any(args.prefix==PurePosixPath(base) or PurePosixPath(base) in args.prefix.parents for base in ("/usr","/opt"))
        run(["cmake", "--install", str(build)], dry=args.dry_run, elevated=needs_root, env=env)
        if args.dry_run: print("Plan complete; no files changed.")
        else:
            print("Installed. Log out/in, add 'Viet IME' in your input-method settings, then open 'Viet IME Settings'.")
            print("Your existing framework choice, input sources and environment files were preserved.")
    return 0

if __name__ == "__main__":
    try: sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print("viet-ime installer: " + str(error), file=sys.stderr)
        sys.exit(1)
