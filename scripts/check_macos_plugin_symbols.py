"""Verify that a macOS Maya bundle exports the entry points Maya resolves.

Run it against a packaged ``umbrella_maya.bundle``:

    python3 scripts/check_macos_plugin_symbols.py \
        dist/modules/*/UmbrellaMayaPlugin/plug-ins/umbrella_maya.bundle

Maya loads a plugin with dlopen() and then resolves its entry points with
dlsym("initializePlugin"). On macOS dlsym looks the name up with the Mach-O
leading underscore, and dyld answers from the export trie, not from the static
symbol table. This script therefore reads the trie instead of trusting ``nm``,
which keeps reporting an entry point that dyld never returns:

- the C++ entry points are emitted through ``__asm__`` labels, so they appear in
  the symbol table under the exact label text;
- if that label does not carry the Mach-O underscore,
  ``-exported_symbol,_initializePlugin`` matches nothing locally and ld silently
  re-exports a same-named symbol from a linked dylib instead. The bundle then
  "loads" and registers no commands.

The script also checks that the packaged Rust runtime is referenced through a
relocatable path, because CMake links the plugin against the absolute path of
the cargo build output.
"""

from __future__ import annotations

import struct
import sys

EXPORT_FLAGS_REEXPORT = 0x08
EXPORT_FLAGS_STUB_AND_RESOLVER = 0x10

LC_LOAD_DYLIB = 0x0C
LC_LOAD_WEAK_DYLIB = 0x18 | 0x80000000
LC_REEXPORT_DYLIB = 0x1F | 0x80000000
LC_LOAD_UPWARD_DYLIB = 0x23 | 0x80000000
LC_LAZY_LOAD_DYLIB = 0x20 | 0x80000000
LC_RPATH = 0x1C | 0x80000000
LC_DYLD_INFO_ONLY = 0x22 | 0x80000000
LC_DYLD_EXPORTS_TRIE = 0x33 | 0x80000000

DEPENDENCY_COMMANDS = (
    LC_LOAD_DYLIB,
    LC_LOAD_WEAK_DYLIB,
    LC_REEXPORT_DYLIB,
    LC_LOAD_UPWARD_DYLIB,
    LC_LAZY_LOAD_DYLIB,
)

ENTRY_POINTS = ("_initializePlugin", "_uninitializePlugin")
RUST_RUNTIME = "libumbrella_maya_plugin.dylib"
RELOCATABLE_PREFIXES = ("@rpath/", "@loader_path/", "@executable_path/")


class MachOError(RuntimeError):
    """Raised when the file is not a Mach-O image this script understands."""


def read_uleb128(data, position):
    result = 0
    shift = 0
    while True:
        byte = data[position]
        position += 1
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, position
        shift += 7


def parse_load_commands(data):
    magic = struct.unpack_from("<I", data, 0)[0]
    if magic == 0xFEEDFACF:
        endian, header_size = "<", 32
    elif magic == 0xFEEDFACE:
        endian, header_size = "<", 28
    else:
        raise MachOError(
            "unsupported Mach-O magic 0x%08x (expected a 32/64-bit little-endian image)" % magic
        )

    ncmds = struct.unpack_from(endian + "I", data, 16)[0]
    commands = {}
    position = header_size
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from(endian + "II", data, position)
        commands.setdefault(cmd, []).append(position)
        position += cmdsize
    return commands, endian


def load_command_string(data, position, endian):
    cmd, cmdsize = struct.unpack_from(endian + "II", data, position)
    name_offset = struct.unpack_from(endian + "I", data, position + 8)[0]
    raw = data[position + name_offset : position + cmdsize]
    return raw.split(b"\0", 1)[0].decode("utf-8", "replace")


def exported_symbols(data, commands, endian):
    """Return {symbol: description} for the dyld export trie."""
    if LC_DYLD_EXPORTS_TRIE in commands:
        position = commands[LC_DYLD_EXPORTS_TRIE][0]
        trie_offset, trie_size = struct.unpack_from(endian + "II", data, position + 8)
    elif LC_DYLD_INFO_ONLY in commands:
        position = commands[LC_DYLD_INFO_ONLY][0]
        fields = struct.unpack_from(endian + "10I", data, position + 8)
        trie_offset, trie_size = fields[8], fields[9]
    else:
        raise MachOError(
            "no export trie: the image has neither LC_DYLD_EXPORTS_TRIE nor LC_DYLD_INFO_ONLY"
        )

    exports = {}

    def walk(node_offset, prefix):
        position = node_offset
        terminal_size, position = read_uleb128(data, position)
        if terminal_size:
            cursor = position
            flags, cursor = read_uleb128(data, cursor)
            if flags & EXPORT_FLAGS_REEXPORT:
                ordinal, cursor = read_uleb128(data, cursor)
                end = data.index(b"\0", cursor)
                imported = data[cursor:end].decode("utf-8", "replace") or prefix
                exports[prefix] = "re-export of %r from dylib ordinal %d" % (imported, ordinal)
            else:
                address, cursor = read_uleb128(data, cursor)
                resolver = ""
                if flags & EXPORT_FLAGS_STUB_AND_RESOLVER:
                    stub, cursor = read_uleb128(data, cursor)
                    resolver = " (resolver 0x%x)" % stub
                kind = flags & 0x03
                suffix = "" if kind == 0 else " kind=%d" % kind
                exports[prefix] = "regular export at 0x%x%s%s" % (address, suffix, resolver)

        children = position + terminal_size
        count = data[children]
        cursor = children + 1
        for _ in range(count):
            end = data.index(b"\0", cursor)
            edge = data[cursor:end].decode("utf-8", "replace")
            child_offset, cursor = read_uleb128(data, end + 1)
            walk(trie_offset + child_offset, prefix + edge)

    walk(trie_offset, "")
    return exports


def check(bundle_path):
    with open(bundle_path, "rb") as handle:
        data = handle.read()

    commands, endian = parse_load_commands(data)
    exports = exported_symbols(data, commands, endian)
    dependencies = [
        load_command_string(data, position, endian)
        for cmd in DEPENDENCY_COMMANDS
        for position in commands.get(cmd, [])
    ]
    rpaths = [load_command_string(data, position, endian) for position in commands.get(LC_RPATH, [])]

    failures = []

    for symbol in ENTRY_POINTS:
        description = exports.get(symbol)
        if description is None:
            failures.append("%s is not exported: dyld cannot resolve it" % symbol)
        elif not description.startswith("regular export"):
            failures.append("%s is not defined by the bundle (%s)" % (symbol, description))

    runtime = [d for d in dependencies if d.endswith(RUST_RUNTIME)]
    if not runtime:
        failures.append("the bundle does not link the packaged Rust runtime %s" % RUST_RUNTIME)
    for dependency in runtime:
        if not dependency.startswith(RELOCATABLE_PREFIXES):
            failures.append(
                "the Rust runtime is referenced by the absolute build path %s; dyld cannot "
                "resolve it outside the build machine" % dependency
            )

    if "@loader_path" not in rpaths:
        failures.append("the bundle has no @loader_path rpath (found %s)" % (", ".join(rpaths) or "none"))

    print(
        "entry points: %s"
        % ", ".join("%s (%s)" % (s, exports.get(s, "MISSING")) for s in ENTRY_POINTS)
    )
    print("runtime dependency: %s" % (", ".join(runtime) or "MISSING"))
    print("rpaths: %s" % (", ".join(rpaths) or "none"))
    return failures


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip())
        return 2

    exit_code = 0
    for bundle_path in argv[1:]:
        print("== %s" % bundle_path)
        try:
            failures = check(bundle_path)
        except (MachOError, OSError, struct.error) as error:
            print("[fail] %s: %s" % (bundle_path, error))
            exit_code = 1
            continue
        for failure in failures:
            print("[fail] %s" % failure)
        if failures:
            exit_code = 1
        else:
            print("[ok] the bundle exports both entry points and references a relocatable runtime")
    return exit_code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
