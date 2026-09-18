# Netac NV7000 (InnoGrit IG5236) — research & recovery toolkit

**Русская версия: [README.ru.md](README.ru.md)**

Unofficial reverse engineering and Linux recovery tooling for the **Netac NV7000**
NVMe SSD (2 TB) built on the **InnoGrit IG5236 "RainierPC"** controller.

This repo documents and automates a **complete unbrick** of a drive that was
totally dead: Linux could not probe it (`nvme: error -ENODEV: probe failed`,
BAR0 reads `0xFFFFFFFF`), and the SSD would hang a JMS583 USB–NVMe bridge.
The drive was brought back **without Windows / without the vendor MPUtility** —
using a user-space NVMe admin driver over **VFIO cdev + iommufd**.

After recovery it works as a normal 2 TB SSD (benchmarks below).

## What's inside

```
crates/                    Rust research workspace (GRIT fw parser, NVMe core,
                           InnoGrit protocol, UASP/JMS583 transport)
tools/recovery/            Python recovery & validation tooling
docs/protocol.md           reversed InnoGrit vendor/admin command protocol
docs/protocol-uasp-jms583.md  UASP transport notes (original attempt)
docs/recovery-runbook.md   full recovery story + pitfalls (start here)
docs/xfs-benchmark-2026-09-19.md  post-recovery XFS benchmarks & thermals
```

`tools/recovery/`:

| File | Purpose |
|---|---|
| `netac_pipeline.py` | user-space NVMe admin driver over VFIO cdev + iommufd; full pipeline: ROM → loader → programmer → rdisk → finalize, vendor commands |
| `nvme_admin_passthru.py` | admin passthrough via `/dev/nvmeX` ioctl + InnoGrit vendor commands |
| `oem_brand.py` | write Model/Serial (OEM info) into the 4K FW config |
| `mega4.sh`, `revive_rdisk.sh`, `boot_recover.sh` | end-to-end recovery scenarios |
| `bench.sh`, `thermal_test.sh`, `apst_off.py` | post-recovery validation |

## Tested environment

- Netac NV7000 2 TB, IG5236, vendor FW package `3.S.J.11` (rdisk build).
- Linux 7.2.x (CachyOS), AMD platform, PCIe 4.0 x4.
- Kernel 7.x removed the legacy VFIO container API — this code uses the modern
  **cdev + iommufd** interface (`/dev/vfio/devices/vfioN` + `/dev/iommu`).
- Vendor firmware images are **not included**. Put your recovery package's
  `m2_0536/` folder into `~/netac-recovery/` (override with `NETAC_FW_DIR`).

## Key findings (the non-obvious parts)

1. **VFIO cdev + iommufd.** `/dev/vfio/vfio` legacy container is gone in 7.x.
   Also set `vfio-pci` module param `disable_idle_d3=Y` and
   `d3cold_allowed=0` — otherwise the controller drops into D3cold and becomes
   inaccessible (config space reads return `0xff`).
2. **The ROM's completion queue is only 2 entries deep**, regardless of CAP.
   Use `AQA = 1 | (1<<16)` (`NETAC_QSIZE=2`), or completions stop after two
   commands and every later command "times out".
3. **FW download in programmer mode requires `NSID=0`** (not `0xFFFFFFFF`),
   otherwise `Invalid Field in Command` (SC=0x02).
4. **Runtime FW (rdisk) only activates after a full power cycle** (3.3 V off).
   A warm reboot keeps the programmer running.
5. **Never `remove`/`rescan` a device that is in D3cold** — it disappears from
   the bus and can hang the whole machine. Keep it in D0 at all times.
6. **OEM info (Model/Serial)** lives in the 4K FW config written by
   `SetFWconfig (0xE5)`: Model at `cfg+0x430` (40 B), SN at `cfg+0x410` (20 B).
7. IG5236 is known to hang in deep power states; consider disabling APST
   (`Set Features 0x0C = 0` or `nvme_core.default_ps_max_latency_us=0`).

More details: [`docs/recovery-runbook.md`](docs/recovery-runbook.md).

## Quick start

```bash
# deps: python3, vfio-pci, pciutils; smartmontools/fio for validation
git clone https://github.com/Egorich-print/netac-nv7000-recovery
cd netac-nv7000-recovery

# 1) vendor firmware -> ~/netac-recovery/m2_0536/
#    (loader.fw, programmer*.fw, rdisk*.fw, Config_New.ini)

# 2) keep the device in D0 and out of the nvme driver
sudo modprobe vfio-pci vfio_iommu_type1
echo Y  | sudo tee /sys/module/vfio_pci/parameters/disable_idle_d3
echo 0  | sudo tee /sys/bus/pci/devices/0000:01:00.0/d3cold_allowed
echo on | sudo tee /sys/bus/pci/devices/0000:01:00.0/power/control
sudo modprobe -r nvme

# 3) bind vfio-pci and run the whole pipeline in ONE process
echo vfio-pci   | sudo tee /sys/bus/pci/devices/0000:01:00.0/driver_override
echo 0000:01:00.0 | sudo tee /sys/bus/pci/drivers/vfio-pci/bind
sudo env NETAC_QSIZE=2 python3 tools/recovery/netac_pipeline.py megaflash4

# 4) FULL power cycle (PSU off/on), then verify
lsblk
sudo smartctl -a /dev/nvme0 | head -30
```

If the PCI function is wedged (BAR0 = `0xFFFFFFFF`), see
`tools/recovery/revive_rdisk.sh` — it does the safe remove/rescan + `setpci`
revival sequence. **Do not** touch the parent port's secondary bus reset.

## Post-recovery results (this drive)

| Metric | Value |
|---|---|
| Sequential read / write (1M QD32) | **7.1 / 5.9 GB/s** |
| Sequential read / write (128K QD16) | 7.5 / 6.6 GB/s |
| 4K random read QD32 | **209k IOPS** |
| 4K random read QD1 | 19.1k IOPS @ 49 µs |
| 4K mixed 70/30 QD32 | 142k read / 61k write IOPS |
| Max temperature, 60 s sustained write | **51 °C** (with heatsink) |
| Health after tests | Percentage Used 0 %, 0 media errors |

## Disclaimer

This is **unofficial** tooling. It talks to the controller directly, writes
firmware and 4K config, and can brick the drive permanently — or hang your
machine — if used carelessly. Everything is provided **as is**, without any
warranty. Vendor firmware images are not distributed here. Use at your own risk.

## License

[MIT](LICENSE) © 2026 3g0r1ch (Egorich-print)
