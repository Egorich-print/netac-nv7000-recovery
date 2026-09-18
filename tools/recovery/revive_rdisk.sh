#!/bin/bash
set -x
sudo -v
DEV=0000:01:00.0
PORT=0000:00:01.1

sudo modprobe vfio-pci
sudo modprobe vfio_iommu_type1
echo Y | sudo tee /sys/module/vfio_pci/parameters/disable_idle_d3
cat /sys/module/vfio_pci/parameters/disable_idle_d3

echo 0 | sudo tee /sys/bus/pci/devices/$PORT/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$PORT/power/control || true

sudo modprobe -r nvme 2>/dev/null || true

if [ -e /sys/bus/pci/devices/$DEV/driver ]; then
  DRV=$(basename "$(readlink /sys/bus/pci/devices/$DEV/driver)" 2>/dev/null)
  echo "$DEV" | sudo tee /sys/bus/pci/drivers/$DRV/unbind || true
fi
sleep 1
echo 0 | sudo tee /sys/bus/pci/devices/$DEV/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$DEV/power/control || true

echo "=== remove/rescan ==="
echo 1 | sudo tee /sys/bus/pci/devices/$DEV/remove
sleep 3
echo 1 | sudo tee /sys/bus/pci/rescan
sleep 6
if [ ! -e /sys/bus/pci/devices/$DEV ]; then
  echo "DEVICE LOST AFTER RESCAN"
  exit 3
fi
echo 0 | sudo tee /sys/bus/pci/devices/$DEV/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$DEV/power/control || true
sudo setpci -s $DEV COMMAND=0x0006
sleep 1
lspci -nn -s $DEV

sudo python3 - <<'PY'
import mmap, os, struct
try:
    fd = os.open("/sys/bus/pci/devices/0000:01:00.0/resource0", os.O_RDWR | os.O_SYNC)
    m = mmap.mmap(fd, 0x4000, mmap.MAP_SHARED, mmap.PROT_READ | mmap.PROT_WRITE)
    print("BARSTATE CSTS=0x%08x VS=0x%08x" % (
        struct.unpack_from("<I", m, 0x1c)[0], struct.unpack_from("<I", m, 8)[0]))
except Exception as e:
    print("BARSTATE err", e)
PY

echo vfio-pci | sudo tee /sys/bus/pci/devices/$DEV/driver_override
echo "$DEV" | sudo tee /sys/bus/pci/drivers/vfio-pci/bind || true
sleep 1
ls -l /sys/bus/pci/devices/$DEV/driver

echo "=== RDISK FLASH ==="
sudo env NETAC_QSIZE=2 python3 -u "$HOME/netac-recovery/netac_pipeline.py" multiflash "$HOME/netac-recovery/m2_0536/rdisk+cust+3SJ11+512b+hcrc+semi+l0ps+iecc+semiswitch+logerr+rel+mstripe+raid+a1perf_raid+ymtcx2-cmfg.fw"
echo rc=$?
sudo dmesg | grep -i -E "01:00|nvme|vfio" | tail -8
echo ALLDONE
