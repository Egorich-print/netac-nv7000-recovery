use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenseKey {
    NoSense,
    NotReady,
    MediumError,
    HardwareError,
    IllegalRequest,
    UnitAttention,
    DataProtect,
    BlankCheck,
    VendorSpecific(u8),
    Unknown(u8),
}

impl fmt::Display for SenseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SenseKey::NoSense => write!(f, "NO SENSE"),
            SenseKey::NotReady => write!(f, "NOT READY"),
            SenseKey::MediumError => write!(f, "MEDIUM ERROR"),
            SenseKey::HardwareError => write!(f, "HARDWARE ERROR"),
            SenseKey::IllegalRequest => write!(f, "ILLEGAL REQUEST"),
            SenseKey::UnitAttention => write!(f, "UNIT ATTENTION"),
            SenseKey::DataProtect => write!(f, "DATA PROTECT"),
            SenseKey::BlankCheck => write!(f, "BLANK CHECK"),
            SenseKey::VendorSpecific(v) => write!(f, "VENDOR SPECIFIC(0x{v:02X})"),
            SenseKey::Unknown(v) => write!(f, "UNKNOWN(0x{v:02X})"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SenseData {
    pub key: SenseKey,
    pub asc: u8,
    pub ascq: u8,
    pub info: [u8; 4],
    pub raw: Vec<u8>,
}

impl fmt::Display for SenseData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sense: {} ASC=0x{:02X} ASCQ=0x{:02X}", self.key, self.asc, self.ascq)
    }
}

pub fn parse_sense_data(data: &[u8]) -> Option<SenseData> {
    if data.len() < 18 {
        return None;
    }
    let response_code = data[0] & 0x7F;
    if response_code < 0x70 || response_code > 0x73 {
        return None;
    }
    let key = match data[2] & 0x0F {
        0x00 => SenseKey::NoSense,
        0x01 => SenseKey::NotReady,
        0x02 => SenseKey::NotReady,
        0x03 => SenseKey::MediumError,
        0x04 => SenseKey::HardwareError,
        0x05 => SenseKey::IllegalRequest,
        0x06 => SenseKey::UnitAttention,
        0x07 => SenseKey::DataProtect,
        0x08 => SenseKey::BlankCheck,
        0x09..=0x0B => SenseKey::VendorSpecific(data[2] & 0x0F),
        v => SenseKey::Unknown(v),
    };
    Some(SenseData {
        key,
        asc: data[12],
        ascq: data[13],
        info: [data[3], data[4], data[5], data[6]],
        raw: data.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_not_ready() {
        let data = b"\x70\x00\x02\x00\x00\x00\x00\x0a\x00\x00\x00\x00\x04\x01\x00\x00\x00\x00";
        let sense = parse_sense_data(data).unwrap();
        assert_eq!(sense.key, SenseKey::NotReady);
        assert_eq!(sense.asc, 0x04);
        assert_eq!(sense.ascq, 0x01);
    }

    #[test]
    fn parse_illegal_request() {
        let data = b"\x70\x00\x05\x00\x00\x00\x00\x0a\x00\x00\x00\x00\x20\x00\x00\x00\x00\x00";
        let sense = parse_sense_data(data).unwrap();
        assert_eq!(sense.key, SenseKey::IllegalRequest);
        assert_eq!(sense.asc, 0x20);
        assert_eq!(sense.ascq, 0x00);
    }

    #[test]
    fn parse_too_short() {
        assert!(parse_sense_data(&[0x70; 10]).is_none());
    }

    #[test]
    fn display() {
        let data = b"\x70\x00\x02\x00\x00\x00\x00\x0a\x00\x00\x00\x00\x04\x01\x00\x00\x00\x00";
        let sense = parse_sense_data(data).unwrap();
        let s = format!("{sense}");
        assert!(s.contains("NOT READY"));
    }
}
