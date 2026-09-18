# Netac NV7000 (Phoenix Edition) — XFS setup + benchmark, 2026-09-19

Диск после recovery: `1dbe:5236` (IG5236 RainierPC), FW `3.S.J.11` (rdisk),
SN `RN-PHOENIX-0001`, 2.00 TB, PCIe 4.0 x4.

## Конфигурация хранилища

- **Точка монтирования: `/mnt/2TB-NVMe`** (label `2TB-NVMe`).
- ФС: **XFS** (`mkfs.xfs -m reflink=1,crc=1`, agcount=12, log 238465 blk).
- `/etc/fstab`:
  ```
  /dev/disk/by-label/2TB-NVMe /mnt/2TB-NVMe xfs noatime,lazytime,logbsize=256k,nosuid,nodev,nofail,x-gvfs-show,rw 0 0
  ```
- Автомонтирование проверено reboot'ом (смонтировался сам, log recovery clean).
- `fstrim.timer` — enabled + active (еженедельный TRIM).
- APST: отключён вручную на время тестов (volatile; после reboot снова включён).
  Для постоянного отключения: unit с `Set Features FID 0x0C = 0` или
  kernel cmdline `nvme_core.default_ps_max_latency_us=0`.

## Бенчмарки fio 3.42 (libaio, direct=1)

| Тест | Результат |
|---|---|
| SEQ WRITE 1M, QD32, 16G | **5659 MiB/s (5.9 GB/s)**, avg lat 5.6 ms |
| SEQ READ 1M, QD32, 16G | **6815 MiB/s (7.1 GB/s)**, avg lat 4.7 ms |
| SEQ WRITE 128K, QD16, 8G | **6306 MiB/s (6.6 GB/s)**, avg lat 308 µs |
| SEQ READ 128K, QD16, 8G | **7136 MiB/s (7.5 GB/s)**, avg lat 273 µs |
| RAND READ 4K, QD1 | 19.1k IOPS, avg clat **48.9 µs** (97% < 50 µs) |
| RAND READ 4K, QD32 | **209k IOPS**, 815 MiB/s, avg clat 149 µs |
| RAND WRITE 4K, QD32 | 27.0k IOPS, 105 MiB/s, avg lat 1.19 ms |
| MIX 70/30 4K, QD32 | read **142k IOPS** / write 60.8k IOPS |
| SMALL FILES (20k × 16K) write | 4.5k IOPS |
| SMALL FILES (20k × 16K) read | 4.7k IOPS |

Замечание: 4K random write относительно невысокий (27k IOPS) — особенность
этой recovery-FW-сборки (semi-write, нет HMB). Для бытовых задач некритично.

## Термальный тест (60s sustained)

- SEQ WRITE 1M QD32: **5958 MiB/s**, 349 GiB за 60 s.
- SEQ READ 1M QD32: **6818 MiB/s**, 400 GiB за 60 s.
- Температура: idle 34 °C → **max 51 °C** (avg 46.8 °C), троттлинга нет.
- Порог Warning (SMART) — 120 °C. **Дополнительное охлаждение не требуется.**

## SMART после всех тестов

- Percentage Used: **0 %**, Available Spare: **100 %**
- Data Units Written: **430 GB** (всего с момента recovery)
- Media and Data Integrity Errors: **0**, Unsafe Shutdowns: 1 (до тестов)

## Наблюдения

- 2 события `AMD-Vi: IO_PAGE_FAULT address=0x0` от nvme в предыдущем boot
  (во время бенчмарков). На работу не повлияло; следить.
- `df` показывает ~36 GB «занято» на пустой ФС — accounting артефакт
  reserved-пула XFS; `xfs_spaceman` подтверждает, что занято 164 блока.
  Косметика, на доступность не влияет.

## Артефакты

- Скрипты: `tools/recovery/{apst_off.py,bench.sh,thermal_test.sh}`.
- Отчёт снят 2026-09-19 (CachyOS, Linux 7.2.3).
