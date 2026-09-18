import ctypes, fcntl, os, struct
NVME=0xC0484E41
def admin(fd, opcode, nsid=0xFFFFFFFF, cdw10=0, cdw11=0, cdw12=0, cdw13=0, cdw14=0, cdw15=0):
    buf=bytearray(72)
    struct.pack_into("<BBHI",buf,0,opcode,0,1,nsid)
    struct.pack_into("<6I",buf,40,cdw10,cdw11,cdw12,cdw13,cdw14,cdw15)
    struct.pack_into("<II",buf,64,60000,0)
    fcntl.ioctl(fd,NVME,buf,True)
    return struct.unpack_from("<I",buf,68)[0]
fd=os.open("/dev/nvme0",os.O_RDWR)
print("APST before:", hex(admin(fd,0x0A,cdw10=0x0C)))
print("set APST=0:", hex(admin(fd,0x09,cdw10=0x0C,cdw11=0)))
print("APST after :", hex(admin(fd,0x0A,cdw10=0x0C)))
