"""Verify the actual Linux source-kit contents, portability and safe refresh behavior."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("viet_build", ROOT / "packaging/build.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

class DiskTests(unittest.TestCase):
    def setUp(self):
        artifacts = ROOT / "tests/artifacts"
        artifacts.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="disk-test-", dir=artifacts)
        self.root = Path(self.temporary.name)
        for name in builder.FILES:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        for directory in builder.SOURCE_DIRS:
            shutil.copytree(ROOT / directory, self.root / directory)

    def tearDown(self):
        resolved = self.root.resolve()
        resolved.relative_to((ROOT / "tests/artifacts").resolve())
        self.temporary.cleanup()

    def test_kit_excludes_windows_and_test_artifacts_and_has_valid_archive(self):
        (self.root / "core-rust/src/accidental.exe").write_bytes(b"windows")
        (self.root / "tests").mkdir()
        (self.root / "tests/private.txt").write_text("must not ship")
        (self.root / "install.sh").write_bytes((ROOT / "install.sh").read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        output = builder.export(self.root)
        kit = output / "viet-ime"
        self.assertFalse((kit / "tests").exists())
        self.assertFalse((kit / "target").exists())
        self.assertFalse(any(p.suffix in (".exe", ".dll", ".pdb", ".pyc") for p in kit.rglob("*")))
        self.assertNotIn(b"\r\n", (kit / "install.sh").read_bytes())
        manifest = (kit / "core-rust/Cargo.toml").read_text()
        self.assertNotIn("[[test]]", manifest)
        self.assertIn('required-features = ["config-app"]', manifest)
        checksum, name = (output / "SHA256SUMS").read_text().strip().split("  ")
        self.assertEqual(checksum, hashlib.sha256((output / name).read_bytes()).hexdigest())
        with tarfile.open(output / builder.ARCHIVE, "r:gz") as archive:
            files = {entry.name: entry for entry in archive.getmembers()}
            self.assertEqual(files["viet-ime/install.sh"].mode, 0o755)
            self.assertNotIn("viet-ime/tests", files)
            for name, entry in files.items():
                self.assertFalse(Path(name).is_absolute())
                self.assertNotIn("..", Path(name).parts)
                self.assertFalse(entry.issym())

    def test_refresh_removes_obsolete_files_and_is_reproducible(self):
        old = self.root / "core-rust/src/obsolete.rs"
        old.write_text("// old source\n")
        output = builder.export(self.root)
        self.assertTrue((output / "viet-ime/core-rust/src/obsolete.rs").exists())
        old.unlink()
        builder.export(self.root)
        self.assertFalse((output / "viet-ime/core-rust/src/obsolete.rs").exists())
        before = (output / builder.ARCHIVE).read_bytes()
        builder.export(self.root)
        self.assertEqual(before, (output / builder.ARCHIVE).read_bytes())

    def test_refuses_to_erase_unmanaged_or_edited_output(self):
        output = self.root / "disk"
        output.mkdir()
        (output / "personal.txt").write_text("keep")
        with self.assertRaises(RuntimeError): builder.export(self.root)
        self.assertEqual((output / "personal.txt").read_text(), "keep")
        (output / "personal.txt").unlink()
        output.rmdir()
        builder.export(self.root)
        edited = output / "viet-ime/README.md"
        edited.write_text("user edits")
        with self.assertRaises(RuntimeError): builder.export(self.root)
        self.assertEqual(edited.read_text(), "user edits")

    def test_failed_build_never_exports_and_build_targets_only_library(self):
        with patch.object(builder.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")) as cargo, patch.object(builder, "export") as export:
            with self.assertRaises(subprocess.CalledProcessError): builder.main(["--source"])
            export.assert_not_called()
            self.assertIn("--lib", cargo.call_args.args[0])

if __name__ == "__main__": unittest.main()
