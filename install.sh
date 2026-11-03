#!/bin/sh
# Build as the invoking user; elevate only dependency installation and file install.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v python3 >/dev/null 2>&1; then
    for arg in "$@"; do
        case "$arg" in --dry-run|--help|-h)
            echo 'Install python3 to preview/run the installer. Usage: sh install.sh [--framework auto|ibus|fcitx5|both] [--dry-run] [--no-deps] [--package deb|rpm|tar]'
            exit 0;;
        esac
    done
    root_cmd() {
        if [ "$(id -u)" -eq 0 ]; then "$@"
        elif command -v sudo >/dev/null 2>&1; then sudo "$@"
        elif command -v doas >/dev/null 2>&1; then doas "$@"
        else echo 'Install python3 with your package manager, then rerun.' >&2; exit 1; fi
    }
    if command -v apt-get >/dev/null 2>&1; then
        root_cmd apt-get update
        root_cmd apt-get install -y python3
    elif command -v dnf >/dev/null 2>&1; then root_cmd dnf install -y python3
    elif command -v pacman >/dev/null 2>&1; then root_cmd pacman -S --needed --noconfirm python
    elif command -v zypper >/dev/null 2>&1; then root_cmd zypper --non-interactive install python3
    elif command -v apk >/dev/null 2>&1; then root_cmd apk add python3
    else echo 'Install python3 with your package manager, then rerun.' >&2; exit 1; fi
fi
exec python3 "$root/packaging/install.py" "$@"
