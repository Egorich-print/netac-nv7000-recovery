use crate::command::{InnogritOpcode, InnogritVendorCmd, RecoveryState};
use transport::traits::{DataDirection, NvmeTransport, TransportError};

pub struct FirmwareImage {
    pub data: Vec<u8>,
    pub kind: FirmwareKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareKind {
    Loader,
    Programmer,
    Runtime,
}

pub struct FirmwarePipeline<T: NvmeTransport> {
    pub transport: T,
    pub state: RecoveryState,
}

impl<T: NvmeTransport> FirmwarePipeline<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            state: RecoveryState::Unknown,
        }
    }

    pub fn enter_recovery(&mut self) -> Result<(), PipelineError> {
        let cmd = InnogritVendorCmd::new(InnogritOpcode::RecoveryMode);
        self.transport
            .send_admin_command(&cmd.nvme, cmd.as_bytes(), DataDirection::ToDevice)?;
        self.state = RecoveryState::Recovery;
        Ok(())
    }

    pub fn flash_loader(&mut self, image: &[u8]) -> Result<(), PipelineError> {
        self.flash_chunks(image)?;
        self.commit()?;
        self.state = RecoveryState::LoaderLoaded;
        Ok(())
    }

    pub fn flash_programmer(&mut self, image: &[u8]) -> Result<(), PipelineError> {
        self.flash_chunks(image)?;
        self.commit()?;
        self.state = RecoveryState::ProgrammerLoaded;
        Ok(())
    }

    pub fn flash_runtime(&mut self, image: &[u8]) -> Result<(), PipelineError> {
        self.flash_chunks(image)?;
        self.commit()?;
        self.state = RecoveryState::RuntimeLoaded;
        Ok(())
    }

    pub fn set_config(&mut self, _config: &[u8]) -> Result<(), PipelineError> {
        let cmd = InnogritVendorCmd::new(InnogritOpcode::SetFirmwareConfig);
        self.transport
            .send_admin_command(&cmd.nvme, cmd.as_bytes(), DataDirection::ToDevice)?;
        self.state = RecoveryState::Configured;
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<(), PipelineError> {
        let cmd = InnogritVendorCmd::new(InnogritOpcode::FinalizeFirmware);
        self.transport
            .send_admin_command(&cmd.nvme, cmd.as_bytes(), DataDirection::ToDevice)?;
        self.state = RecoveryState::Complete;
        Ok(())
    }

    fn flash_chunks(&mut self, image: &[u8]) -> Result<(), PipelineError> {
        let chunk_size = 4096u32;
        let mut offset = 0u32;

        while offset < image.len() as u32 {
            let end = (offset + chunk_size).min(image.len() as u32);
            let chunk = &image[offset as usize..end as usize];

            self.transport.firmware_download(offset, chunk)?;
            offset = end;
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<(), PipelineError> {
        self.transport.firmware_commit(0, nvme_core::command::FirmwareCommitAction::Activate)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum PipelineError {
    Transport(TransportError),
    InvalidState(RecoveryState),
    FirmwareFile(String),
}

impl From<TransportError> for PipelineError {
    fn from(e: TransportError) -> Self {
        PipelineError::Transport(e)
    }
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Transport(e) => write!(f, "transport: {e}"),
            PipelineError::InvalidState(s) => write!(f, "invalid state: {s:?}"),
            PipelineError::FirmwareFile(s) => write!(f, "firmware: {s}"),
        }
    }
}

impl std::error::Error for PipelineError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grit::GritImage;
    use crate::command::{INNOGRIT_MAGIC0, INNOGRIT_MAGIC1};
    use nvme_core::command::{NVME_OP_FIRMWARE_DOWNLOAD, NVME_OP_FIRMWARE_COMMIT};
    use transport::mock::MockTransport;

    fn build_grit_with_data(section_name: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let entry_size = 16usize;
        let header_size = 16usize;
        let offset = (header_size + entry_size) as u32;

        let mut buf = Vec::new();
        buf.extend_from_slice(b"GRIT");
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());

        let table_start = buf.len();
        buf.extend_from_slice(section_name);
        buf.extend_from_slice(&offset.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let table_end = buf.len();

        let csum = crc32fast::hash(&buf[table_start..table_end]);
        buf[12..16].copy_from_slice(&csum.to_le_bytes());
        buf.extend_from_slice(data);
        buf
    }

    const NVME_CMD_SIZE: usize = 64;

    fn parse_innogrit_data(data: &[u8]) -> Option<(u32, u32)> {
        let inp = data.get(NVME_CMD_SIZE..)?;
        if inp.len() < 12 { return None; }
        let magic0 = u32::from_le_bytes(inp[0..4].try_into().ok()?);
        let magic1 = u32::from_le_bytes(inp[4..8].try_into().ok()?);
        if magic0 != INNOGRIT_MAGIC0 || magic1 != INNOGRIT_MAGIC1 {
            return None;
        }
        let opcode = u32::from_le_bytes(inp[8..12].try_into().ok()?);
        let payload_len = if inp.len() >= 16 {
            u32::from_le_bytes(inp[12..16].try_into().ok()?)
        } else { 0 };
        Some((opcode, payload_len))
    }

    #[test]
    fn pipeline_full_sequence() {
        let payload = vec![0xABu8; 128];
        let grit = build_grit_with_data(b"data", &payload);
        let image = GritImage::parse(&grit).unwrap();
        let section_data = image.get_section_data("data").unwrap().to_vec();

        let transport = MockTransport::new();
        let mut pipeline = FirmwarePipeline::new(transport);

        pipeline.enter_recovery().unwrap();
        pipeline.flash_loader(&section_data).unwrap();
        pipeline.set_config(&[]).unwrap();
        pipeline.finalize().unwrap();

        let cmds = &pipeline.transport.commands;
        assert!(cmds.len() >= 5, "expected at least 5 commands, got {}", cmds.len());

        // 1. Recovery command (Innogrit opcode 0)
        let (recv_opcode, _) = parse_innogrit_data(&cmds[0].1).expect("should be Innogrit cmd");
        assert_eq!(recv_opcode, 0x00, "recovery opcode");

        // 2..N. FW Download commands (opcode 0x11)
        let fw_dl_count = cmds.iter()
            .skip(1)
            .take_while(|(cmd, _, _)| cmd.opcode() == NVME_OP_FIRMWARE_DOWNLOAD)
            .count();
        assert!(fw_dl_count > 0, "expected FW Download commands");
        let fw_commit_count = cmds.iter()
            .filter(|(cmd, _, _)| cmd.opcode() == NVME_OP_FIRMWARE_COMMIT)
            .count();
        assert_eq!(fw_commit_count, 1, "exactly one FW Commit");

        // Next-to-last: Config (Innogrit opcode 0x0B)
        let config_entry = cmds.iter().rev().nth(1).unwrap();
        let (cfg_opcode, _) = parse_innogrit_data(&config_entry.1).expect("should be Innogrit cmd");
        assert_eq!(cfg_opcode, 0x0B, "config opcode");

        // Last: Finalize (Innogrit opcode 0x03)
        let last = cmds.last().unwrap();
        let (fin_opcode, _) = parse_innogrit_data(&last.1).expect("should be Innogrit cmd");
        assert_eq!(fin_opcode, 0x03, "finalize opcode");
    }

    #[test]
    fn pipeline_state_transitions() {
        let payload = vec![0x00u8; 64];
        let grit = build_grit_with_data(b"data", &payload);
        let image = GritImage::parse(&grit).unwrap();
        let section_data = image.get_section_data("data").unwrap().to_vec();

        let transport = MockTransport::new();
        let mut pipeline = FirmwarePipeline::new(transport);

        assert_eq!(pipeline.state, RecoveryState::Unknown);
        pipeline.enter_recovery().unwrap();
        assert_eq!(pipeline.state, RecoveryState::Recovery);
        pipeline.flash_loader(&section_data).unwrap();
        assert_eq!(pipeline.state, RecoveryState::LoaderLoaded);
        pipeline.set_config(&[]).unwrap();
        assert_eq!(pipeline.state, RecoveryState::Configured);
        pipeline.finalize().unwrap();
        assert_eq!(pipeline.state, RecoveryState::Complete);
    }

    #[test]
    fn pipeline_transport_error_propagation() {
        let transport = MockTransport::new();
        let mut pipeline = FirmwarePipeline::new(transport);
        pipeline.transport.responses.push(Err(transport::TransportError::Timeout));

        let result = pipeline.enter_recovery();
        assert!(result.is_err());
        match result {
            Err(PipelineError::Transport(transport::TransportError::Timeout)) => {}
            _ => panic!("expected TransportError::Timeout"),
        }
    }
}
