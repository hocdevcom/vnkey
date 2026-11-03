"""Binary packaging fails closed: no source kit or fake DEB is published on failure."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location("viet_native",ROOT / "packaging/native.py")
native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)

class NativePackageTests(unittest.TestCase):
    def setUp(self):
        artifacts=ROOT / "tests/artifacts";artifacts.mkdir(exist_ok=True)
        self.temp=tempfile.TemporaryDirectory(prefix="native-",dir=artifacts)
        self.root=Path(self.temp.name)
        self.args=SimpleNamespace(builder="docker",format="deb",framework="both",platform="linux/amd64",from_build=None,skip_tests=False,output=None)
    def tearDown(self):
        self.root.resolve().relative_to((ROOT / "tests/artifacts").resolve())
        self.temp.cleanup()
    def test_missing_builder_preserves_existing_disk(self):
        disk=self.root / "disk";disk.mkdir();(disk / "keep.txt").write_text("keep")
        publish=Mock()
        with patch.object(native.shutil,"which",return_value=None):
            with self.assertRaisesRegex(RuntimeError,"Linux builder"):native.run(self.args,self.root,publish)
        publish.assert_not_called();self.assertEqual((disk / "keep.txt").read_text(),"keep")
        self.assertFalse((self.root / "target").exists())
    def test_container_without_artifact_cannot_publish(self):
        publish=Mock()
        with patch.object(native,"require_engine"),patch.object(native,"execute") as run:
            with self.assertRaisesRegex(RuntimeError,"did not produce"):native.run(self.args,self.root,publish)
        publish.assert_not_called()
        build,container=[call.args[0] for call in run.call_args_list]
        self.assertEqual(build[:2],["docker","build"]);self.assertIn("linux/amd64",build)
        self.assertIn("--builder",container);self.assertIn("native",container);self.assertIn("--output",container)
        self.assertIn("--framework",container);self.assertIn("both",container)
    def test_windows_cannot_take_native_path(self):
        self.args.builder="native"
        with patch.object(native.sys,"platform","win32"):
            with self.assertRaisesRegex(RuntimeError,"requires Linux"):
                native.native_package(self.args,self.root,self.root)
    def test_source_or_windows_file_is_rejected_as_binary_package(self):
        invalid=self.root / "installer.deb";invalid.write_bytes(b"MZ-not-a-debian-package")
        with self.assertRaisesRegex(RuntimeError,"Invalid binary package"):native.check_package_magic(invalid,"deb")

if __name__=="__main__":unittest.main()
