"""Relocate WebKit's compiled-in helper paths without changing ELF offsets."""
import pathlib
import sys
import re

library = pathlib.Path(sys.argv[1])
binary = library.read_bytes()

matches = re.findall(rb'/usr/(?:lib/(?:x86_64-linux-gnu/|aarch64-linux-gnu/)?|libexec/)webkit2gtk-4\.[01]', binary)
if not matches:
    print("Warning: WebKit helper path was not found in the bundled library. It might already be relative or not hardcoded.", file=sys.stderr)
    sys.exit(0)

for original in set(matches):
    relative = b"././" + original[4:]
    assert len(original) == len(relative)
    binary = binary.replace(original, relative)

library.write_bytes(binary)
print("Successfully relocated WebKit helper paths:", set(matches))
