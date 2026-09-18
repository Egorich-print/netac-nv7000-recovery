# JMS583 UASP Transport Protocol

## 1. Scope

This document describes the USB transport layer for the JMicron JMS583 NVMe bridge
used by the Netac NV7000 (Innogrit IG5236) recovery tool.

**Purpose:**
- Enumerate JMS583 on USB bus
- Send SCSI commands over UASP (USB Attached SCSI Protocol)
- Tunnel NVMe Admin Commands through the bridge

## 2. Hardware Topology

```
Host (macOS/Linux)
  │
  │  USB 3.x (5/10 Gbps)
  │
JMS583 (USB-to-NVMe bridge chip)
  │
  │  PCIe / NVMe
  │
Innogrit IG5236 (NVMe SSD controller)
  │
  └── YMTC X2-9060 NAND
```

The JMS583 is a bridge that exposes the connected NVMe device as a USB Mass Storage
device. It speaks UASP (USB Attached SCSI Protocol) on the USB side and translates
SCSI commands to NVMe commands on the device side.

## 3. USB Identification

| Field       | Value    | Description               |
|-------------|----------|---------------------------|
| VID         | `0x152D` | JMicron Technology Corp.  |
| PID         | `0x0583` | JMS583                    |
| Class       | `0x08`   | Mass Storage              |
| Subclass    | `0x06`   | SCSI Transparent Command Set |
| Protocol    | `0x62`   | UAS (USB Attached SCSI)   |

### USB Descriptors (from Windows)

```
Device Descriptor:
  bcdUSB:         0x0320 (USB 3.2)
  idVendor:       0x152D
  idProduct:      0x0583
  bcdDevice:      0x0100

Configuration Descriptor:
  bNumInterfaces: 1

  Interface 0:
    bInterfaceClass:    0x08 (Mass Storage)
    bInterfaceSubClass: 0x06 (SCSI)
    bInterfaceProtocol: 0x62 (UAS)

    Endpoint 0x02: Bulk OUT (Command + Data OUT)
    Endpoint 0x81: Bulk IN  (Data IN)
    Endpoint 0x83: Bulk IN  (Status IN)
    Endpoint 0x04: Bulk OUT (Data OUT, alt)
```

## 4. UASP Transport Model

UASP (USB Attached SCSI Protocol, UAS-2 specification) uses Information Units (IUs)
multiplexed over bulk endpoints:

```
Command IU ───────────┐
  (Bulk OUT endpoint) │
                      v
              +──────────────+
              │  JMS583      │
              │  Bridge      │
              +──────────────+
                      │
Data IU (IN/OUT) ◄────┤
                      │
Status IU ────────────┤
  (Bulk IN endpoint)  v
```

**Endpoint roles (UASP):**

| Endpoint | Direction | Purpose                          |
|----------|-----------|----------------------------------|
| 0x02     | OUT       | Command IU + Task Management IU  |
| 0x81     | IN        | Data IN IU                       |
| 0x83     | IN        | Status IU                        |
| 0x04     | OUT       | Data OUT IU                      |

**Protocol flow (for each command):**

1. Build Command IU with SCSI CDB → send to command endpoint (0x02)
2. Wait for device to indicate readiness (poll status or await R2T)
3. Transfer data IU (if any) → data IN (0x81) or data OUT (0x04)
4. Receive Status IU → read from status endpoint (0x83)

### 4.1 IU Types (UASP)

| IU ID | Name               | Direction | Endpoint |
|-------|--------------------|-----------|----------|
| 0x01  | Command IU         | OUT       | CMD      |
| 0x02  | Data IU            | IN/OUT    | DATA     |
| 0x03  | Task Management IU | OUT       | CMD      |
| 0x04  | Ready to Transfer (R2T) IU | IN | STATUS    |
| 0x05  | Status IU          | IN        | STATUS   |

## 5. UASP Command IU

### 5.1 Structure

```c
struct __attribute__((packed)) uasp_command_iu {
    uint8_t  iu_id;      // 0x01 = Command IU
    uint8_t  reserved;
    uint16_t tag;        // task tag, echo'd in Status IU

    uint8_t  priority;
    uint8_t  reserved2;
    uint8_t  lun;        // logical unit number (usually 0)
    uint8_t  reserved3;

    uint8_t  cdb[16];    // SCSI Command Descriptor Block
};
// Total: 24 bytes header + 16 bytes CDB = 32 bytes
```

**Note:** The UASP spec allows additional CDB bytes (up to 32 total) when
`additional_cdb_length` in the INQUIRY data indicates support. JMS583 uses
16-byte CDBs exclusively.

### 5.2 Task Tag

The task tag is a 16-bit value chosen by the host. It must be unique for
in-flight commands. The Status IU echoes this tag.

### 5.3 Data Direction

The CDB itself encodes direction via the opcode and flags byte. No separate
direction flag exists in the IU header.

## 6. UASP Status IU

```c
struct __attribute__((packed)) uasp_status_iu {
    uint8_t  iu_id;      // 0x05 = Status IU
    uint8_t  reserved;
    uint16_t tag;        // echoes the Command IU tag

    uint8_t  status;     // 0x00 = GOOD, 0x02 = CHECK CONDITION
    uint8_t  reserved2[3];

    uint8_t  sense_data[18]; // optional, for CHECK CONDITION
};
// Total: 8 bytes header + optional sense data
```

Common status values:

| Status             | Value | Meaning                    |
|--------------------|-------|----------------------------|
| GOOD               | 0x00  | Command completed normally |
| CHECK CONDITION    | 0x02  | Error, sense data follows  |
| BUSY               | 0x08  | Device busy, retry later   |
| RESERVATION CONFLICT | 0x18 | Reserved by another initiator |
| TASK SET FULL      | 0x28  | Too many pending commands  |
| ACA ACTIVE         | 0x30  | Auto-contingent allegiance |

## 7. JMS583 NVMe Tunnel

### 7.1 Overview

The JMS583 uses **SCSI opcode `0xA1`** (ATA PASS-THROUGH 12) to tunnel NVMe
admin commands. Despite the opcode name, the bridge interprets the payload
as an NVMe transport request, not an ATA command.

This is a **vendor-specific extension** by JMicron — not part of any SCSI or
ATA standard.

### 7.2 ATA PASS-THROUGH (12) CDB

```c
struct __attribute__((packed)) jms583_cdb {
    uint8_t  opcode;    // 0xA1
    uint8_t  flags;     // direction + protocol flags
    uint8_t  reserved;  // 0x00
    uint16_t tfr_len;   // data transfer length (big endian)
    uint8_t  control;   // 0x00
    uint8_t  reserved2[10];
};
```

**Flags byte encoding (from `middleware.dll` reverse engineering):**

| Bit(s) | Field     | Value | Meaning               |
|--------|-----------|-------|-----------------------|
| 7      | t_length  | 1     | Transfer length valid |
| 6      | t_dir     | 0     | Host-to-Device (OUT)  |
|        |           | 1     | Device-to-Host (IN)   |
| 5      | byte_block| 0     | Sector count          |
| 4-0    | protocol  | 00001 | ATA soft reset        |
|        |           | 00010 | ATA PIO data-in       |
|        |           | 00100 | ATA DMA               |
|        |           | 01110 | **NVMe tunneling**    |

**JMS583 flags values (from DLL):**

| Direction | Flags Byte | Description                     |
|-----------|------------|---------------------------------|
| Write     | `0x81`     | host-to-device, NVMe tunnel     |
| Read      | `0x82`     | device-to-host, NVMe tunnel     |
| Other     | `0x83`     | bidirectional / status only     |

### 7.3 NVMe Payload Format

The NVMe SQE is **NOT embedded in the CDB**. It is placed in a data buffer
that accompanies the UASP command. The JMS583 reads the SQE from the data
stream.

```c
struct __attribute__((packed)) jms583_nvme_packet {
    uint32_t    magic;      // "NVME" = 0x454D564E
    uint8_t     sqe[64];    // NVMe Admin Command SQE
    // optional: command-specific data follows
};
```

**Data flow:**

```
USB Host                              JMS583
  │                                      │
  │  Command IU (opcode=0xA1, flags=0x81)│
  │  + "NVME" + SQE[64] (data OUT)       │
  │─────────────────────────────────────>│
  │                                      │  Parse SQE from data buffer
  │                                      │  Forward over PCIe/NVMe
  │                                      │
  │  Data IU (if command has data IN)    │
  │<─────────────────────────────────────│
  │                                      │
  │  Status IU                           │
  │<─────────────────────────────────────│
```

### 7.4 Command Flow (3 phases)

From `middleware.dll` RVA `0xF890`:

**Phase 1 — Submit Command:**

1. Build a proprietary IU buffer (112 bytes, `0x70`)
2. Copy NVMe SQE to the data area with "NVME" marker
3. Issue `DeviceIoControl(hDevice, 0x4D014, &IU, 0x70, ...)`

**Phase 2 — Transfer Data** (if applicable):

1. Update IU with direction flags and data pointer
2. Issue `DeviceIoControl(hDevice, 0x4D014, &IU, 0x70, ...)`

**Phase 3 — Get Status:**

1. Update IU with status opcode (`0x8FA1`), IU length = `0x118` (280)
2. Issue `DeviceIoControl(hDevice, 0x4D014, &IU, 0x70, ...)`
3. Return status to caller

**Note:** On macOS/Linux via libusb, these 3 phases become UASP IUs on bulk
endpoints, not Windows IOCTLs.

## 8. SCSI Commands Used

### 8.1 INQUIRY

```
CDB: 12 01 00 00 FF 00 00 00 00 00 00 00 00 00 00 00
      |  |  |  |  |  |
      |  |  |  |  +-- allocation length (0xFF = 255 bytes)
      |  |  |  +----- page code (0 = standard)
      |  |  +-------- EVPD (0 = standard inquiry)
      |  +----------- commands (0 = standard inquiry)
      +-------------- opcode (0x12 = INQUIRY)

Response (36 bytes standard):
  Peripheral Device Type: 0x00 (SBC, Direct Access)
  Removable: 0
  Version: 0x06 (SPC-4)
  Vendor: "JMicron "
  Product: "Tech Media   "
  Revision: "xxxx"
```

### 8.2 READ CAPACITY (16)

```
CDB: 9E 10 00 00 00 00 00 00 00 00 00 00 00 20 00 00
      |  |  |                    |        |
      |  |  |                    |        +-- allocation length (0x20 = 32)
      |  |  |                    +----------- PMI (0 = complete capacity)
      |  |  +------------------------------- LBA (0 = return max capacity)
      |  +---------------------------------- service action (0x10)
      +------------------------------------- opcode (0x9E = READ CAPACITY 16)
```

### 8.3 NVMe Admin Tunnel (via ATA PASS-THROUGH)

```
CDB: A1 81 00 LL HH 00 00 00 00 00 00 00 00 00 00 00
      |  |  |  |  |
      |  |  |  |  +-- transfer length high byte (big endian)
      |  |  |  +----- transfer length low byte
      |  |  +-------- reserved
      |  +----------- flags (0x81 = write, 0x82 = read)
      +-------------- opcode (0xA1 = ATA PASS-THROUGH)
```

## 9. Status Handling

```rust
/// UASP Status IU interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UaspStatus {
    /// Command completed successfully
    Good,
    /// Error — check sense data
    CheckCondition,
    /// Device busy — retry
    Busy,
    /// Other failure
    Failed(u8),
}
```

After receiving a Status IU with CHECK CONDITION, the host should issue
a REQUEST SENSE command (`0x03`) to retrieve the sense data.

## 10. Error Codes

| Error                      | Likely Cause                     |
|----------------------------|----------------------------------|
| No device found            | JMS583 not connected / not in UAS mode |
| Permission denied          | Need root / udev rule on Linux   |
| UASP command timeout       | Bridge or SSD not responding     |
| CHECK CONDITION sense      | NVMe command returned error      |
| Status IU not received     | Protocol mismatch / firmware bug |

## 11. Implementation Roadmap

```
Phase 1: USB enumeration + scan CLI
   └── netac-recover scan → detect JMS583, show endpoints

Phase 2: UASP I/O primitives
   └── Command IU builder, Status IU parser

Phase 3: SCSI transport
   └── INQUIRY → verify bridge is alive
   └── READ CAPACITY → confirm SSD is visible

Phase 4: NVMe tunnel
   └── ATA PASS-THROUGH (0xA1) → NVMe admin commands
   └── Identify Controller

Phase 5: Innogrit protocol over NVMe tunnel
   └── Recovery mode → FW download → Finalize
```

## 12. Reverse Engineering References

- `middleware.dll` from `MPutility.exe` (Innogrit MPTool)
- USB transport handler: RVA `0xF890` in `middleware.dll`
- Dispatch function: RVA `0x4BC0`
- Interface constant: `USB_JMI = 0x10` (dispatch selector)
- IOCTL code: `0x4D014` (Windows driver, NOT used on macOS/Linux)
- ATA PASS-THROUGH opcode: `0xA1`
- JMS583 driver: JMicron USB-to-NVMe bridge (proprietary)

**Key differences Windows vs. native UASP:**

| Aspect          | Windows (DLL)                | Linux/macOS (this tool)       |
|-----------------|------------------------------|-------------------------------|
| Transport API   | DeviceIoControl + kernel drv | libusb + raw UASP IUs         |
| Buffer mgmt     | VirtualAlloc bounce buffers  | Vec allocation                 |
| Endpoint access | Driver-managed               | Direct bulk transfers          |
| Error handling  | GetLastError                 | libusb error codes             |
