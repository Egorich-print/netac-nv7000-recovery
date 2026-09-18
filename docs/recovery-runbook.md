# Recovery runbook — Netac NV7000 (InnoGrit IG5236), 2026-09-18..19

Успешное оживление полностью «мёртвого» NVMe SSD по прямому PCIe на Linux,
без Windows/MPUtility, через user-space доступ к контроллеру.

## Исходное состояние

- Диск: Netac NV7000 2TB, InnoGrit IG5236 (`1f40:5236`), ранее пытались
  восстановить через JMS583 (мост зависал).
- На Linux-хосте (CachyOS, AMD, Linux 7.2.3) диск виден в lspci, но
  `nvme` probe падает `-ENODEV`, BAR0 читается `0xFFFFFFFF` (контроллер
  в глубоком hang). Данные не представляли ценности.

## Ключевые препятствия и решения

| # | Проблема | Решение |
|---|---|---|
| 1 | Легаси VFIO container API (`/dev/vfio/vfio`, `VFIO_SET_IOMMU`) удалён в ядре 7.x → `EINVAL` | Новый **cdev + iommufd**: `/dev/vfio/devices/vfio0`, `IOMMU_IOAS_ALLOC/MAP`, `VFIO_DEVICE_BIND_IOMMUFD`, `VFIO_DEVICE_ATTACH_IOMMUFD_PT` |
| 2 | AMD-Vi blocking domain: DMA устройства без драйвера уходит в fault (`IO_PAGE_FAULT`) | Явный маппинг буферов в IOAS (VFIO), IOVA→PRP/ASQ/ACQ |
| 3 | BAR0 = `0xFFFFFFFF` после boot-probe | `remove/rescan` (только из D0!) + `setpci COMMAND=0x0006` + запрет D3cold |
| 4 | ROM-контроллер перестаёт слать комплишены после 2 команд | **CQ depth = 2** → `AQA = 1\|1<<16`, `NETAC_QSIZE=2` |
| 5 | `0x11` FW download: `Invalid Field` в programmer-режиме | Использовать **`NSID=0`** (в ROM — `0xFFFFFFFF`) |
| 6 | После commit programmer `probe` не находил устройство | `probe` должен делать полный disable/enable (`CC.EN=0→1`), а не только писать CC |
| 7 | Устройство падает в D3cold после выхода Python-процесса | Все стадии — в **одном процессе**; `disable_idle_d3=1`; `d3cold_allowed=0`; cold power-cycle для подъёма |
| 8 | rdisk (runtime FW) не активируется | Только **полный power-cycle** (3.3В), warm reboot оставляет programmer |

## Протокол (точные команды)

Все вендорские команды — NVMe admin SQE, `cdw14='GRIT' (0x54495247)`,
`cdw15='INNO' (0x4F4E4E49)` (если указано).

1. **Probe/идентификация**: `Identify CNS=1` → ROM: `SN=SSD-5236 / MN=MN-5236 /
   FR=2.0.0.3`; programmer: `SSD-DRAM-120216-V / INNOGRIT / 3.S.J.11`;
   rdisk: `SSD-DRAM-120216-F / INNOGRIT / 3.S.J.11`.
2. **Loader** (20 KB): `0xF7` ToRecovery (cdw12=1) → `0x11` download 4КБ-чанками
   (`NSID=0xFFFFFFFF`, `cdw10=NUMD=(len-1)>>2`, `cdw11=OFST/4`) → `0x10` commit
   (`cdw10=0x19`).
3. **Programmer** (626 688 B): `0x11` чанки → `0x10` commit.
4. **DDR training / NAND**: `0xE4 cdw12=3`; `0xF2 cdw10=0x400` (mpinfo, "INMW…").
5. **Prep**: `0xE4 cdw12=1` (BrandNew), `0xE4 cdw12=5` (RemoveCheckFtlBuild).
6. **Rdisk** (888 832 B CMFG): `0x11` чанки **`NSID=0`** → `0xE5 cdw10=0x400,
   cdw12=1, cdw14=~CRC32(conf)` (CONF-страница) → `sleep 3` → `0x10 cdw10=0x19`.
7. **Finalize**: `0xD0` (SmartClean), `0xE4 cdw12=1` (BrandNew).
8. **Полный power-cycle** → в BIOS/ядре диск: `1dbe:5236`, namespace 2.00 TB.

## Верификация

```
lsblk            → nvme0n1 1.8T
smartctl -a      → Model INNOGRIT, FW 3.S.J.11, Namespace 1 Size 2.00 TB
dd iflag=direct  → ~3–4.6 GB/s (read/write)
```

После активации rdisk `lsblk`/`smartctl` видят полноценные 2 ТБ, I/O работает.
Ядро помечает `1dbe:5236` как `NVME_QUIRK_BOGUS_NID` («Ignoring bogus
Namespace Identifiers») — известный квирк для этого контроллера.

## Состояние диска после recovery

- PCI: `1dbe:5236` (IG5236 RainierPC), драйвер `nvme`, привязывается сам.
- FW: `3.S.J.11` (rdisk, SN `SSD-DRAM-120216-F`), **2.00 TB**, SMART ок.
- OEM-брендинг (Model `Netac NVMe SSD 2TB`, серийник `RN######`) не записан —
  заводские defaults. Опционально: MPUtility (Windows) / OEM info команда.

## Не пригодилось

- JMS583/UASP-путь (Rust `transport`) — не использовался.
- UART/GPIO strap (GPIO11→1V8) — не понадобился.
- Windows MPUtility — не понадобился.

## Воспроизведение

Инструменты: `tools/recovery/` (см. README). Основной сценарий —
`mega4.sh` + `netac_pipeline.py megaflash4`, затем полный power-cycle.
