# Changelog

## 2026-09-19

- **Netac NV7000 восстановлен** (unbrick InnoGrit IG5236 через Linux/VFIO,
  прямой PCIe). Диск: `1dbe:5236`, FW `3.S.J.11` (rdisk), 2.00 TB, I/O 3–4.5 GB/s.
- Добавлен `tools/recovery/` — user-space NVMe драйвер (VFIO cdev + iommufd),
  ioctl passthrough и проверенные сценарии; `docs/recovery-runbook.md`.
- Ключевые находки: легаси VFIO container API удалён в 7.x (нужен cdev+iommufd);
  CQ depth=2 (QSIZE=2); FW download в programmer-режиме требует NSID=0;
  активация rdisk — только после полного power-cycle.

## 2026-08-06

- Проект выделен из `New OpenCode Project` в canonical-репозиторий `~/ai-workstation/Projects/netac/`.
- Перенесены: `crates/`, `Cargo.toml`, `Cargo.lock`, `docs/`, `gen_manual.py`, `grit_checksum.py`, `setup.bat`.
- JMS583 research notes → `docs/history/RESEARCH_NOTES.md`.
