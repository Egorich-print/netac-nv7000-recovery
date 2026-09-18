#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NvmeCommand {
    pub cdw0: u32,
    pub nsid: u32,
    pub reserved: u64,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

#[test]
fn nvme_command_size() {
    assert_eq!(core::mem::size_of::<NvmeCommand>(), 64);
}

impl NvmeCommand {
    pub fn new(opcode: u8) -> Self {
        let mut cmd = Self::default();
        cmd.set_opcode(opcode);
        cmd.nsid = 0xFFFFFFFF;
        cmd
    }

    pub fn set_opcode(&mut self, opcode: u8) {
        self.cdw0 = (opcode as u32) << 5;
    }

    pub fn opcode(&self) -> u8 {
        ((self.cdw0 >> 5) & 0xFF) as u8
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        unsafe { &*(self as *const Self as *const [u8; 64]) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareDownloadAction {
    Download,
    Commit { slot: u8, action: FirmwareCommitAction },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareCommitAction {
    Save = 0,
    Activate = 1,
}

pub const NVME_OP_FIRMWARE_DOWNLOAD: u8 = 0x11;
pub const NVME_OP_FIRMWARE_COMMIT: u8 = 0x10;
pub const NVME_OP_IDENTIFY: u8 = 0x06;
pub const NVME_OP_GET_LOG_PAGE: u8 = 0x02;
pub const NVME_OP_SET_FEATURES: u8 = 0x08;

pub fn firmware_download(offset: u32, data_len: u32) -> NvmeCommand {
    let mut cmd = NvmeCommand::new(NVME_OP_FIRMWARE_DOWNLOAD);
    cmd.cdw10 = (data_len / 4).wrapping_sub(1);
    cmd.cdw11 = offset / 4;
    cmd
}

pub fn firmware_commit(slot: u8, action: FirmwareCommitAction) -> NvmeCommand {
    let mut cmd = NvmeCommand::new(NVME_OP_FIRMWARE_COMMIT);
    cmd.cdw10 = (slot as u32) | ((action as u32) << 3);
    cmd
}

pub fn identify_controller() -> NvmeCommand {
    let mut cmd = NvmeCommand::new(NVME_OP_IDENTIFY);
    cmd.cdw10 = 1;
    cmd
}

pub fn get_log_page(lid: u8, size: u32) -> NvmeCommand {
    let mut cmd = NvmeCommand::new(NVME_OP_GET_LOG_PAGE);
    cmd.cdw10 = (lid as u32) | ((size / 4).wrapping_sub(1)) << 16;
    cmd
}
