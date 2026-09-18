#[derive(Debug, Clone)]
pub struct NandConfig {
    pub nand_major_id: u8,
    pub nand_minor_id: u8,
    pub disk_size_gb: u32,
    pub sn_mask: String,
    pub pmic_type: PmicType,
    pub nand_work_mode: NandWorkMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmicType {
    None,
    Sy21523E,
    Mps5417,
    Mp8854Gl,
    Act88321,
    Act85610,
    Act88430,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NandWorkMode {
    DynamicSLC,
    PureXLC,
}

#[derive(Debug, Clone)]
pub enum FlashVendor {
    YmtcX1,
    YmtcX2,
    MicronB47R,
    Hynix3DV6,
    Hynix3DV7,
    SandiskBiCS3,
    SandiskBiCS4,
    SandiskBiCS5,
    IntelB17A,
    Samsung,
    Toshiba,
}

pub struct ConfigBuilder {
    config: NandConfig,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: NandConfig {
                nand_major_id: 0,
                nand_minor_id: 0,
                disk_size_gb: 2048,
                sn_mask: String::new(),
                pmic_type: PmicType::None,
                nand_work_mode: NandWorkMode::PureXLC,
            },
        }
    }

    pub fn nand_major_id(mut self, id: u8) -> Self {
        self.config.nand_major_id = id;
        self
    }

    pub fn nand_minor_id(mut self, id: u8) -> Self {
        self.config.nand_minor_id = id;
        self
    }

    pub fn disk_size_gb(mut self, gb: u32) -> Self {
        self.config.disk_size_gb = gb;
        self
    }

    pub fn sn_mask(mut self, mask: &str) -> Self {
        self.config.sn_mask = mask.to_string();
        self
    }

    pub fn build(self) -> NandConfig {
        self.config
    }
}

pub fn ymtc_x2_9060_config() -> NandConfig {
    ConfigBuilder::new()
        .nand_major_id(0x9C)
        .nand_minor_id(0xFE)
        .disk_size_gb(2048)
        .sn_mask("RN######")
        .build()
}
