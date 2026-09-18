const COMMAND_IU_ID: u8 = 0x01;
const STATUS_IU_ID: u8 = 0x05;
const STATUS_GOOD: u8 = 0x00;
const STATUS_CHECK_CONDITION: u8 = 0x02;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommandIU {
    pub iu_id: u8,
    pub rsvd: u8,
    pub tag: u16,
    pub rsvd2: u8,
    pub additional_len: u8,
    pub rsvd3: u16,
    pub cdb: [u8; 16],
}

impl CommandIU {
    pub fn new(tag: u16, lun: u8, cdb: &[u8]) -> Self {
        let mut cmd = Self {
            iu_id: COMMAND_IU_ID,
            rsvd: 0,
            tag,
            rsvd2: 0,
            additional_len: 0,
            rsvd3: 0,
            cdb: [0u8; 16],
        };
        cmd.rsvd2 = lun;
        let copy_len = cdb.len().min(16);
        cmd.cdb[..copy_len].copy_from_slice(&cdb[..copy_len]);
        cmd
    }

    pub fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, size) }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StatusIU {
    pub iu_id: u8,
    pub rsvd: u8,
    pub tag: u16,
    pub status: u8,
    pub rsvd2: [u8; 3],
    pub sense: [u8; 18],
}

impl StatusIU {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < core::mem::size_of::<Self>() {
            return None;
        }
        if data[0] != STATUS_IU_ID {
            return None;
        }
        let mut iu = Self {
            iu_id: STATUS_IU_ID,
            rsvd: 0,
            tag: 0,
            status: 0,
            rsvd2: [0; 3],
            sense: [0; 18],
        };
        let src = &data[..core::mem::size_of::<Self>()];
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                &mut iu as *mut _ as *mut u8,
                core::mem::size_of::<Self>(),
            );
        }
        Some(iu)
    }

    pub const fn size() -> usize {
        core::mem::size_of::<Self>()
    }

    pub fn is_good(&self) -> bool {
        self.status == STATUS_GOOD
    }

    pub fn is_check_condition(&self) -> bool {
        self.status == STATUS_CHECK_CONDITION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_iu_size() {
        assert_eq!(core::mem::size_of::<CommandIU>(), 24);
    }

    #[test]
    fn command_iu_new() {
        let cdb = [0x12u8, 0x00, 0x00, 0x00, 0x60, 0x00];
        let iu = CommandIU::new(0x0001, 0, &cdb);
        assert_eq!(iu.iu_id, 0x01);
        assert_eq!(iu.tag, 0x0001);
        assert_eq!(iu.cdb[..6], cdb);
    }

    #[test]
    fn command_iu_serialize() {
        let iu = CommandIU::new(0xABCD, 0, &[0x12u8, 0x00]);
        let bytes = iu.as_bytes();
        assert_eq!(bytes.len(), 24);
        assert_eq!(bytes[0], 0x01);
        assert_eq!(bytes[2], 0xCD);
        assert_eq!(bytes[3], 0xAB);
        assert_eq!(bytes[8], 0x12);
    }

    #[test]
    fn status_iu_size() {
        assert_eq!(StatusIU::size(), 26);
    }

    #[test]
    fn status_iu_parse_good() {
        let mut raw = vec![0u8; 26];
        raw[0] = 0x05;
        raw[4] = 0x00;
        let iu = StatusIU::parse(&raw).unwrap();
        assert_eq!(iu.iu_id, 0x05);
        assert!(iu.is_good());
        assert!(!iu.is_check_condition());
    }

    #[test]
    fn status_iu_parse_check_condition() {
        let mut raw = vec![0u8; 26];
        raw[0] = 0x05;
        raw[4] = 0x02;
        let iu = StatusIU::parse(&raw).unwrap();
        assert!(iu.is_check_condition());
        assert!(!iu.is_good());
    }

    #[test]
    fn status_iu_parse_short() {
        let raw = vec![0u8; 4];
        let iu = StatusIU::parse(&raw);
        assert!(iu.is_none());
    }

    #[test]
    fn status_iu_parse_wrong_id() {
        let raw = vec![0x01u8; 26];
        let iu = StatusIU::parse(&raw);
        assert!(iu.is_none());
    }
}
