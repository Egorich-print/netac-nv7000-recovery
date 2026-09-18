#!/usr/bin/env python3
"""Write OEM info (Model Number / Serial Number) into the InnoGrit IG5236 4K FW config.

Uses NVMe admin passthrough on /dev/nvmeX:
  GetFWconfig 0xE6 -> patch fields -> SetFWconfig 0xE5 (with ~CRC32) -> controller reset.

Offsets were recovered from the vendor MPUtility (middleware.dll packer):
  cfg+0x410 : Serial Number (20 bytes)
  cfg+0x430 : Model Number  (40 bytes)

Usage:
    sudo python3 oem_brand.py "Netac NV7000 (Phoenix Edition)" "RN-PHOENIX-0001"
    sudo python3 oem_brand.py --dev /dev/nvme0 "Model" "SERIAL"
"""
import ctypes
import fcntl
import os
import struct
import sys
import time
import zlib

NVME_IOCTL_ADMIN_CMD = 0xC0484E41
NVME_IOCTL_RESET = 0x4E44
MAGIC_GRIT = 0x54495247
MAGIC_INNO = 0x4F4E4E49

SN_OFF, SN_LEN = 0x410, 20
MODEL_OFF, MODEL_LEN = 0x430, 40


def admin(fd, opcode, nsid=0xFFFFFFFF, cdw10=0, cdw11=0, cdw12=0, cdw13=0,
          cdw14=0, cdw15=0, addr=0, data_len=0, timeout_ms=60000):
    buf = bytearray(72)
    struct.pack_into("<BBHI", buf, 0, opcode, 0, 1, nsid)
    struct.pack_into("<QQ", buf, 16, 0, addr)
    struct.pack_into("<II", buf, 32, 0, data_len)
    struct.pack_into("<6I", buf, 40, cdw10, cdw11, cdw12, cdw13, cdw14, cdw15)
    struct.pack_into("<II", buf, 64, timeout_ms, 0)
    fcntl.ioctl(fd, NVME_IOCTL_ADMIN_CMD, buf, True)
    return struct.unpack_from("<I", buf, 68)[0]


def get_config(fd):
    data = bytearray(4096)
    cbuf = (ctypes.c_char * 4096).from_buffer(data)
    r = admin(fd, 0xE6, cdw10=0x400, cdw12=1, cdw14=MAGIC_GRIT, cdw15=MAGIC_INNO,
              addr=ctypes.addressof(cbuf), data_len=4096)
    return r, data


def set_config(fd, data):
    cbuf = (ctypes.c_char * 4096).from_buffer(data)
    crc = zlib.crc32(bytes(data)) & 0xFFFFFFFF
    r = admin(fd, 0xE5, cdw10=0x400, cdw12=1, cdw13=0, cdw14=(~crc) & 0xFFFFFFFF,
              cdw15=0, addr=ctypes.addressof(cbuf), data_len=4096)
    return r, crc


def identify(fd):
    data = bytearray(4096)
    cbuf = (ctypes.c_char * 4096).from_buffer(data)
    admin(fd, 0x06, nsid=0, cdw10=1, addr=ctypes.addressof(cbuf), data_len=4096)
    return (data[4:24].decode("latin1").strip("\x00 "),
            data[24:64].decode("latin1").strip("\x00 "),
            data[64:72].decode("latin1").strip("\x00 "))


def patch_str(buf, off, length, value):
    raw = value.encode("latin1")[:length]
    buf[off:off + length] = raw + b"\x00" * (length - len(raw))


def main():
    args = sys.argv[1:]
    dev = "/dev/nvme0"
    if args[:1] == ["--dev"]:
        dev = args[1]
        args = args[2:]
    if len(args) != 2:
        print(__doc__)
        sys.exit(1)
    model, serial = args

    fd = os.open(dev, os.O_RDWR)
    print("before:", identify(fd))
    r, cfg = get_config(fd)
    if r != 0:
        print("GetFWconfig failed:", hex(r))
        sys.exit(2)
    patch_str(cfg, MODEL_OFF, MODEL_LEN, model)
    patch_str(cfg, SN_OFF, SN_LEN, serial)
    r, crc = set_config(fd, cfg)
    print(f"SetFWconfig result={r} crc32=0x{crc:08x}")
    if r != 0:
        sys.exit(3)
    time.sleep(1)
    r, cfg2 = get_config(fd)
    print("readback model:", bytes(cfg2[MODEL_OFF:MODEL_OFF + MODEL_LEN]).split(b"\x00")[0])
    print("readback sn   :", bytes(cfg2[SN_OFF:SN_OFF + SN_LEN]).split(b"\x00")[0])
    print("resetting controller...")
    fcntl.ioctl(fd, NVME_IOCTL_RESET, 0)
    time.sleep(5)
    try:
        fd2 = os.open(dev, os.O_RDWR)
        print("after:", identify(fd2))
    except OSError as e:
        print("reopen failed:", e)


if __name__ == "__main__":
    main()
