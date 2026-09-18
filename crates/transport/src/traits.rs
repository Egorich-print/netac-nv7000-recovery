use nvme_core::command::NvmeCommand;

#[derive(Debug)]
pub enum TransportError {
    NotFound,
    PermissionDenied,
    Io(String),
    InvalidResponse,
    Timeout,
    DeviceError(u32),
    Protocol(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::NotFound => write!(f, "device not found"),
            TransportError::PermissionDenied => write!(f, "permission denied"),
            TransportError::Io(msg) => write!(f, "I/O error: {}", msg),
            TransportError::InvalidResponse => write!(f, "invalid response"),
            TransportError::Timeout => write!(f, "timeout"),
            TransportError::DeviceError(code) => write!(f, "device error 0x{code:04X}"),
            TransportError::Protocol(msg) => write!(f, "protocol error: {}", msg),
        }
    }
}

impl std::error::Error for TransportError {}

pub trait NvmeTransport {
    fn send_admin_command(
        &mut self,
        cmd: &NvmeCommand,
        data: &[u8],
        dir: DataDirection,
    ) -> Result<Vec<u8>, TransportError>;

    fn firmware_download(&mut self, offset: u32, data: &[u8]) -> Result<(), TransportError> {
        let cmd = nvme_core::command::firmware_download(offset, data.len() as u32);
        self.send_admin_command(&cmd, data, DataDirection::ToDevice)?;
        Ok(())
    }

    fn firmware_commit(&mut self, slot: u8, action: nvme_core::command::FirmwareCommitAction) -> Result<(), TransportError> {
        let cmd = nvme_core::command::firmware_commit(slot, action);
        self.send_admin_command(&cmd, &[], DataDirection::None)?;
        Ok(())
    }

    fn identify_controller(&mut self) -> Result<nvme_core::identify::IdentifyController, TransportError> {
        let cmd = nvme_core::command::identify_controller();
        let data = self.send_admin_command(&cmd, &[], DataDirection::FromDevice)?;
        let id: nvme_core::identify::IdentifyController = unsafe {
            core::ptr::read(data.as_ptr() as *const _)
        };
        Ok(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirection {
    None,
    ToDevice,
    FromDevice,
}
