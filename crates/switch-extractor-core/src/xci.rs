use crate::error::{ExtractorError, Result};
use crate::pfs0::{Pfs0, Pfs0Entry};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunk

#[derive(Debug, Clone)]
pub struct XciPartition {
    pub name: String,
    pub abs_offset: u64,
    pub size: u64,
    pub entries: Vec<Pfs0Entry>,
}

pub struct Xci {
    file: File,
    pub header_offset: u64,
    pub root_hfs0_offset: u64,
    pub partitions: HashMap<String, XciPartition>,
    pub package_id: u64,
    pub rom_size_id: u8,
    pub valid_data_end: u64,
}

impl Xci {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Find CardHeader HEAD magic
        let header_offset = Self::find_header(&mut file)?;

        // Read CardHeader fields at header_offset + 0x104
        file.seek(SeekFrom::Start(header_offset + 0x104))?;
        let _rom_start = file.read_u32::<LittleEndian>()?;
        let _backup_area = file.read_u32::<LittleEndian>()?;
        let _title_key_dec = file.read_u8()?;
        let rom_size_id = file.read_u8()?;
        let _card_ver = file.read_u8()?;
        let _flags = file.read_u8()?;

        let package_id = file.read_u64::<LittleEndian>()?;
        let valid_data_end_blocks = file.read_u32::<LittleEndian>()?;
        let valid_data_end = (valid_data_end_blocks as u64) * 0x200;

        // PartitionFsHeaderAddress at header_offset + 0x130
        file.seek(SeekFrom::Start(header_offset + 0x130))?;
        let pfs_header_addr = file.read_u64::<LittleEndian>()?;
        let _pfs_header_size = file.read_u64::<LittleEndian>()?;

        // Locate Root HFS0
        let candidate_offsets = [
            pfs_header_addr,
            0xF000,
            0x10000,
            0x200,
            0x1000,
            header_offset + 0x1000,
            header_offset + 0x200,
        ];

        let mut root_hfs0_offset = None;
        for &cand in &candidate_offsets {
            if file.seek(SeekFrom::Start(cand)).is_ok() {
                let mut magic = [0u8; 4];
                if file.read_exact(&mut magic).is_ok() && &magic == b"HFS0" {
                    root_hfs0_offset = Some(cand);
                    break;
                }
            }
        }

        let root_off = match root_hfs0_offset {
            Some(o) => o,
            None => {
                return Err(ExtractorError::Format(
                    "Could not locate valid Root HFS0 partition table in XCI.".to_string(),
                ));
            }
        };

        // Parse Root HFS0
        let root_pfs = Pfs0::new(&mut file, root_off)?;
        let mut partitions = HashMap::new();

        for ent in &root_pfs.entries {
            // Each entry in Root HFS0 is a sub-partition (e.g. "secure", "normal", "update")
            if let Ok(sub_pfs) = Pfs0::new(&mut file, ent.abs_offset) {
                let part = XciPartition {
                    name: ent.name.clone(),
                    abs_offset: ent.abs_offset,
                    size: ent.size,
                    entries: sub_pfs.entries,
                };
                partitions.insert(ent.name.clone(), part);
            }
        }

        Ok(Self {
            file,
            header_offset,
            root_hfs0_offset: root_off,
            partitions,
            package_id,
            rom_size_id,
            valid_data_end,
        })
    }

    fn find_header(file: &mut File) -> Result<u64> {
        let mut magic = [0u8; 4];

        // 1. Untrimmed standard: HEAD at 0x1100 (Header at 0x1000)
        file.seek(SeekFrom::Start(0x1100))?;
        if file.read_exact(&mut magic).is_ok() && &magic == b"HEAD" {
            return Ok(0x1000);
        }

        // 2. Trimmed XCI: HEAD at 0x100 (Header at 0x0)
        file.seek(SeekFrom::Start(0x100))?;
        if file.read_exact(&mut magic).is_ok() && &magic == b"HEAD" {
            return Ok(0x0);
        }

        // 3. Direct start: HEAD at 0x0
        file.seek(SeekFrom::Start(0x0))?;
        if file.read_exact(&mut magic).is_ok() && &magic == b"HEAD" {
            return Ok(0x0);
        }

        Err(ExtractorError::InvalidMagic {
            magic: String::from_utf8_lossy(&magic).to_string(),
            expected: "HEAD".to_string(),
        })
    }

    pub fn secure_partition(&self) -> Option<&XciPartition> {
        self.partitions.get("secure")
    }

    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub fn extract_secure_partition<F, C>(
        &mut self,
        out_dir: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<Vec<PathBuf>>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        let sec = match self.partitions.get("secure") {
            Some(s) => s,
            None => {
                return Err(ExtractorError::Format(
                    "XCI does not contain a secure partition.".to_string(),
                ));
            }
        };

        std::fs::create_dir_all(out_dir)?;
        let entries = sec.entries.clone();
        let total = entries.len();
        let mut extracted = Vec::with_capacity(total);

        let mut buf = vec![0u8; CHUNK_SIZE];

        for (idx, entry) in entries.iter().enumerate() {
            let out_file = out_dir.join(&entry.name);
            let mut out = File::create(&out_file)?;

            self.file.seek(SeekFrom::Start(entry.abs_offset))?;
            let mut remaining = entry.size;
            let mut written = 0u64;

            while remaining > 0 {
                if let Some(cancel) = cancel_check
                    && cancel() {
                        drop(out);
                        let _ = std::fs::remove_file(&out_file);
                        return Err(ExtractorError::Cancelled);
                    }

                let to_read = (remaining as usize).min(CHUNK_SIZE);
                self.file.read_exact(&mut buf[..to_read])?;
                out.write_all(&buf[..to_read])?;

                remaining -= to_read as u64;
                written += to_read as u64;

                if let Some(cb) = progress_cb {
                    cb(idx + 1, total, &entry.name, written, entry.size);
                }
            }

            out.flush()?;
            extracted.push(out_file);
        }

        Ok(extracted)
    }

    pub fn convert_to_nsp<F, C>(
        &mut self,
        output_nsp_path: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<u64>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        let sec = match self.partitions.get("secure") {
            Some(s) => s,
            None => {
                return Err(ExtractorError::Format(
                    "XCI does not contain a secure partition.".to_string(),
                ));
            }
        };

        if let Some(parent) = output_nsp_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let entries = sec.entries.clone();
        let file_count = entries.len();
        let entry_size = 0x18; // PFS0 entry size

        let mut string_table = Vec::new();
        let mut pfs0_entries = Vec::with_capacity(file_count);
        let mut current_data_offset = 0u64;

        for entry in &entries {
            let name_bytes = entry.name.as_bytes();
            let name_offset = string_table.len() as u32;
            string_table.extend_from_slice(name_bytes);
            string_table.push(0);

            pfs0_entries.push((current_data_offset, entry.size, name_offset));
            current_data_offset += entry.size;
        }

        let mut out_file = File::create(output_nsp_path)?;

        // Write PFS0 Header
        out_file.write_all(b"PFS0")?;
        out_file.write_u32::<LittleEndian>(file_count as u32)?;
        out_file.write_u32::<LittleEndian>(string_table.len() as u32)?;
        out_file.write_u32::<LittleEndian>(0)?; // reserved

        for (data_off, file_sz, name_off) in pfs0_entries {
            out_file.write_u64::<LittleEndian>(data_off)?;
            out_file.write_u64::<LittleEndian>(file_sz)?;
            out_file.write_u32::<LittleEndian>(name_off)?;
            out_file.write_u32::<LittleEndian>(0)?; // reserved
        }

        out_file.write_all(&string_table)?;

        let mut total_written = 0x10 + (file_count * entry_size) as u64 + string_table.len() as u64;
        let mut buf = vec![0u8; CHUNK_SIZE];

        // Stream NCAs directly from XCI secure partition to the new NSP
        for (idx, entry) in entries.iter().enumerate() {
            self.file.seek(SeekFrom::Start(entry.abs_offset))?;
            let mut remaining = entry.size;
            let mut written_file = 0u64;

            while remaining > 0 {
                if let Some(cancel) = cancel_check
                    && cancel() {
                        drop(out_file);
                        let _ = std::fs::remove_file(output_nsp_path);
                        return Err(ExtractorError::Cancelled);
                    }

                let to_read = (remaining as usize).min(CHUNK_SIZE);
                self.file.read_exact(&mut buf[..to_read])?;
                out_file.write_all(&buf[..to_read])?;

                remaining -= to_read as u64;
                written_file += to_read as u64;
                total_written += to_read as u64;

                if let Some(cb) = progress_cb {
                    cb(idx + 1, file_count, &entry.name, written_file, entry.size);
                }
            }
        }

        out_file.flush()?;
        Ok(total_written)
    }
}
