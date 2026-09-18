# Status

> Обновлено 2026-09-19: **диск восстановлен** (unbrick InnoGrit IG5236 через Linux/VFIO).

## Итог recovery (Netac NV7000 / IG5236)

- Диск полностью оживлён на Linux-хосте (CachyOS, Linux 7.2.3) 2026-09-19.
- Устройство: PCI `1dbe:5236` (IG5236 RainierPC); FW `3.S.J.11` (rdisk,
  `SSD-DRAM-120216-F`); namespace **2.00 TB**; I/O 3–4.5 GB/s; SMART читается.
- Прошиты по очереди: ROM → `loader.fw` → `programmer+cust+3SJ11+ymtcx2.fw`
  → `rdisk+...-ymtcx2-cmfg.fw` (+ CONF через `0xE5`) + BrandNew/SmartClean.
- Активация runtime FW — только после **полного power-cycle** (сброс 3.3В).
- OEM-брендинг записан (2026-09-19) через `GetFWconfig(0xE6)` → патч
  `cfg+0x430` (Model, 40 б) / `cfg+0x410` (SN, 20 б) → `SetFWconfig(0xE5)`:
  **Model `Netac NV7000 (Phoenix Edition)`, SN `RN-PHOENIX-0001`**.
  Переживает reboot. Бэкап конфига: `fwconfig_backup.bin`.

## Хранилище (2026-09-19)

- Диск размечен: GPT, `nvme0n1p1` → **XFS** (label `2TB-NVMe`).
- Монтируется автоматически: **`/mnt/2TB-NVMe`** (fstab, `noatime,lazytime,logbsize=256k,nofail`).
- `fstrim.timer` активен. Бенчмарки: 5.9–7.5 GB/s seq, 209k IOPS randread QD32;
  термал: max 51 °C под sustained-нагрузкой (доп. охлаждение не требуется).
- Отчёт: `docs/xfs-benchmark-2026-09-19.md`; скрипты — `tools/recovery/`.
- APST после reboot снова включён (volatile) — постоянно отключить:
  unit с `Set Features 0x0C=0` или `nvme_core.default_ps_max_latency_us=0`.

## Что дальше (опционально)

- Записать OEM info (Model `Netac NVMe SSD 2TB`, SN `RN######`) — через
  MPUtility (Windows, M.2/PCIe) или вендорную OEM/OTP-команду.
- Vendor control requests `bReq=0x0B/0x0C` (план из RESEARCH_NOTES) —
  не понадобились: оживление шло напрямую по PCIe.
- UART hardware recovery — не понадобился.

## Артефакты

- `tools/recovery/` — user-space NVMe (VFIO cdev + iommufd), ioctl passthrough,
  проверенные сценарии (`mega4.sh` и др.). См. `tools/recovery/README.md`.
- `docs/recovery-runbook.md` — хронология миссии и ключевые находки.
- Прошивки: `~/ai-workstation/Archives/Hardware/Storage/Netac-NV7000/.../m2_0536`
  (копия на target: `~/netac-recovery/m2_0536/`).

## Текущее состояние кода

- Rust workspace (`netac-cli`, `nvme-core`, `innogrit-protocol`, `transport`) —
  без изменений; UASP/JMS583-путь не использовался.
- Реальный recovery выполнен Python-тулингом из `tools/recovery/`.
