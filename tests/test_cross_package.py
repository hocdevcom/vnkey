"""Check the published DEB independently through python-debian and pyelftools."""
import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT / "target/cross-tools"))
try:
    from debian.debfile import DebFile
    from elftools.elf.elffile import ELFFile
except ImportError:
    DebFile=ELFFile=None
spec=importlib.util.spec_from_file_location("viet_cross",ROOT / "packaging/cross.py")
cross=importlib.util.module_from_spec(spec);spec.loader.exec_module(cross)

@unittest.skipUnless(DebFile,"cross-package inspection tools are not installed")
class CrossPackageTests(unittest.TestCase):
    def test_ar_control_data_round_trip_and_file_modes(self):
        artifacts=ROOT / "tests/artifacts";artifacts.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="deb-format-",dir=artifacts) as directory:
            root=Path(directory);root.resolve().relative_to(artifacts.resolve())
            payload=root / "payload";binary=payload / "usr/bin/example";binary.parent.mkdir(parents=True);binary.write_bytes(b"format-test")
            package=root / "format-test.deb"
            cross.write_deb(package,"Package: format-test\nVersion: 1\nArchitecture: amd64\nMaintainer: Test <test@localhost>\nDescription: format fixture\n",payload)
            deb=DebFile(package)
            self.assertEqual(deb.debcontrol()["Package"],"format-test")
            member=deb.data.tgz().getmember("./usr/bin/example")
            self.assertEqual(member.mode,0o755)
            self.assertEqual(deb.data.tgz().extractfile(member).read(),b"format-test")
            self.assertIn(hashlib.md5(b"format-test").hexdigest(),deb.control.get_content("md5sums").decode())
            deb.close()

    def test_published_package_contains_linux_binaries_without_build_dependencies(self):
        packages=list((ROOT / "disk").glob("*.deb"))
        if not packages:self.skipTest("no compiled DEB in disk/")
        self.assertEqual(len(packages),1)
        deb=DebFile(packages[0]);control=deb.debcontrol()
        self.assertEqual(control["Architecture"],"amd64")
        if "deepin25" in packages[0].name and "debian12-deepin25" not in packages[0].name:
            self.assertIn("deepin",control["Depends"])
            self.assertIn("libfcitx5core7 (>= 5.1.12",control["Depends"])
        for name in ("cargo","rustc","cmake","build-essential"):
            self.assertNotIn(name,control["Depends"])
        members=[item for item in deb.data.tgz().getmembers() if item.isfile()]
        self.assertFalse(any(item.name.endswith((".exe",".dll",".rs")) for item in members))
        binaries={}
        for member in members:
            data=deb.data.tgz().extractfile(member).read()
            if data.startswith(b"\x7fELF"):
                elf=ELFFile(io.BytesIO(data));self.assertEqual(elf.header.e_machine,"EM_X86_64")
                binaries[Path(member.name).name]=elf
                for tag in elf.get_section_by_name(".dynamic").iter_tags():
                    if tag.entry.d_tag=="DT_NEEDED":
                        self.assertNotIn("\\",tag.needed);self.assertNotIn("/",tag.needed)
        self.assertEqual(set(binaries),{"libviet_core.so","libviet.so","ibus-viet-plugin","viet-config"})
        core=binaries["libviet_core.so"].get_section_by_name(".dynsym")
        exports={s.name for s in core.iter_symbols() if s.entry.st_shndx!="SHN_UNDEF" and s.entry.st_info.bind=="STB_GLOBAL"}
        self.assertEqual(exports,{"viet_engine_new","viet_engine_free","viet_engine_set_app_id","viet_engine_process_key","viet_engine_reset"})
        cpp=binaries["libviet.so"].get_section_by_name(".dynsym")
        self.assertFalse(any("_ZNSt3__1" in symbol.name for symbol in cpp.iter_symbols()),"Fcitx must use GNU libstdc++ ABI, not Zig libc++ ABI")
        deb.close()

if __name__=="__main__":unittest.main()
