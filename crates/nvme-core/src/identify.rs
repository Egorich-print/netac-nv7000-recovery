#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IdentifyController {
    pub vid: u16,
    pub ssvid: u16,
    pub sn: [u8; 20],
    pub mn: [u8; 40],
    pub fr: [u8; 8],
    pub rab: u8,
    pub ieee: [u8; 3],
    pub cmic: u8,
    pub mdts: u8,
    pub cnlor: u16,
    pub oacs: u16,
    pub fna: u8,
    pub vwc: u8,
    pub awun: u16,
    pub awuf: u16,
    pub awupf: u16,
    pub nvscc: u8,
    pub nwpc: u8,
    pub acwu: u16,
    pub sgls: u32,
    pub mnan: u32,
    pub maxdna: u32,
    pub maxcna: u32,
    pub sqes: u8,
    pub cqes: u8,
    pub nn: u32,
    pub oncs: u16,
    pub fuses: u16,
    pub fna2: u8,
    pub icdoff: u8,
    pub _reserved: [u8; 3840],
}

impl IdentifyController {
    pub fn model_number(&self) -> &str {
        let end = self.mn.iter().position(|&b| b == 0).unwrap_or(40);
        core::str::from_utf8(&self.mn[..end]).unwrap_or("")
    }

    pub fn serial_number(&self) -> &str {
        let end = self.sn.iter().position(|&b| b == 0).unwrap_or(20);
        core::str::from_utf8(&self.sn[..end]).unwrap_or("")
    }

    pub fn firmware_rev(&self) -> &str {
        let end = self.fr.iter().position(|&b| b == 0).unwrap_or(8);
        core::str::from_utf8(&self.fr[..end]).unwrap_or("")
    }
}
