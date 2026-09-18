use crate::scsi::{ScsiDirection, ScsiError, ScsiTransport};
use nvme_core::command::NvmeCommand;

const NVME_MAGIC: u32 = 0x454D564E; // "NVME" little-endian

// JMS583 0xA1 CDB flags
const JMS583_FLAG_WRITE: u8 = 0x81;
const JMS583_FLAG_READ: u8 = 0x82;
const JMS583_FLAG_STATUS: u8 = 0x83;

#[derive(Debug, Clone, Copy)]
pub enum TunnelDirection {
    FromDevice,
    ToDevice,
    None,
}

#[derive(Debug)]
pub enum TunnelError {
    Transport(String),
    InvalidResponse,
    NvmeError(u32),
}

impl From<ScsiError> for TunnelError {
    fn from(e: ScsiError) -> Self {
        match e {
            ScsiError::Transport(msg) => TunnelError::Transport(msg),
            ScsiError::InvalidResponse => TunnelError::InvalidResponse,
            ScsiError::CheckCondition { .. } => TunnelError::Transport("SCSI check condition".into()),
            _ => TunnelError::Transport(format!("{e}")),
        }
    }
}

pub trait NvmeTunnel {
    fn nvme_admin(&mut self, cmd: &NvmeCommand, data: &mut [u8], dir: TunnelDirection) -> Result<(), TunnelError>;
}

pub struct Jms583NvmeTunnel<T: ScsiTransport> {
    transport: T,
}

impl<T: ScsiTransport> Jms583NvmeTunnel<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: ScsiTransport> NvmeTunnel for Jms583NvmeTunnel<T> {
    fn nvme_admin(&mut self, cmd: &NvmeCommand, data: &mut [u8], dir: TunnelDirection) -> Result<(), TunnelError> {
        let (flags, scsi_dir, buffer_len) = match dir {
            TunnelDirection::ToDevice => (JMS583_FLAG_WRITE, ScsiDirection::Out, data.len()),
            TunnelDirection::FromDevice => (JMS583_FLAG_READ, ScsiDirection::In, data.len()),
            TunnelDirection::None => (JMS583_FLAG_STATUS, ScsiDirection::None, 0),
        };

        let tfr_len = (data.len() as u16).to_be_bytes();

        // Build 0xA1 CDB
        let mut cdb = [0u8; 16];
        cdb[0] = 0xA1;
        cdb[1] = flags;
        cdb[2] = 0x00;
        cdb[3] = tfr_len[0];
        cdb[4] = tfr_len[1];
        cdb[5] = 0x00;
        // bytes 6-15 are reserved

        // Pack "NVME" + SQE into the data buffer (for OUT) or just provide empty buffer (for IN)
        // For IN: data is allocated by caller, we read into it
        // For OUT: data contains "NVME" + SQE + payload, we send it
        let data_out = match dir {
            TunnelDirection::ToDevice => {
                let mut buf = Vec::with_capacity(data.len() + 68);
                buf.extend_from_slice(&NVME_MAGIC.to_le_bytes());
                let sqe_bytes: &[u8; 64] = &cmd.as_bytes();
                buf.extend_from_slice(sqe_bytes);
                buf.extend_from_slice(data);
                buf
            }
            _ => Vec::new(),
        };

        let result = self.transport.execute(&cdb, scsi_dir, buffer_len, &data_out)?;

        // For IN: copy received data back to caller
        if matches!(dir, TunnelDirection::FromDevice) {
            let copy_len = data.len().min(result.data.len());
            data[..copy_len].copy_from_slice(&result.data[..copy_len]);
        }

        Ok(())
    }
}
