"""Relocate WebKit's compiled-in helper paths without changing ELF offsets."""
import pathlib
import sys

library = pathlib.Path(sys.argv[1])
original = sys.argv[2].encode()
if not original.startswith(b"/usr/"):
    raise SystemExit("Unexpected WebKit helper directory")
relative = b"././" + original[4:]
assert len(original) == len(relative)
binary = library.read_bytes()
if original not in binary:
    raise SystemExit("WebKit helper path was not found in the bundled library")
library.write_bytes(binary.replace(original, relative))
