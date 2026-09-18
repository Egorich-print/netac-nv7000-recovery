# JMS583 research notes
# 
# USB layout:
#   Alt 0 (BOT):  IN=0x81, OUT=0x02
#   Alt 1 (UAS):  CMD=0x01 OUT, DATA=0x04 OUT, STATUS=0x82 IN, DATA_IN=0x83 IN
#
# Known vendor control: bmReq=0xC0, bReq=0x0A → 16 bytes (bridge info)
#   Response: 0a 00 00 00 00 00 03 06 10 00 06 00 05 00 0a 14
#
# Linux findings:
#   1. UASP CMD IU always accepted (endpoint 0x01) — bridge is alive
#   2. All data/status phases timeout — NVMe controller hangs the bridge
#   3. BOT mode also fails — bridge won't respond in alt 0
#   4. Control transfers fail after bridge is hung
#
# Key insight: NVMe controller IG5236 is hung, blocks all NVMe commands.
# Recovery requires either:
#   A. JMicron IOCTL 0x4D014 format reverse-engineering (Windows driver)
#   B. Direct IG5236 UART access (hardware pins on PCB)
#   C. JMS583 vendor control to reset PCIe link
#
# Next steps for tomorrow:
#   1. Fresh device connect → immediately probe vendor control requests
#   2. Try bReq=0x0B,0x0C with various wValue/wIndex for bridge reset
#   3. If no progress, switch to UART hardware recovery
#
# Relevant files:
#   crates/transport/src/uasp_jms583.rs  — UASP transport (ready)
#   crates/transport/src/nvme_tunnel.rs  — 0xA1 NVMe tunnel (ready)
#   crates/transport/src/scsi_commands.rs — SCSI INQUIRY/TUR/CAPACITY/SENSE (ready)
#   crates/netac-cli/src/main.rs          — CLI with all commands (ready)
#   docs/protocol.md                      — InnoGrit protocol specification
#   docs/uasp-jms583.md                   — JMS583 UASP transport spec
#
# Build status: ✅ 20 tests pass, cargo build clean (macOS + Linux)
