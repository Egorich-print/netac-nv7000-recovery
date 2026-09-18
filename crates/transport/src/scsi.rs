use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScsiDirection {
    None,
    In,
    Out,
}

#[derive(Debug, Clone)]
pub struct ScsiResult {
    pub status: u8,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum ScsiError {
    Transport(String),
    InvalidResponse,
    CheckCondition { sense: Vec<u8> },
    Timeout,
    DeviceNotResponding,
    NotConnected,
}

impl fmt::Display for ScsiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScsiError::Transport(msg) => write!(f, "transport: {msg}"),
            ScsiError::InvalidResponse => write!(f, "invalid response"),
            ScsiError::CheckCondition { .. } => write!(f, "check condition"),
            ScsiError::Timeout => write!(f, "timeout"),
            ScsiError::DeviceNotResponding => write!(f, "device not responding"),
            ScsiError::NotConnected => write!(f, "not connected"),
        }
    }
}

impl std::error::Error for ScsiError {}

pub trait ScsiTransport {
    fn execute(
        &mut self,
        cdb: &[u8],
        direction: ScsiDirection,
        buffer_len: usize,
        data_out: &[u8],
    ) -> Result<ScsiResult, ScsiError>;
}

#[allow(dead_code)]
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
// Mock SCSI transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MockScsiTransport {
    pub commands: Vec<(Vec<u8>, ScsiDirection, usize)>,
    pub fixed_response: Option<Vec<u8>>,
}

impl MockScsiTransport {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            fixed_response: None,
        }
    }

    pub fn with_inquiry_response() -> Self {
        let mut data = vec![0u8; 96];
        data[0] = 0x00;
        let vendor = b"JMicron ";
        let product = b"Tech Media      ";
        let rev = b"0208";
        data[8..16].copy_from_slice(vendor);
        data[16..32].copy_from_slice(product);
        data[32..36].copy_from_slice(rev);
        Self {
            commands: Vec::new(),
            fixed_response: Some(data),
        }
    }
}

impl ScsiTransport for MockScsiTransport {
    fn execute(
        &mut self,
        cdb: &[u8],
        direction: ScsiDirection,
        buffer_len: usize,
        _data_out: &[u8],
    ) -> Result<ScsiResult, ScsiError> {
        self.commands.push((cdb.to_vec(), direction, buffer_len));
        if let Some(ref data) = self.fixed_response {
            let len = buffer_len.min(data.len());
            Ok(ScsiResult {
                status: 0x00,
                data: data[..len].to_vec(),
            })
        } else {
            Ok(ScsiResult {
                status: 0x00,
                data: vec![0u8; buffer_len],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scsi_commands::inquiry;

    #[test]
    fn test_inquiry_mock() {
        let mut mock = MockScsiTransport::with_inquiry_response();
        let result = inquiry(&mut mock).unwrap();
        assert_eq!(result.vendor, "JMicron");
        assert_eq!(result.product, "Tech Media");
        assert_eq!(result.revision, "0208");
    }
}
