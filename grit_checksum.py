#!/usr/bin/env python3
"""Compute and verify GRIT firmware checksums.

The GRIT format has a 16-byte header:
  bytes 0-3:   magic "GRIT"
  bytes 4-7:   header_size or flags (LE u32)
  bytes 8-11:  num_sections or flags (LE u32)
  bytes 12-15: checksum (LE u32)

The section table starts at byte 16. Each entry is 16 bytes:
  bytes 0-3:   section name (ASCII, 4 chars)
  bytes 4-7:   type/id/load_addr (LE u32)
  bytes 8-11:  file offset (LE u32)
  bytes 12-15: size or load_addr (LE u32)

The checksum is CRC32(IEEE) of all section table entries.
"""

import struct
import zlib
import sys
import os


def parse_grit(data):
    if len(data) < 16:
        raise ValueError("File too small for GRIT header")
    if data[:4] != b'GRIT':
        raise ValueError("Not a GRIT file (magic != 'GRIT')")

    field1 = struct.unpack_from('<I', data, 4)[0]
    field2 = struct.unpack_from('<I', data, 8)[0]
    stored_cs = struct.unpack_from('<I', data, 12)[0]

    # Parse section table entries
    pos = 16
    entries = []
    while pos + 16 <= len(data):
        name_bytes = data[pos:pos+4]
        # Stop at zero entry or non-printable name
        if name_bytes == b'\x00\x00\x00\x00':
            break
        if not all(b == 0 or (32 <= b < 127) for b in name_bytes):
            break
        f2 = struct.unpack_from('<I', data, pos+4)[0]
        off = struct.unpack_from('<I', data, pos+8)[0]
        f4 = struct.unpack_from('<I', data, pos+12)[0]
        entries.append({
            'name': name_bytes.decode('ascii', errors='replace'),
            'field2': f2,
            'offset': off,
            'field4': f4,
        })
        pos += 16

    if not entries:
        raise ValueError("No section table entries found")

    return field1, field2, stored_cs, entries


def grit_checksum(data):
    """Compute the GRIT checksum (CRC32 of section table entries)."""
    _, _, _, entries = parse_grit(data)

    # CRC32 of all section entry bytes
    num_entries = len(entries)
    entry_bytes = data[16:16 + num_entries * 16]
    return zlib.crc32(entry_bytes) & 0xFFFFFFFF


def checksum_verify(data):
    """Verify the stored checksum matches the computed one."""
    stored_cs = struct.unpack_from('<I', data, 12)[0]
    computed_cs = grit_checksum(data)
    return computed_cs == stored_cs, stored_cs, computed_cs


def analyze_firmware(filepath):
    """Full analysis of a GRIT firmware file."""
    with open(filepath, 'rb') as f:
        data = f.read()

    basename = os.path.basename(filepath)
    field1, field2, stored_cs, entries = parse_grit(data)
    computed_cs = grit_checksum(data)
    matches = computed_cs == stored_cs

    print(f"File: {basename}")
    print(f"  Size: {len(data)} bytes")
    print(f"  Header:")
    print(f"    Magic:          GRIT")
    print(f"    Field 1:        0x{field1:08X} ({field1})")
    print(f"    Field 2:        0x{field2:08X} ({field2})")
    print(f"    Stored CS:      0x{stored_cs:08X}")
    print(f"  Section table ({len(entries)} entries):")
    for i, e in enumerate(entries):
        print(f"    [{i:2d}] {e['name']:4s}  "
              f"field2=0x{e['field2']:08X}  "
              f"offset=0x{e['offset']:08X} ({e['offset']})  "
              f"field4=0x{e['field4']:08X}")
    cs_range = f"bytes[16:{16 + len(entries) * 16})"
    print(f"  Checksum ({cs_range}):")
    print(f"    Stored:   0x{stored_cs:08X}")
    print(f"    Computed: 0x{computed_cs:08X}")
    print(f"    Match:    {'✓ YES' if matches else '✗ NO'}")
    print()
    return matches


def main():
    if len(sys.argv) < 2:
        print("Usage: grit_checksum.py <firmware.fw> [firmware2.fw ...]")
        sys.exit(1)

    all_ok = True
    for arg in sys.argv[1:]:
        if os.path.isfile(arg):
            if not analyze_firmware(arg):
                all_ok = False
        elif os.path.isdir(arg):
            for root, _, files in os.walk(arg):
                for f in sorted(files):
                    if f.endswith('.fw'):
                        fpath = os.path.join(root, f)
                        with open(fpath, 'rb') as fh:
                            magic = fh.read(4)
                        if magic != b'GRIT':
                            continue
                        if not analyze_firmware(fpath):
                            all_ok = False
        else:
            print(f"Not found: {arg}")
            all_ok = False

    sys.exit(0 if all_ok else 1)


if __name__ == '__main__':
    main()
