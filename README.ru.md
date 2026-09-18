# Netac NV7000 (InnoGrit IG5236) — исследование и recovery-тулкит

**English version: [README.md](README.md)**

Неофициальный реверс-инжиниринг и Linux-инструментарий для восстановления
NVMe SSD **Netac NV7000** (2 ТБ) на контроллере **InnoGrit IG5236 «RainierPC»**.

Репозиторий описывает и автоматизирует **полное оживление** диска, который был
абсолютно мёртв: Linux не мог его даже опросить
(`nvme: error -ENODEV: probe failed`, BAR0 читался как `0xFFFFFFFF`), а сам
диск вешал USB-мост JMS583. Восстановление сделано **без Windows и без
вендорской MPUtility** — через user-space NVMe-драйвер поверх
**VFIO cdev + iommufd**.

После восстановления диск работает как обычный SSD на 2 ТБ (бенчмарки ниже).

## Что внутри

```
crates/                    Rust-исследование (парсер GRIT, NVMe-ядро,
                           протокол InnoGrit, UASP/JMS583 транспорт)
tools/recovery/            Python-инструменты восстановления и проверки
docs/protocol.md           реверс вендорского протокола InnoGrit
docs/protocol-uasp-jms583.md  заметки по UASP (первая попытка)
docs/recovery-runbook.md   полная история восстановления и грабли (читать тут)
docs/xfs-benchmark-2026-09-19.md  бенчмарки XFS и термалка после recovery
```

`tools/recovery/`:

| Файл | Назначение |
|---|---|
| `netac_pipeline.py` | user-space NVMe-драйвер через VFIO cdev + iommufd; полный конвейер ROM → loader → programmer → rdisk → finalize + вендорские команды |
| `nvme_admin_passthru.py` | admin passthrough через ioctl `/dev/nvmeX` + вендорские команды |
| `oem_brand.py` | запись Model/Serial (OEM info) в 4K-конфиг прошивки |
| `mega4.sh`, `revive_rdisk.sh`, `boot_recover.sh` | готовые сценарии восстановления |
| `bench.sh`, `thermal_test.sh`, `apst_off.py` | проверка после восстановления |

## Проверенное окружение

- Netac NV7000 2 ТБ, IG5236, вендорский пакет прошивок `3.S.J.11` (сборка rdisk).
- Linux 7.2.x (CachyOS), AMD, PCIe 4.0 x4.
- В ядре 7.x удалён легаси VFIO container API — код использует современный
  **cdev + iommufd** (`/dev/vfio/devices/vfioN` + `/dev/iommu`).
- Прошивки вендора **не входят в репозиторий**. Положите каталог `m2_0536/`
  из своего recovery-пакета в `~/netac-recovery/` (путь переопределяется
  через `NETAC_FW_DIR`).

## Ключевые находки (самое неочевидное)

1. **VFIO cdev + iommufd.** Легаси `/dev/vfio/vfio` в 7.x больше нет. Плюс
   обязательно `disable_idle_d3=Y` для `vfio-pci` и `d3cold_allowed=0` —
   иначе контроллер уходит в D3cold и пропадает (config space читается как `0xff`).
2. **CQ у ROM-контроллера глубиной 2** (несмотря на CAP). Ставьте
   `AQA = 1 | (1<<16)` (`NETAC_QSIZE=2`), иначе после двух команд комплишены
   прекращаются и всё «виснет».
3. **FW download в режиме programmer требует `NSID=0`** (не `0xFFFFFFFF`),
   иначе `Invalid Field in Command` (SC=0x02).
4. **Runtime-прошивка (rdisk) активируется только после полного сброса
   питания** (обесточить 3.3 В). Тёплый reboot оставляет работать programmer.
5. **Никогда не делайте `remove`/`rescan` устройства в D3cold** — оно
   пропадает с шины и может повесить машину. Держите его в D0.
6. **OEM info (Model/Serial)** лежит в 4K-конфиге: Model на `cfg+0x430` (40 Б),
   SN на `cfg+0x410` (20 Б), запись — `SetFWconfig (0xE5)`.
7. IG5236 склонен зависать в глубоких power state — стоит отключать APST
   (`Set Features 0x0C = 0` или `nvme_core.default_ps_max_latency_us=0`).

Подробности: [`docs/recovery-runbook.md`](docs/recovery-runbook.md).

## Быстрый старт

```bash
# зависимости: python3, vfio-pci, pciutils; smartmontools/fio — для проверки
git clone https://github.com/Egorich-print/netac-nv7000-recovery
cd netac-nv7000-recovery

# 1) прошивки вендора -> ~/netac-recovery/m2_0536/
#    (loader.fw, programmer*.fw, rdisk*.fw, Config_New.ini)

# 2) держим устройство в D0 и не даём nvme его трогать
sudo modprobe vfio-pci vfio_iommu_type1
echo Y  | sudo tee /sys/module/vfio_pci/parameters/disable_idle_d3
echo 0  | sudo tee /sys/bus/pci/devices/0000:01:00.0/d3cold_allowed
echo on | sudo tee /sys/bus/pci/devices/0000:01:00.0/power/control
sudo modprobe -r nvme

# 3) привязываем vfio-pci и запускаем весь конвейер в ОДНОМ процессе
echo vfio-pci   | sudo tee /sys/bus/pci/devices/0000:01:00.0/driver_override
echo 0000:01:00.0 | sudo tee /sys/bus/pci/drivers/vfio-pci/bind
sudo env NETAC_QSIZE=2 python3 tools/recovery/netac_pipeline.py megaflash4

# 4) ПОЛНЫЙ power-cycle (БП выключить/включить), затем проверка
lsblk
sudo smartctl -a /dev/nvme0 | head -30
```

Если PCI-функция зависла (BAR0 = `0xFFFFFFFF`) — смотрите
`tools/recovery/revive_rdisk.sh`: там безопасная последовательность
remove/rescan + `setpci`. **Не трогайте** secondary bus reset родительского порта.

## Результаты после восстановления (на этом диске)

| Метрика | Значение |
|---|---|
| Последовательное чтение/запись (1M QD32) | **7.1 / 5.9 ГБ/с** |
| Последовательное чтение/запись (128K QD16) | 7.5 / 6.6 ГБ/с |
| Случайное чтение 4K QD32 | **209k IOPS** |
| Случайное чтение 4K QD1 | 19.1k IOPS @ 49 µs |
| Смешанная 70/30 4K QD32 | 142k чтение / 61k запись IOPS |
| Макс. температура при 60 с непрерывной записи | **51 °C** (с радиатором) |
| Состояние после тестов | Percentage Used 0 %, 0 ошибок |

## Отказ от ответственности

Это **неофициальный** инструментарий. Он общается с контроллером напрямую,
пишет прошивку и 4K-конфиг и при неаккуратном использовании может навсегда
«окирпичить» диск или повесить машину. Всё предоставляется **как есть**, без
гарантий. Вендорские образы прошивок здесь не распространяются.
Используйте на свой риск.

## Лицензия

[MIT](LICENSE) © 2026 3g0r1ch (Egorich-print)
