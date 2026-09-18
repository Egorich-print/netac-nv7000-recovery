use std::fmt;

const GRIT_MAGIC: &[u8; 4] = b"GRIT";
const HEADER_SIZE: usize = 16;
const ENTRY_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct GritImage {
    header: GritHeader,
    sections: Vec<GritSection>,
    raw: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct GritHeader {
    pub header_flags: u32,
    pub section_count: u16,
    pub entry_count: u16,
    pub checksum: u32,
}

#[derive(Debug, Clone)]
pub struct GritSection {
    pub name: String,
    pub file_offset: u32,
    pub size: u32,
    pub flags: u32,
}

#[derive(Debug)]
pub enum GritError {
    BadMagic,
    Truncated,
    ChecksumMismatch { stored: u32, computed: u32 },
    SectionNotFound(String),
}

impl fmt::Display for GritError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GritError::BadMagic => write!(f, "not a GRIT image (bad magic)"),
            GritError::Truncated => write!(f, "truncated GRIT image"),
            GritError::ChecksumMismatch { stored, computed } => {
                write!(f, "checksum mismatch: stored=0x{stored:08X} computed=0x{computed:08X}")
            }
            GritError::SectionNotFound(name) => write!(f, "section \"{name}\" not found"),
        }
    }
}

impl std::error::Error for GritError {}

impl GritImage {
    pub fn parse(data: &[u8]) -> Result<Self, GritError> {
        if data.len() < HEADER_SIZE {
            return Err(GritError::Truncated);
        }
        if &data[0..4] != GRIT_MAGIC {
            return Err(GritError::BadMagic);
        }

        let header_flags = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let section_count = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let entry_count = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let stored_checksum = u32::from_le_bytes(data[12..16].try_into().unwrap());

        let table_size = section_count as usize * ENTRY_SIZE;
        let table_end = HEADER_SIZE + table_size;
        if data.len() < table_end {
            return Err(GritError::Truncated);
        }

        let computed = crc32fast::hash(&data[HEADER_SIZE..table_end]);
        if computed != stored_checksum {
            return Err(GritError::ChecksumMismatch {
                stored: stored_checksum,
                computed,
            });
        }

        let mut sections = Vec::with_capacity(section_count as usize);
        for i in 0..section_count as usize {
            let entry_off = HEADER_SIZE + i * ENTRY_SIZE;
            let name_raw = &data[entry_off..entry_off + 4];
            let name = String::from_utf8_lossy(name_raw).trim_end_matches('\0').to_string();
            let file_offset = u32::from_le_bytes(data[entry_off + 4..entry_off + 8].try_into().unwrap());
            let size = u32::from_le_bytes(data[entry_off + 8..entry_off + 12].try_into().unwrap());
            let flags = u32::from_le_bytes(data[entry_off + 12..entry_off + 16].try_into().unwrap());

            let end = file_offset as usize + size as usize;
            if end > data.len() {
                return Err(GritError::Truncated);
            }

            sections.push(GritSection {
                name,
                file_offset,
                size,
                flags,
            });
        }

        Ok(GritImage {
            header: GritHeader {
                header_flags,
                section_count,
                entry_count,
                checksum: stored_checksum,
            },
            sections,
            raw: data.to_vec(),
        })
    }

    pub fn header(&self) -> &GritHeader {
        &self.header
    }

    pub fn sections(&self) -> &[GritSection] {
        &self.sections
    }

    pub fn get_section_data(&self, name: &str) -> Result<&[u8], GritError> {
        self.sections
            .iter()
            .find(|s| s.name == name)
            .map(|s| &self.raw[s.file_offset as usize..][..s.size as usize])
            .ok_or_else(|| GritError::SectionNotFound(name.to_string()))
    }

    pub fn verify_checksum(&self) -> bool {
        let table_size = self.header.section_count as usize * ENTRY_SIZE;
        let table_end = HEADER_SIZE + table_size;
        let computed = crc32fast::hash(&self.raw[HEADER_SIZE..table_end]);
        computed == self.header.checksum
    }
}

impl fmt::Display for GritImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "GRIT Image")?;
        writeln!(f)?;
        writeln!(f, "  header_flags:  0x{:08X}", self.header.header_flags)?;
        writeln!(f, "  section_count: {}", self.header.section_count)?;
        writeln!(f, "  entry_count:   {}", self.header.entry_count)?;
        writeln!(f, "  checksum:      0x{:08X} {}", self.header.checksum,
                 if self.verify_checksum() { "✓" } else { "✗" })?;
        writeln!(f)?;
        writeln!(f, "  Sections ({}):", self.sections.len())?;
        writeln!(f, "  {}", "-".repeat(68))?;
        writeln!(f, "  {:<8} {:>10} {:>12} {:>10} {:>12}", "Name", "Offset", "Size", "Flags", "End")?;
        writeln!(f, "  {}", "-".repeat(68))?;
        for s in &self.sections {
            let end = s.file_offset + s.size;
            writeln!(
                f,
                "  {:<8} 0x{:08X} {:>8} bytes 0x{:08X} 0x{:08X}",
                s.name, s.file_offset, s.size, s.flags, end
            )?;
        }
        writeln!(f, "  {}", "-".repeat(68))?;
        let total_data: u32 = self.sections.iter().map(|s| s.size).sum();
        writeln!(f, "  Total data: {} bytes", total_data)?;
        writeln!(f, "  File size:  {} bytes", self.raw.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_grit(sections: &[(&[u8; 4], u32, u32)]) -> Vec<u8> {
        let count = sections.len() as u16;
        let mut data = Vec::new();
        data.extend_from_slice(GRIT_MAGIC);
        data.extend_from_slice(&0x00000001u32.to_le_bytes());
        data.extend_from_slice(&count.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let table_start = data.len();
        for (name, offset, size) in sections {
            data.extend_from_slice(*name);
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&size.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        let table_end = data.len();
        let csum = crc32fast::hash(&data[table_start..table_end]);
        data[12..16].copy_from_slice(&csum.to_le_bytes());

        for &(_, offset, size) in sections {
            let pos = data.len();
            if pos < offset as usize {
                data.resize(offset as usize, 0);
            }
            data.extend(std::iter::repeat(0xCC).take(size as usize));
        }

        data
    }

    #[test]
    fn parse_valid_grit() {
        let sections = [
            (b"data", 0x30, 2560),
            (b"loer", 0xA30, 4096),
        ];
        let grit = build_grit(&sections);
        let image = GritImage::parse(&grit).unwrap();
        assert_eq!(image.header.section_count, 2);
        assert_eq!(image.sections().len(), 2);
        assert_eq!(image.sections()[0].name, "data");
        assert_eq!(image.sections()[1].name, "loer");
        assert!(image.verify_checksum());
    }

    #[test]
    fn parse_bad_magic() {
        let data = b"XXXX\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let err = GritImage::parse(data).unwrap_err();
        assert!(matches!(err, GritError::BadMagic));
    }

    #[test]
    fn parse_truncated_header() {
        let data = b"GRIT\x00";
        let err = GritImage::parse(data).unwrap_err();
        assert!(matches!(err, GritError::Truncated));
    }

    #[test]
    fn parse_corrupted_checksum() {
        let sections = [
            (b"ATCM", 0x90, 1024),
            (b"BTCM", 0x490, 512),
        ];
        let mut grit = build_grit(&sections);
        grit[12] ^= 0xFF;
        let err = GritImage::parse(&grit).unwrap_err();
        assert!(matches!(err, GritError::ChecksumMismatch { .. }));
    }

    #[test]
    fn get_section_data() {
        let sections = [
            (b"data", 0x30, 64),
            (b"code", 0x70, 128),
        ];
        let grit = build_grit(&sections);
        let image = GritImage::parse(&grit).unwrap();

        let data_sec = image.get_section_data("data").unwrap();
        assert_eq!(data_sec.len(), 64);
        assert!(data_sec.iter().all(|&b| b == 0xCC));

        let code_sec = image.get_section_data("code").unwrap();
        assert_eq!(code_sec.len(), 128);
    }

    #[test]
    fn section_not_found() {
        let sections = [(b"data", 0x30, 64)];
        let grit = build_grit(&sections);
        let image = GritImage::parse(&grit).unwrap();
        let err = image.get_section_data("nonexistent").unwrap_err();
        assert!(matches!(err, GritError::SectionNotFound(_)));
    }

    #[test]
    fn truncated_section_data() {
        let sections = [(b"data", 0x30, 4096)];
        let grit = build_grit(&sections);
        let truncated = &grit[..grit.len() - 100];
        let err = GritImage::parse(truncated).unwrap_err();
        assert!(matches!(err, GritError::Truncated));
    }

    #[test]
    fn parse_loader_like() {
        let sections = [
            (b"data", 0x30, 2560),
            (b"loer", 0xA30, 10973),
        ];
        let grit = build_grit(&sections);
        let image = GritImage::parse(&grit).unwrap();
        assert_eq!(image.header.section_count, 2);
        assert_eq!(image.sections()[0].size, 2560);
        assert_eq!(image.sections()[1].size, 10973);
    }

    #[test]
    fn display_roundtrip() {
        let sections = [(b"test", 0x30, 128)];
        let grit = build_grit(&sections);
        let image = GritImage::parse(&grit).unwrap();
        let display = format!("{}", image);
        assert!(display.contains("GRIT Image"));
        assert!(display.contains("test"));
        assert!(display.contains("checksum"));
    }
}
