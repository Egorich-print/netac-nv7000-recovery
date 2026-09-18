// Direct NVMe transport via Linux /dev/ng* or /dev/nvme*.
//
// Uses the Linux NVMe ioctl interface (NVME_IOCTL_ADMIN_CMD).
//
// Status: skeleton, requires Linux NVMe headers.

use crate::traits::{DataDirection, NvmeTransport, TransportError};
use nvme_core::command::NvmeCommand;

pub struct LinuxNvmeTransport {
    path: String,
    fd: Option<i32>,
}

impl LinuxNvmeTransport {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            fd: None,
        }
    }

    pub fn open(&mut self) -> Result<(), TransportError> {
        // TODO: open /dev/ng0 or /dev/nvme0 via std::fs::File or raw fd
        Err(TransportError::NotFound)
    }
}

impl NvmeTransport for LinuxNvmeTransport {
    fn send_admin_command(
        &mut self,
        _cmd: &NvmeCommand,
        _data: &[u8],
        _dir: DataDirection,
    ) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::Protocol("Linux NVMe not implemented".into()))
    }
}
