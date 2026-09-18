# Innogrit IG5236 Recovery Protocol

## Overview

Reverse-engineered from `middleware.dll` (255 KB Win32 PE) shipped with
Netac NV7000 MPUtility. The DLL implements firmware update and configuration
for Innogrit IG5236 (and related) NVMe controllers.

## Architecture

```
MPutility.exe (Python/PyInstaller)
    |
    +-- middle_io.py (ctypes wrapper)
    |       |
    |       +-- middleware.dll (MID_* exports)
    |               |
    |               +-- DeviceIoControl (Windows)
    |               |       IOCTL_STORAGE_PROTOCOL_COMMAND (0x2DD3C0)
    |               |       ProtocolType = 3 (Nvme)
    |               |
    |               +-- CreateFileA (open physical drive)
    |
    +-- tab_mp.py (UI orchestration)
    +-- Config_New.ini (production config)
```

## Transport Layers

### Direct NVMe (Windows)

```
Rust/MPTool
 -> IOCTL_STORAGE_PROTOCOL_COMMAND (0x2DD3C0)
 -> storport/nvme.sys
 -> PCIe NVMe
 -> IG5236
```

IOCTL wrapper structure (partial, inferred from DLL):

```
STORAGE_PROTOCOL_COMMAND {
    offset  size  field
    0x00    4     Version         = 1
    0x04    4     Length          = 0x54 (84)
    0x08    4     ProtocolType    = 3 (Nvme)
    0x0C    4     Flags           = 0x80000000
    0x18    4     CommandLength   = 0x40 (64 = NVMe SQE)
    0x28    4     TimeOutValue    = 0x3C (60 s)
    0x30    4     DataFromDevOff  = 0x90
    0x34    4     DataToDevOff    = 0x90
    0x50    64    NVMe SQE
}
```

### USB via JMS583 (Linux/macOS)

```
Rust
 -> libusb
 -> UASP (USB Attached SCSI Protocol)
 -> SCSI Command IU
 -> NVMe Admin Command passthrough
 -> JMicron JMS583 bridge
 -> IG5236
```

Three code paths in `middleware.dll`:

| Interface | Value | Function | Notes |
|-----------|-------|----------|-------|
| NVMe      | 0x20  | `0x4BC0` → `DeviceIoControl(0x2DD3C0)` | Direct NVMe IOCTL |
| USB JMI   | 0x10  | `0x4BC0` → `0xF890` | JMS583 UASP path |
| SATA      | 0/1   | `0x4BC0` → `0xFEA0` | ATA passthrough |

## Command Types

### Innogrit Vendor Commands

Sent via the same SQE-based mechanism but with vendor-specific
payload encoding.

Base structure (size = 0xD0 or 0xE4 bytes):

```
struct InnogritVendorCmd {
    // NVMe SQE (64 bytes at offset 0x50 from IOCTL header)
    uint32_t cdw0;          // 0x00000000
    uint32_t nsid;          // 0xFFFFFFFF (broadcast)
    uint64_t reserved;      // zero
    uint64_t mptr;          // zero
    uint64_t prp1;          // zero

    // Vendor payload (after SQE)
    uint8_t  payload[?];    // contains INNOGRIT magic + opcode
};
```

Magic bytes present in ALL vendor commands:

| Offset | Value | ASCII |
|--------|-------|-------|
| +0x48  | `0x4F4E4E49` | "INNO" |
| +0x4C  | `0x54495247` | "GRIT" |

### Known Opcodes

| Opcode | CmdSize | DLL Function | Purpose |
|--------|---------|--------------|---------|
| 0x00   | 0xD0    | `MID_ToRecoveryMode` | Enter ROM/bootloader mode |
| 0x03   | 0xE4    | `MID_UpdateFW` | Finalize firmware / trigger update |
| 0x0B   | 0xFF    | `MID_SetFWconfig` | Write NAND geometry + production config |

### Standard NVMe Commands

| NVMe Opcode | Phase Code | Library Function | Purpose |
|-------------|------------|------------------|---------|
| 0x11        | 3          | `dispatch@0x2A22A` | Firmware Download |
| 0x10/0x13   | 8          | `dispatch@0x2A22A` | Firmware Commit/Activate |
| 0x08        | 1, 5       | `dispatch@0x2A22A` | Set Features |
| 0x04        | 2          | `dispatch@0x2A22A` | Delete CQ |
| 0x12        | 4          | `dispatch@0x2A22A` | Device Self-test (?) |
| 0x02        | -          | `MID_GetLogPage_SMART` | Get Log Page (SMART) |
| 0x06        | -          | `MID_GetMpInfo` | Identify (vendor-specific) |

The phase code → NVMe opcode mapping is at `middleware.dll` RVA 0x2A22A.

## Firmware Update Pipeline

### Stage Sequence

```
 1. MID_ToRecoveryMode (opcode=0)
        ↓  Controller enters ROM/bootloader mode
 2. NVMe Firmware Download (0x11)  —  loader.fw (20 KB)
 3. NVMe Firmware Commit (0x10)    —  Activate loader
        ↓  Controller reboots into loader
 4. NVMe Firmware Download (0x11)  —  programmer*.fw (~660 KB)
 5. NVMe Firmware Commit (0x10)    —  Activate programmer
        ↓  NAND initialization
 6. NVMe Firmware Download (0x11)  —  rdisk*.fw (~870 KB)
 7. NVMe Firmware Commit (0x10)    —  Activate rdisk
        ↓  Controller runs full FW
 8. MID_SetFWconfig (opcode=0x0B)  —  Config_New.ini + NAND geometry
 9. MID_UpdateFW (opcode=0x03)     —  Finalize production configuration
10. Power cycle                     —  Reset to activate
11. Verify: MID_GetMpInfo / SMART
```

### Chunked Data Transfer

NVMe Firmware Download (0x11) transfers firmware in chunks.

Host buffer for each chunk is allocated via `HeapAlloc(0x3000)` (12 KB
in NVMe path, could differ in UASP path). The exact chunk size is not a
fixed constant but is determined by:

- `DataToDeviceLength` in `STORAGE_PROTOCOL_COMMAND`
- NVMe `NUMD` (Number of Dwords - 1) in CDW10
- Offset `OFST` in CDW11

SQE for Firmware Download:

```
CDW0  = (0x11 << 5) | flags     // opcode 0x11 in bits 7:0
CDW1  = 0xFFFFFFFF               // NSID broadcast
CDW10 = NUMD                     // (chunk_size / 4) - 1
CDW11 = OFST                     // offset / 4
```

Firmware Commit (0x10):

```
CDW0  = (0x10 << 5) | flags
CDW10 = slot | action            // bits 2:0 = slot, bits 5:3 = action
```

| Action | Value | Meaning |
|--------|-------|---------|
| Save   | 0     | Save to slot |
| Activate | 1   | Activate from slot |

### Recovery Mode Entry

`MID_ToRecoveryMode` sends Innogrit vendor command (opcode=0, size=0xD0).

This tells the IG5236 ROM bootloader to stay in recovery mode,
allowing firmware download without normal NVMe initialization.

The controller must be detected as `DRV_ROM_DISK` or `DRV_NOT_INNOGRIT_DISK`
before the recovery command can be sent.

### Configuration (MID_SetFWconfig)

Sends Innogrit vendor command (opcode=0x0B, size=0xFF).

Payload is built from `Config_New.ini` and contains:

| Field | Source | Purpose |
|-------|--------|---------|
| NAND Major ID | INI `NAND_MAJORID` | YMTC = 0x9C |
| NAND Minor ID | INI `NAND_MINORID` | X2-9060 = 0xFE |
| Disk Size (GB) | INI | 2048 for NV7000 2TB |
| SN Mask | INI `SN_MASK` | `RN######` |
| PMIC Type | INI | SY21523E / MPS5417 / etc |
| NAND Work Mode | INI | TLC / QLC |
| DDR Config | INI | Speed, type, size |

### Finalize (MID_UpdateFW)

Sends Innogrit vendor command (opcode=3, size=0xE4).

Called after all firmware stages and configuration are complete.
May trigger production state finalization or burn-in.

Python signature:

```python
midUpdateFW(iIndex, PhysicalDrvName, org_fw_base, FWbin,
            iCommit, iPowerOffOnSec, iNeedPowerOffOn,
            ProductionInfo)
```

## Error Codes

From `ErrCodeList.ini`:

| Code  | Description |
|-------|-------------|
| 0x000B | Find Bridge but cannot connect to SSD |
| 0x0029 | FW Not Ready |
| 0x3001 | Open device failed |
| 0x3003 | Download FW Fail |
| 0x3004 | Commit Fail |
| 0x300A | To Recovery Mode Fail |
| 0xEE00..EE12 | USB bridge errors |

## Interface Detection

`function@0x2AC0` (called `OpenPhysicalDevice`-like):

1. `CreateFileA(PhysicalDrvName, GENERIC_RW, ...)`
2. `DeviceIoControl(h, 0x2D1400, ...)` → checks response:
   - Value 0x11 → interface = 0x20 (NVMe)
   - Value 0x07 → calls sub-detection → interface = detected type
   - Otherwise → interface = 0xFF (unknown)

The returned interface type selects the command path for all MID_* functions.

## Key DLL Internal Functions

| RVA | Size | Role |
|-----|------|------|
| 0x2AC0 | ~240 | Open device + detect interface type |
| 0x4BC0 | ~600 | Innogrit vendor command dispatcher |
| 0xF890 | ? | USB JMI (JMS583) command sender |
| 0xFEA0 | ? | SATA command sender |
| 0x2A1F2 | ~500 | NVMe FW download/commit workflow |
| 0x2A5FD | ~800 | NVMe admin command SQE builder |
| 0x2A620 | ~800 | SQE → STORAGE_PROTOCOL_COMMAND |

## Implementation Notes

### USB (JMS583) Path

The DLL has a separate USB/JMI command path at `0xF890`.
This function sends native SCSI commands over UASP instead of
using `IOCTL_STORAGE_PROTOCOL_COMMAND`.

- Interface type `0x10` (USB_JMI) → `0xF890`
- Interface type `0` or `1` (SATA) → `0xFEA0`
- Interface type `0x20` (NVMe) → `DeviceIoControl(0x2DD3C0)`

For recovery over USB, the UASP path must be implemented directly
(libusb + UASP protocol), bypassing the OS NVMe driver.

### Opcode Dispatch

The dispatch table at RVA 0x2A22A maps "phase codes" to NVMe opcodes:

```
[esi] == 1 → op 0x08 (Set Features)
[esi] == 2 → op 0x04 (Delete CQ)
[esi] == 3 → op 0x11 (Firmware Download)
[esi] == 4 → op 0x12 (Device Self-test)
[esi] == 5 → op 0x08 (Set Features)
[esi] == 8 → op 0x10 (Firmware Commit)
```

These are the standard NVMe admin commands used during production firmware update.
