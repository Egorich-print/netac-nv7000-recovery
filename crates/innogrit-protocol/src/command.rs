use nvme_core::command::NvmeCommand;

pub const INNOGRIT_MAGIC0: u32 = 0x4F4E4E49;
pub const INNOGRIT_MAGIC1: u32 = 0x54495247;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InnogritVendorCmd {
    pub nvme: NvmeCommand,
    pub magic0: u32,
    pub magic1: u32,
    pub opcode: u32,
    pub payload_len: u32,
    pub payload: [u8; 148],
}

impl InnogritVendorCmd {
    pub fn new(opcode: InnogritOpcode) -> Self {
        let mut cmd = Self {
            nvme: NvmeCommand::new(0),
            magic0: INNOGRIT_MAGIC0,
            magic1: INNOGRIT_MAGIC1,
            opcode: opcode as u32,
            payload_len: 0,
            payload: [0u8; 148],
        };
        cmd.nvme.nsid = 0xFFFFFFFF;
        cmd
    }

    pub fn total_size(&self) -> usize {
        64 + 4 + 4 + 4 + 4 + self.payload_len as usize
    }

    pub fn as_bytes(&self) -> &[u8] {
        let size = self.total_size();
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, size) }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnogritOpcode {
    RecoveryMode = 0x00,
    FinalizeFirmware = 0x03,
    SetFirmwareConfig = 0x0B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    Unknown,
    Normal,
    Recovery,
    LoaderLoaded,
    ProgrammerLoaded,
    RuntimeLoaded,
    Configured,
    Complete,
}

impl RecoveryState {
    pub fn next(&self) -> Option<RecoveryState> {
        use RecoveryState::*;
        match self {
            Unknown => Some(Normal),
            Normal => Some(Recovery),
            Recovery => Some(LoaderLoaded),
            LoaderLoaded => Some(ProgrammerLoaded),
            ProgrammerLoaded => Some(RuntimeLoaded),
            RuntimeLoaded => Some(Configured),
            Configured => Some(Complete),
            Complete => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn innogrit_opcode_values() {
        assert_eq!(InnogritOpcode::RecoveryMode as u32, 0x00);
        assert_eq!(InnogritOpcode::FinalizeFirmware as u32, 0x03);
        assert_eq!(InnogritOpcode::SetFirmwareConfig as u32, 0x0B);
    }

    #[test]
    fn vendor_cmd_has_magic() {
        let cmd = InnogritVendorCmd::new(InnogritOpcode::RecoveryMode);
        assert_eq!(cmd.magic0, INNOGRIT_MAGIC0);
        assert_eq!(cmd.magic1, INNOGRIT_MAGIC1);
        assert_eq!(cmd.opcode, 0);
    }
}
