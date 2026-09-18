#!/bin/bash
set -x
LOG="$HOME/netac-recovery/mega4.log"
exec > >(tee "$LOG") 2>&1
sudo -v
DEV=0000:01:00.0
PORT=0000:00:01.1

sudo modprobe vfio-pci
sudo modprobe vfio_iommu_type1
echo Y | sudo tee /sys/module/vfio_pci/parameters/disable_idle_d3
echo 0 | sudo tee /sys/bus/pci/devices/$PORT/d3cold_allowed || true
echo on | sudo tee /sys/bus/pci/devices/$PORT/power/control || true
sudo modprobe -r nvme 2>/dev/null || true

lspci -nn -s $DEV || { echo "DEVICE MISSING AT START"; exit 3; }

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

echo vfio-pci | sudo tee /sys/bus/pci/devices/$DEV/driver_override
echo "$DEV" | sudo tee /sys/bus/pci/drivers/vfio-pci/bind || true
sleep 1
ls -l /sys/bus/pci/devices/$DEV/driver

echo "=== MEGAFLASH4 ==="
sudo env NETAC_QSIZE=2 python3 -u "$HOME/netac-recovery/netac_pipeline.py" megaflash4
echo "megaflash4 rc=$?"
echo ALLDONE
