"""Portable installer planning tests; never invoke a package manager or sudo."""
import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("viet_installer", ROOT / "packaging/install.py")
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)

class InstallerTests(unittest.TestCase):
    def test_framework_uses_session_before_desktop(self):
        self.assertEqual(installer.framework_for({"GTK_IM_MODULE":"fcitx","XDG_CURRENT_DESKTOP":"GNOME"},set()),"fcitx5")
        self.assertEqual(installer.framework_for({"XDG_CURRENT_DESKTOP":"GNOME"},{"fcitx5"}),"fcitx5")
        self.assertEqual(installer.framework_for({"XDG_CURRENT_DESKTOP":"GNOME"},{"fcitx5","ibus-daemon"}),"ibus")
        for desktop, want in [("GNOME","ibus"),("ubuntu:GNOME","ibus"),("KDE","fcitx5"),("XFCE","fcitx5"),("Hyprland","fcitx5")]:
            self.assertEqual(installer.framework_for({"XDG_CURRENT_DESKTOP":desktop},set(),lambda _:None),want)
        self.assertEqual(installer.framework_for({"GTK_IM_MODULE":"notfcitx","XDG_CURRENT_DESKTOP":"GNOME"},set()),"ibus")

    def test_derivatives_and_unknown_distros(self):
        for release, want in [({"ID":"linuxmint","ID_LIKE":"ubuntu debian"},"apt"),({"ID":"fedora"},"dnf"),({"ID":"endeavouros"},"pacman"),({"ID":"opensuse-tumbleweed"},"zypper"),({"ID":"alpine"},"apk"),({"ID":"nixos"},"none")]:
            self.assertEqual(installer.manager_for(release),want)

    def test_installs_only_selected_framework_and_no_partial_arch_upgrade(self):
        for manager in installer.PACKAGES:
            commands=installer.dependency_commands(manager,"ibus")
            self.assertTrue(commands)
            self.assertFalse(any("fcitx5" in part for command in commands for part in command))
            self.assertNotIn("-Sy",[part for command in commands for part in command])
        commands=installer.dependency_commands("apt","both","deb")
        self.assertIn("dpkg-dev",commands[-1]);self.assertIn("libibus-1.0-dev",commands[-1]);self.assertIn("libfcitx5core-dev",commands[-1])

    def test_dry_run_never_executes_or_downloads(self):
        output=io.StringIO()
        with patch.object(installer,"os_release",return_value={"ID":"ubuntu"}),patch.object(installer,"running_processes",return_value=set()),patch.dict(installer.os.environ,{"XDG_CURRENT_DESKTOP":"GNOME","GTK_IM_MODULE":"","QT_IM_MODULE":"","XMODIFIERS":""}),patch.object(installer.subprocess,"run") as execute,patch.object(installer.urllib.request,"urlopen") as download,contextlib.redirect_stdout(output):
            self.assertEqual(installer.main(["--dry-run"]),0)
            execute.assert_not_called();download.assert_not_called()
        text=output.getvalue()
        self.assertIn("Framework: ibus",text);self.assertIn("apt-get install",text)
        self.assertIn("-DVIET_BUILD_FCITX5=OFF",text);self.assertIn("cmake --install",text)

    def test_package_plan_does_not_install_build_output(self):
        output=io.StringIO()
        with patch.object(installer,"os_release",return_value={}),contextlib.redirect_stdout(output):
            installer.main(["--dry-run","--no-deps","--framework","fcitx5","--package","tar"])
        self.assertIn("cpack",output.getvalue());self.assertNotIn("cmake --install",output.getvalue())

    def test_rust_version_handles_release_and_nightly(self):
        self.assertLess(installer.rust_version("rustc 1.75.0"),installer.MIN_RUST)
        self.assertEqual(installer.rust_version("rustc 1.85.1 (abc)"),(1,85))
        self.assertGreater(installer.rust_version("rustc 1.99.0-nightly"),installer.MIN_RUST)

if __name__=="__main__": unittest.main()
