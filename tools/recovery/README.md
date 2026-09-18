# Recovery tooling — Netac NV7000 / InnoGrit IG5236

Инструменты, которыми был восстановлен Netac NV7000 (IG5236) 2026-09-19.
Работают на Linux (проверено на 7.2.x), обходят мёртвый NVMe-драйвер через
**VFIO cdev + iommufd** и общаются с контроллером напрямую по BAR0.

## Файлы

| Файл | Назначение |
|---|---|
| `netac_pipeline.py` | User-space NVMe admin-драйвер (VFIO cdev + iommufd). Команды: `status`, `ns`, `kick`, `fwtest`, `loader`, `flash`, `multiflash`, `megaflash`..`megaflash4`, `finalize` |
| `nvme_admin_passthru.py` | Passthrough admin-команд через `/dev/nvmeX` ioctl (`NVME_IOCTL_ADMIN_CMD = 0xC0484E41`) + вендорские `0xD1`/`0xD0` |
| `mega4.sh` | Проверенный one-shot сценарий: revive → programmer → rdisk → finalize |
| `revive_rdisk.sh` | revive + multiflash rdisk + finalize |
| `boot_recover.sh` | Ранняя версия (revive + programmer) |

## Ключевые находки (иначе не работает)

1. **AMD-Vi блокирует DMA** устройства без драйвера (blocking domain).
   Лечится привязкой `vfio-pci` + explicit IOAS mapping (iommufd).
   Легаси `/dev/vfio/vfio` container API в ядре 7.x удалён — только cdev.
2. **CQ у ROM-контроллера глубиной 2**: AQA=32 приводит к остановке
   комплишенов после 2 команд. Нужен `NETAC_QSIZE=2`.
3. **FW download в programmer-режиме требует `NSID=0`** (не `0xFFFFFFFF`).
   В ROM-режиме — `0xFFFFFFFF`.
4. **Каждый exit процесса VFIO может уронить устройство в D3cold**
   (config space header 0x7f). Лечится cold power-cycle (3.3В), warm reboot
   недостаточно. Все стадии делать в одном процессе.
5. **Активация runtime FW** (rdisk) происходит только после **полного
   power-cycle** (tёплый reboot не поднимает rdisk — остаётся programmer).
6. `disable_idle_d3=1` для vfio-pci обязателен.
7. Никогда не делать remove/rescan устройства, находящегося в D3cold —
   это роняет хост (проверено дважды).

## Минимальная последовательность

```bash
# 0) подготовка
sudo modprobe vfio-pci vfio_iommu_type1
echo Y | sudo tee /sys/module/vfio_pci/parameters/disable_idle_d3
sudo modprobe -r nvme                      # чтобы не трогал устройство
# при необходимости: remove/rescan (только если устройство в D0!)
echo vfio-pci > /sys/bus/pci/devices/0000:01:00.0/driver_override
echo 0000:01:00.0 > /sys/bus/pci/drivers/vfio-pci/bind
# 1) один процесс на всё
NETAC_QSIZE=2 sudo -E python3 netac_pipeline.py megaflash4
# 2) полный power-cycle (выключить питание!)
# 3) проверка: lsblk → nvme0n1 1.8T, smartctl → 2.00 TB
```

## Требуемые артефакты

Вендорский recovery-пакет Netac (например, `netac_3.S.J.11_external`) с
каталогом `m2_0536/`: `loader.fw`, `programmer+cust+3SJ11+ymtcx2.fw`,
`programmer+cust+3SJ11+rdt+ymtcx2-cmfg.fw`, `rdisk+...-ymtcx2-cmfg.fw`,
`Config_New.ini`. Прошивки в этот репозиторий не входят — получите их
от вендора/из вашего recovery-пакета.

Положите их в `~/netac-recovery/m2_0536/` (путь переопределяется через
`NETAC_FW_DIR`).

## Статус

Диск восстановлен 2026-09-19: PCI `1dbe:5236` (IG5236 RainierPC),
FW `3.S.J.11` (rdisk, `SSD-DRAM-120216-F`), namespace **2.00 TB**,
I/O 3-4.5 GB/s. OEM-брендинг (Model `Netac NVMe SSD 2TB`, SN `RN######`)
не записан — опционально через MPUtility (Windows) / команду OEM info.
