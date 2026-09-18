use crate::scsi::{ScsiDirection, ScsiError, ScsiTransport};
use crate::sense::{parse_sense_data, SenseData};
use std::fmt;

// ---------------------------------------------------------------------------
// INQUIRY
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InquiryData {
    pub peripheral_type: u8,
    pub vendor: String,
    pub product: String,
    pub revision: String,
}

impl fmt::Display for InquiryData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SCSI Inquiry")?;
        writeln!(f, "  Peripheral Type: 0x{:02X}", self.peripheral_type)?;
        writeln!(f, "  Vendor:          {}", self.vendor)?;
        writeln!(f, "  Product:         {}", self.product)?;
        writeln!(f, "  Revision:        {}", self.revision)
    }
}

pub fn inquiry<T: ScsiTransport>(transport: &mut T) -> Result<InquiryData, ScsiError> {
    let cdb = [0x12, 0x00, 0x00, 0x00, 0x60, 0x00];
    let result = transport.execute(&cdb, ScsiDirection::In, 96, &[])?;
    parse_inquiry_response(&result.data).ok_or(ScsiError::InvalidResponse)
}

fn parse_inquiry_response(data: &[u8]) -> Option<InquiryData> {
    if data.len() < 36 {
        return None;
    }
    Some(InquiryData {
        peripheral_type: data[0] & 0x1F,
        vendor: ascii_string(&data[8..16]),
        product: ascii_string(&data[16..32]),
        revision: ascii_string(&data[32..36]),
    })
}

fn ascii_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0 && b >= 0x20 && b < 0x7F)
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// TEST UNIT READY
// ---------------------------------------------------------------------------

pub fn test_unit_ready<T: ScsiTransport>(transport: &mut T) -> Result<(), ScsiError> {
    let cdb = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    transport.execute(&cdb, ScsiDirection::None, 0, &[])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// READ CAPACITY (10)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Capacity {
    pub last_lba: u32,
    pub block_size: u32,
}

impl fmt::Display for Capacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size_bytes = (self.last_lba as u64 + 1) * self.block_size as u64;
        writeln!(f, "Read Capacity (10)")?;
        writeln!(f, "  Last LBA:    {}", self.last_lba)?;
        writeln!(f, "  Block Size:  {} bytes", self.block_size)?;
        write!(
            f,
            "  Total Size:  {} bytes ({} GB)",
            size_bytes,
            size_bytes / 1_000_000_000
        )
    }
}

pub fn read_capacity<T: ScsiTransport>(transport: &mut T) -> Result<Capacity, ScsiError> {
    let cdb = [0x25, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let result = transport.execute(&cdb, ScsiDirection::In, 8, &[])?;
    Capacity::parse(&result.data)
}

impl Capacity {
    pub fn parse(data: &[u8]) -> Result<Self, ScsiError> {
        if data.len() < 8 {
            return Err(ScsiError::InvalidResponse);
        }
        let last_lba = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let block_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        Ok(Capacity { last_lba, block_size })
    }
}

// ---------------------------------------------------------------------------
// REQUEST SENSE
// ---------------------------------------------------------------------------

pub fn request_sense<T: ScsiTransport>(transport: &mut T) -> Result<SenseData, ScsiError> {
    let cdb = [0x03, 0x00, 0x00, 0x00, 0x12, 0x00];
    let result = transport.execute(&cdb, ScsiDirection::In, 18, &[])?;
    parse_sense_data(&result.data).ok_or(ScsiError::InvalidResponse)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scsi::MockScsiTransport;

    #[test]
    fn test_inquiry() {
        let mut mock = MockScsiTransport::with_inquiry_response();
        let result = inquiry(&mut mock).unwrap();
        assert_eq!(result.vendor, "JMicron");
        assert_eq!(result.product, "Tech Media");
        assert_eq!(result.revision, "0208");
    }

    #[test]
    fn test_tur() {
        let mut mock = MockScsiTransport::new();
        test_unit_ready(&mut mock).unwrap();
        assert_eq!(mock.commands.len(), 1);
        assert_eq!(
            &mock.commands[0].0[..6],
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_capacity() {
        let mut mock = MockScsiTransport::new();
        mock.fixed_response = Some(vec![0x00, 0xE8, 0xE0, 0x0F, 0x00, 0x00, 0x02, 0x00]);
        let cap = read_capacity(&mut mock).unwrap();
        assert_eq!(cap.last_lba, 0x00E8E00F);
        assert_eq!(cap.block_size, 512);
    }

    #[test]
    fn test_capacity_parse_short() {
        assert!(Capacity::parse(&[0; 4]).is_err());
    }

    #[test]
    fn test_request_sense() {
        let mut mock = MockScsiTransport::new();
        mock.fixed_response = Some(vec![
            0x70, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x04, 0x01,
            0x00, 0x00, 0x00, 0x00,
        ]);
        let sense = request_sense(&mut mock).unwrap();
        assert_eq!(sense.asc, 0x04);
        assert_eq!(sense.ascq, 0x01);
    }
}
