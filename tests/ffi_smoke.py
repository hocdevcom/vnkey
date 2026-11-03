"""Exercise the built shared library and verify exactly five C ABI exports."""
import ctypes as C
import os
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = ROOT / "tests/artifacts"
ARTIFACTS.mkdir(exist_ok=True)
os.environ["XDG_CONFIG_HOME"] = str(ARTIFACTS / "ffi-config")
os.environ["XDG_RUNTIME_DIR"] = str(ARTIFACTS / "ffi-runtime")
LIBRARY = ROOT / "target/release" / ("viet_core.dll" if os.name == "nt" else "libviet_core.so")
library = C.CDLL(str(LIBRARY))
class Result(C.Structure):
    _fields_ = [("action_type", C.c_uint32), ("backspace_count", C.c_uint32),
                ("preedit_str", C.c_void_p), ("commit_str", C.c_void_p)]
library.viet_engine_new.restype = C.c_void_p
library.viet_engine_free.argtypes = [C.c_void_p]
library.viet_engine_free.restype = None
library.viet_engine_set_app_id.argtypes = [C.c_void_p, C.c_char_p]
library.viet_engine_set_app_id.restype = None
library.viet_engine_reset.argtypes = [C.c_void_p]
library.viet_engine_reset.restype = None
library.viet_engine_process_key.argtypes = [C.c_void_p, C.c_uint32, C.c_uint32, C.c_uint32]
library.viet_engine_process_key.restype = Result
def text(pointer):
    return C.string_at(pointer).decode("utf-8") if pointer else ""

first = library.viet_engine_new()
second = library.viet_engine_new()
assert first and second
try:
    library.viet_engine_set_app_id(first, b"code")
    assert library.viet_engine_process_key(first, ord("a"), 0, 0).action_type == 2
    result = library.viet_engine_process_key(first, ord("s"), 0, 0)
    assert (result.action_type, result.backspace_count, text(result.commit_str)) == (4, 1, "á")
    # Results are handle-owned: another handle's call cannot invalidate this string.
    library.viet_engine_process_key(second, ord("o"), 0, 0)
    assert text(result.commit_str) == "á"
    library.viet_engine_reset(first)
    library.viet_engine_set_app_id(first, b"google-chrome")
    library.viet_engine_process_key(first, ord("a"), 0, 1 << 18)
    result = library.viet_engine_process_key(first, ord("s"), 0, 1 << 18)
    assert (result.action_type, result.backspace_count, text(result.commit_str)) == (3, 1, "á")
    library.viet_engine_reset(first)
    library.viet_engine_set_app_id(first, b"unknown")
    result = library.viet_engine_process_key(first, ord("a"), 0, 0)
    assert (result.action_type, text(result.preedit_str)) == (1, "a")
    result = library.viet_engine_process_key(first, 0xFFFFFFFF, 0, 0)
    assert result.action_type == 0 and text(result.commit_str) == "a"
finally:
    library.viet_engine_free(first)
    library.viet_engine_free(second)
library.viet_engine_free(None)
assert library.viet_engine_process_key(None, 97, 0, 0).action_type == 0

expected = {"viet_engine_new", "viet_engine_free", "viet_engine_set_app_id",
            "viet_engine_process_key", "viet_engine_reset"}
if os.name == "nt":
    # PE export directory inspection; no external tools/dependencies needed.
    data = LIBRARY.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    coff = pe + 4
    sections = struct.unpack_from("<H", data, coff + 2)[0]
    optional_size = struct.unpack_from("<H", data, coff + 16)[0]
    optional = coff + 20
    magic = struct.unpack_from("<H", data, optional)[0]
    export_rva = struct.unpack_from("<I", data, optional + (112 if magic == 0x20B else 96))[0]
    section_table = optional + optional_size
    def offset(rva):
        for i in range(sections):
            entry = section_table + 40*i
            virtual_size, va, raw_size, raw = struct.unpack_from("<IIII", data, entry + 8)
            if va <= rva < va + max(virtual_size, raw_size): return raw + rva - va
        raise AssertionError(f"RVA outside sections: {rva}")
    export = offset(export_rva)
    names_count = struct.unpack_from("<I", data, export + 24)[0]
    names = offset(struct.unpack_from("<I", data, export + 32)[0])
    exported = set()
    for i in range(names_count):
        start = offset(struct.unpack_from("<I", data, names + i*4)[0])
        exported.add(data[start:data.index(b"\0", start)].decode("ascii"))
else:
    import subprocess
    output = subprocess.check_output(["nm", "-D", "--defined-only", str(LIBRARY)], text=True)
    exported = {line.split()[-1] for line in output.splitlines() if line.strip()}
assert exported == expected, exported
print("Shared-library actions, string lifetime, null handles and 5 exports: OK")
