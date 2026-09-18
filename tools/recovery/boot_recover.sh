#!/bin/bash
set -x
sudo -v
DEV=0000:01:00.0
PORT=0000:00:01.1
FW="$HOME/netac-recovery/m2_0536/programmer+cust+3SJ11+ymtcx2.fw"

echo 0 | sudo tee /sys/bus/pci/devices/$PORT/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$PORT/power/control || true

read_bar() {
  sudo python3 - <<'PY'
import mmap, os, struct
try:
    fd = os.open("/sys/bus/pci/devices/0000:01:00.0/resource0", os.O_RDWR | os.O_SYNC)
    m = mmap.mmap(fd, 0x4000, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE)
    print("BARSTATE CSTS=0x%08x CC=0x%08x VS=0x%08x CAP=0x%016x" % (
        struct.unpack_from("<I", m, 0x1c)[0],
        struct.unpack_from("<I", m, 0x14)[0],
        struct.unpack_from("<I", m, 8)[0],
        struct.unpack_from("<Q", m, 0)[0]))
except Exception as e:
    print("BARSTATE err", e)
PY
}

echo "=== lspci ==="
lspci -nn -s $DEV || true
echo "=== initial bar ==="
read_bar

sudo python3 - <<'PY'
import mmap, os, struct, sys
try:
    fd = os.open("/sys/bus/pci/devices/0000:01:00.0/resource0", os.O_RDWR | os.O_SYNC)
    m = mmap.mmap(fd, 0x4000, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE)
    csts = struct.unpack_from("<I", m, 0x1c)[0]
    sys.exit(0 if csts != 0xFFFFFFFF else 42)
except Exception:
    sys.exit(42)
PY
RC=$?
if [ $RC -ne 0 ]; then
  echo "=== MMIO dead -> remove/rescan ==="
  echo 1 | sudo tee /sys/bus/pci/devices/$DEV/remove
  sleep 3
  echo 1 | sudo tee /sys/bus/pci/rescan
  sleep 6
fi

echo 0 | sudo tee /sys/bus/pci/devices/$DEV/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$DEV/power/control || true
echo "=== bar after ==="
read_bar

echo "=== bind vfio-pci ==="
echo vfio-pci | sudo tee /sys/bus/pci/devices/$DEV/driver_override
echo "$DEV" | sudo tee /sys/bus/pci/drivers/vfio-pci/bind
sleep 2
lspci -nn -s $DEV

echo "=== flash programmer ==="
sudo python3 -u "$HOME/netac-recovery/netac_pipeline.py" flash "$FW"
echo "flash rc=$?"
sudo dmesg | grep -i -E "01:00|vfio" | tail -8
echo ALLDONE
