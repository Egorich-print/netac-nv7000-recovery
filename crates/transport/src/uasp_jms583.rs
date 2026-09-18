use crate::scsi::{ScsiDirection, ScsiError, ScsiResult, ScsiTransport};
use crate::uasp::{CommandIU, StatusIU};
use std::time::Duration;

const JMICRON_VID: u16 = 0x152D;
const JMS583_PID: u16 = 0x0583;
const UASP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Jms583DeviceInfo {
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
    pub interface: u8,
}

impl std::fmt::Display for Jms583DeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "USB Device")?;
        writeln!(f, "  VID:          0x{:04X}", self.vid)?;
        writeln!(f, "  PID:          0x{:04X}", self.pid)?;
        writeln!(f, "  Manufacturer: {}", self.manufacturer)?;
        writeln!(f, "  Product:      {}", self.product)?;
        writeln!(f, "  Serial:       {}", self.serial)
    }
}

pub fn scan_jms583() -> Result<Vec<Jms583DeviceInfo>, ScsiError> {
    let devices = rusb::devices()
        .map_err(|e| ScsiError::Transport(format!("enumerate: {e}")))?;

    let mut results = Vec::new();

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if desc.vendor_id() != JMICRON_VID || desc.product_id() != JMS583_PID {
            continue;
        }

        let handle = match device.open() {
            Ok(h) => h,
            Err(_) => continue,
        };

        let manufacturer = handle
            .read_manufacturer_string_ascii(&desc)
            .unwrap_or_default();
        let product = handle
            .read_product_string_ascii(&desc)
            .unwrap_or_default();
        let serial = handle
            .read_serial_number_string_ascii(&desc)
            .unwrap_or_default();

        for i in 0..desc.num_configurations() {
            let config = match device.config_descriptor(i) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for iface in config.interfaces() {
                for idesc in iface.descriptors() {
                    if idesc.class_code() == 0x08 && idesc.sub_class_code() == 0x06 {
                        let proto = idesc.protocol_code();
                        if proto == 0x50 || proto == 0x62 {
                            results.push(Jms583DeviceInfo {
                                vid: desc.vendor_id(),
                                pid: desc.product_id(),
                                manufacturer: manufacturer.clone(),
                                product: product.clone(),
                                serial: serial.clone(),
                                interface: idesc.interface_number(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}

pub struct UaspDevice {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    interface: u8,
    trace_level: u8,
}

impl UaspDevice {
    pub fn open(info: &Jms583DeviceInfo) -> Result<Self, ScsiError> {
        let dev = rusb::open_device_with_vid_pid(info.vid, info.pid)
            .ok_or(ScsiError::NotConnected)?;

        dev.detach_kernel_driver(info.interface).ok();
        dev.claim_interface(info.interface)
            .map_err(|e| ScsiError::Transport(format!("claim: {e}")))?;

        // Switch to UAS alt setting (1)
        dev.set_alternate_setting(info.interface, 1)
            .map_err(|e| ScsiError::Transport(format!("alt setting: {e}")))?;

        Ok(Self {
            handle: dev,
            interface: info.interface,
            trace_level: 0,
        })
    }

    pub fn set_trace_level(&mut self, level: u8) {
        self.trace_level = level;
    }
}

impl ScsiTransport for UaspDevice {
    fn execute(
        &mut self,
        cdb: &[u8],
        direction: ScsiDirection,
        buffer_len: usize,
        data_out: &[u8],
    ) -> Result<ScsiResult, ScsiError> {
        static TAG: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(1);
        let tag = TAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let cmd_iu = CommandIU::new(tag, 0, cdb);
        let cmd_bytes = cmd_iu.as_bytes().to_vec();

        if self.trace_level >= 1 {
            let dir_str = match direction {
                ScsiDirection::In => "IN",
                ScsiDirection::Out => "OUT",
                ScsiDirection::None => "NONE",
            };
            eprintln!(
                "[trace] UASP cmd: CDB={cdb:02x?} dir={dir_str} len={buffer_len} tag={tag}"
            );
        }
        if self.trace_level >= 2 {
            eprintln!(
                "[trace] Command IU ({}): {}",
                cmd_bytes.len(),
                hex_fmt(&cmd_bytes, 16)
            );
        }

        // UAS endpoints: cmd=0x01, status=0x82, datain=0x83, dataout=0x04
        self.handle
            .write_bulk(0x01, &cmd_bytes, UASP_TIMEOUT)
            .map_err(|e| ScsiError::Transport(format!("write cmd: {e}")))?;

        if direction == ScsiDirection::Out && !data_out.is_empty() {
            if self.trace_level >= 1 {
                eprintln!("[trace] Data OUT: {} bytes", data_out.len());
            }
            self.handle
                .write_bulk(0x04, data_out, UASP_TIMEOUT)
                .map_err(|e| ScsiError::Transport(format!("write data: {e}")))?;
        }

        let mut data = Vec::new();
        if direction == ScsiDirection::In && buffer_len > 0 {
            let mut buf = vec![0u8; buffer_len];
            let n = self
                .handle
                .read_bulk(0x83, &mut buf, UASP_TIMEOUT)
                .map_err(|e| ScsiError::Transport(format!("read data: {e}")))?;
            data = buf[..n].to_vec();
            if self.trace_level >= 1 {
                eprintln!("[trace] Data IN: {} bytes", data.len());
            }
            if self.trace_level >= 2 {
                eprintln!("[trace] Data IN hex: {}", hex_fmt(&data, 16));
            }
        }

        let mut status_buf = vec![0u8; StatusIU::size()];
        let n = self
            .handle
            .read_bulk(0x82, &mut status_buf, UASP_TIMEOUT)
            .map_err(|e| ScsiError::Transport(format!("read status: {e}")))?;

        if self.trace_level >= 2 {
            eprintln!(
                "[trace] Status IU raw ({n}): {}",
                hex_fmt(&status_buf[..n], 16)
            );
        }

        let status_iu =
            StatusIU::parse(&status_buf[..n]).ok_or(ScsiError::InvalidResponse)?;

        if self.trace_level >= 1 {
            eprintln!(
                "[trace] Status: 0x{:02X} good={} check_cond={}",
                status_iu.status,
                status_iu.is_good(),
                status_iu.is_check_condition()
            );
        }

        if status_iu.is_check_condition() {
            if self.trace_level >= 1 {
                eprintln!("[trace] Sense: {}", hex_fmt(&status_iu.sense, 16));
            }
            return Err(ScsiError::CheckCondition {
                sense: status_iu.sense.to_vec(),
            });
        }

        Ok(ScsiResult {
            status: status_iu.status,
            data,
        })
    }
}

fn hex_fmt(data: &[u8], _max: usize) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(JMICRON_VID, 0x152D);
        assert_eq!(JMS583_PID, 0x0583);
    }

    #[test]
    fn device_info_display() {
        let info = Jms583DeviceInfo {
            vid: 0x152D,
            pid: 0x0583,
            manufacturer: "JMicron".into(),
            product: "JMS583".into(),
            serial: "123456".into(),
            interface: 0,
        };
        let s = format!("{info}");
        assert!(s.contains("VID"));
        assert!(s.contains("0x152D"));
    }
}
