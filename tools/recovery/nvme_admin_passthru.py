#!/usr/bin/env python3
"""NVMe admin passthrough via /dev/nvmeX ioctl (kernel UAPI nvme_passthru_cmd, 72B)."""
import ctypes
import fcntl
import os
import struct
import sys
import time

NVME_IOCTL_ADMIN_CMD = 0xC0484E41
MAGIC_GRIT = 0x54495247
MAGIC_INNO = 0x4F4E4E49
DEV = sys.argv[1] if len(sys.argv) > 1 else "/dev/nvme0"


def admin(fd, opcode, nsid=0xFFFFFFFF, cdw10=0, cdw11=0, cdw12=0, cdw13=0,
          cdw14=0, cdw15=0, addr=0, data_len=0, timeout_ms=60000):
    buf = bytearray(72)
    struct.pack_into("<BBHI", buf, 0, opcode, 0, 1, nsid)
    struct.pack_into("<II", buf, 8, 0, 0)
    struct.pack_into("<QQ", buf, 16, 0, addr)
    struct.pack_into("<II", buf, 32, 0, data_len)
    struct.pack_into("<6I", buf, 40, cdw10, cdw11, cdw12, cdw13, cdw14, cdw15)
    struct.pack_into("<II", buf, 64, timeout_ms, 0)
    fcntl.ioctl(fd, NVME_IOCTL_ADMIN_CMD, buf, True)
    return struct.unpack_from("<I", buf, 68)[0]


def identify(fd, nsid, cns, label):
    data = bytearray(4096)
    cbuf = (ctypes.c_char * 4096).from_buffer(data)
    r = admin(fd, 0x06, nsid=nsid, cdw10=cns, addr=ctypes.addressof(cbuf),
              data_len=4096)
    print(f"[{label}] result=0x{r & 0xFFFFFFFF:08x}")
    return data, r


def main():
    fd = os.open(DEV, os.O_RDWR)
    d, r = identify(fd, 0, 1, "id-ctrl")
    if r == 0:
        print(f"  SN={d[4:24].decode('latin1').strip(chr(0) + ' ')!r}")
        print(f"  MN={d[24:64].decode('latin1').strip(chr(0) + ' ')!r}")
        print(f"  FR={d[64:72].decode('latin1').strip(chr(0) + ' ')!r}")

    d, r = identify(fd, 1, 0, "id-ns")
    if r == 0:
        nsze, ncap, nuse = struct.unpack_from("<QQQ", d, 0)
        print(f"  NSZE={nsze} NCAP={ncap} NUSE={nuse}")

    blocks = 3907029168
    print(f"SetThinCapacity 0xD1 blocks={blocks}")
    r = admin(fd, 0xD1, nsid=1, cdw12=blocks & 0xFFFFFFFF, cdw13=blocks >> 32)
    print(f"  result=0x{r & 0xFFFFFFFF:08x}")
    time.sleep(1)
    print("SmartClean 0xD0")
    r = admin(fd, 0xD0, cdw14=MAGIC_GRIT, cdw15=MAGIC_INNO)
    print(f"  result=0x{r & 0xFFFFFFFF:08x}")
    time.sleep(2)

    d, r = identify(fd, 1, 0, "id-ns-after")
    if r == 0:
        nsze, ncap, nuse = struct.unpack_from("<QQQ", d, 0)
        print(f"  NSZE={nsze} NCAP={ncap} NUSE={nuse}")
    d, r = identify(fd, 0, 1, "id-ctrl-after")
    if r == 0:
        print(f"  SN={d[4:24].decode('latin1').strip(chr(0) + ' ')!r}")
        print(f"  MN={d[24:64].decode('latin1').strip(chr(0) + ' ')!r}")
        print(f"  FR={d[64:72].decode('latin1').strip(chr(0) + ' ')!r}")


if __name__ == "__main__":
    main()
