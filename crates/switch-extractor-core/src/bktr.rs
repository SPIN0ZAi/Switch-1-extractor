use crate::error::{ExtractorError, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BktrRelocationEntry {
    pub virt_offset: u64,
    pub phys_offset: u64,
    pub is_patch: bool,
}

#[derive(Debug, Clone)]
pub struct BktrRelocationBucket {
    pub virtual_offset_end: u64,
    pub entries: Vec<BktrRelocationEntry>,
}

#[derive(Debug, Clone)]
pub struct BktrRelocationBlock {
    pub total_size: u64,
    pub bucket_virtual_offsets: Vec<u64>,
    pub buckets: Vec<BktrRelocationBucket>,
}

impl BktrRelocationBlock {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 0x4000 {
            return Err(ExtractorError::Format(
                "BKTR relocation block header is too small".to_string(),
            ));
        }

        let mut r = Cursor::new(&data[..0x4000]);
        let _ = r.read_u32::<LittleEndian>()?;
        let num_buckets = r.read_u32::<LittleEndian>()? as usize;
        let total_size = r.read_u64::<LittleEndian>()?;

        let mut bucket_virtual_offsets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            bucket_virtual_offsets.push(r.read_u64::<LittleEndian>()?);
        }

        let mut buckets = Vec::with_capacity(num_buckets);
        for i in 0..num_buckets {
            let bucket_start = 0x4000 + i * 0x4000;
            let bucket_end = bucket_start + 0x4000;
            if bucket_end > data.len() {
                return Err(ExtractorError::Format(format!(
                    "BKTR relocation bucket {} exceeds data length",
                    i
                )));
            }

            let b_data = &data[bucket_start..bucket_end];
            let mut b_r = Cursor::new(b_data);
            let _ = b_r.read_u32::<LittleEndian>()?;
            let num_entries = b_r.read_u32::<LittleEndian>()? as usize;
            let virtual_offset_end = b_r.read_u64::<LittleEndian>()?;

            let mut entries = Vec::with_capacity(num_entries);
            for e_idx in 0..num_entries {
                let e_off = 0x10 + e_idx * 20;
                if e_off + 20 > 0x4000 {
                    break;
                }
                let mut e_r = Cursor::new(&b_data[e_off..e_off + 20]);
                let virt_offset = e_r.read_u64::<LittleEndian>()?;
                let phys_offset = e_r.read_u64::<LittleEndian>()?;
                let is_patch = e_r.read_u32::<LittleEndian>()? != 0;

                entries.push(BktrRelocationEntry {
                    virt_offset,
                    phys_offset,
                    is_patch,
                });
            }

            buckets.push(BktrRelocationBucket {
                virtual_offset_end,
                entries,
            });
        }

        Ok(Self {
            total_size,
            bucket_virtual_offsets,
            buckets,
        })
    }

    pub fn get_relocation(&self, offset: u64) -> Option<(BktrRelocationEntry, u64)> {
        if offset >= self.total_size || self.buckets.is_empty() {
            return None;
        }

        let mut bucket_idx = 0;
        for (i, &b_off) in self.bucket_virtual_offsets.iter().enumerate().skip(1) {
            if b_off <= offset {
                bucket_idx = i;
            } else {
                break;
            }
        }

        let bucket = self.buckets.get(bucket_idx)?;
        if bucket.entries.is_empty() {
            return None;
        }

        // Binary search within bucket
        let mut low = 0;
        let mut high = bucket.entries.len() - 1;
        let mut found_idx = 0;

        while low <= high {
            let mid = (low + high) / 2;
            if bucket.entries[mid].virt_offset > offset {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                found_idx = mid;
                if mid == bucket.entries.len() - 1 || bucket.entries[mid + 1].virt_offset > offset {
                    break;
                }
                low = mid + 1;
            }
        }

        let entry = bucket.entries[found_idx];

        // Determine next virtual offset boundary
        let next_virt = if found_idx + 1 < bucket.entries.len() {
            bucket.entries[found_idx + 1].virt_offset
        } else if bucket_idx + 1 < self.buckets.len()
            && !self.buckets[bucket_idx + 1].entries.is_empty()
        {
            self.buckets[bucket_idx + 1].entries[0].virt_offset
        } else {
            bucket.virtual_offset_end.min(self.total_size)
        };

        Some((entry, next_virt))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BktrSubsectionEntry {
    pub offset: u64,
    pub ctr_val: u32,
}

#[derive(Debug, Clone)]
pub struct BktrSubsectionBucket {
    pub physical_offset_end: u64,
    pub entries: Vec<BktrSubsectionEntry>,
}

#[derive(Debug, Clone)]
pub struct BktrSubsectionBlock {
    pub total_size: u64,
    pub bucket_physical_offsets: Vec<u64>,
    pub buckets: Vec<BktrSubsectionBucket>,
}

impl BktrSubsectionBlock {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 0x4000 {
            return Err(ExtractorError::Format(
                "BKTR subsection block header is too small".to_string(),
            ));
        }

        let mut r = Cursor::new(&data[..0x4000]);
        let _ = r.read_u32::<LittleEndian>()?;
        let num_buckets = r.read_u32::<LittleEndian>()? as usize;
        let total_size = r.read_u64::<LittleEndian>()?;

        let mut bucket_physical_offsets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            bucket_physical_offsets.push(r.read_u64::<LittleEndian>()?);
        }

        let mut buckets = Vec::with_capacity(num_buckets);
        for i in 0..num_buckets {
            let bucket_start = 0x4000 + i * 0x4000;
            let bucket_end = bucket_start + 0x4000;
            if bucket_end > data.len() {
                return Err(ExtractorError::Format(format!(
                    "BKTR subsection bucket {} exceeds data length",
                    i
                )));
            }

            let b_data = &data[bucket_start..bucket_end];
            let mut b_r = Cursor::new(b_data);
            let _ = b_r.read_u32::<LittleEndian>()?;
            let num_entries = b_r.read_u32::<LittleEndian>()? as usize;
            let physical_offset_end = b_r.read_u64::<LittleEndian>()?;

            let mut entries = Vec::with_capacity(num_entries);
            for e_idx in 0..num_entries {
                let e_off = 0x10 + e_idx * 16;
                if e_off + 16 > 0x4000 {
                    break;
                }
                let mut e_r = Cursor::new(&b_data[e_off..e_off + 16]);
                let offset = e_r.read_u64::<LittleEndian>()?;
                let _0x8 = e_r.read_u32::<LittleEndian>()?;
                let ctr_val = e_r.read_u32::<LittleEndian>()?;

                entries.push(BktrSubsectionEntry { offset, ctr_val });
            }

            buckets.push(BktrSubsectionBucket {
                physical_offset_end,
                entries,
            });
        }

        Ok(Self {
            total_size,
            bucket_physical_offsets,
            buckets,
        })
    }

    pub fn get_subsection(&self, offset: u64) -> Option<(BktrSubsectionEntry, u64)> {
        if self.buckets.is_empty() {
            return None;
        }

        let mut bucket_idx = 0;
        for (i, &b_off) in self.bucket_physical_offsets.iter().enumerate().skip(1) {
            if b_off <= offset {
                bucket_idx = i;
            } else {
                break;
            }
        }

        let bucket = self.buckets.get(bucket_idx)?;
        if bucket.entries.is_empty() {
            return None;
        }

        let mut low = 0;
        let mut high = bucket.entries.len() - 1;
        let mut found_idx = 0;

        while low <= high {
            let mid = (low + high) / 2;
            if bucket.entries[mid].offset > offset {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                found_idx = mid;
                if mid == bucket.entries.len() - 1 || bucket.entries[mid + 1].offset > offset {
                    break;
                }
                low = mid + 1;
            }
        }

        let entry = bucket.entries[found_idx];

        let next_phys = if found_idx + 1 < bucket.entries.len() {
            bucket.entries[found_idx + 1].offset
        } else if bucket_idx + 1 < self.buckets.len()
            && !self.buckets[bucket_idx + 1].entries.is_empty()
        {
            self.buckets[bucket_idx + 1].entries[0].offset
        } else {
            bucket.physical_offset_end.min(self.total_size)
        };

        Some((entry, next_phys))
    }
}

pub struct BktrReader<RBase: Read + Seek, RPatch: Read + Seek> {
    base_reader: Option<RBase>,
    patch_reader: RPatch,

    base_nca_offset: u64,
    base_sec_offset: u64,
    base_sec_ctr: [u8; 8],
    base_key: Option<[u8; 16]>,
    base_crypt_type: u8,

    patch_nca_offset: u64,
    patch_sec_offset: u64,
    patch_sec_ctr: [u8; 8],
    patch_key: Option<[u8; 16]>,

    reloc_block: BktrRelocationBlock,
    subsec_block: BktrSubsectionBlock,

    pos: u64,
    total_size: u64,
}

impl<RBase: Read + Seek, RPatch: Read + Seek> BktrReader<RBase, RPatch> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_reader: Option<RBase>,
        patch_reader: RPatch,
        base_nca_offset: u64,
        base_sec_offset: u64,
        base_sec_ctr: [u8; 8],
        base_key: Option<[u8; 16]>,
        base_crypt_type: u8,
        patch_nca_offset: u64,
        patch_sec_offset: u64,
        patch_sec_ctr: [u8; 8],
        patch_key: Option<[u8; 16]>,
        reloc_block: BktrRelocationBlock,
        subsec_block: BktrSubsectionBlock,
    ) -> Self {
        let total_size = reloc_block.total_size;
        Self {
            base_reader,
            patch_reader,
            base_nca_offset,
            base_sec_offset,
            base_sec_ctr,
            base_key,
            base_crypt_type,
            patch_nca_offset,
            patch_sec_offset,
            patch_sec_ctr,
            patch_key,
            reloc_block,
            subsec_block,
            pos: 0,
            total_size,
        }
    }

    pub fn size(&self) -> u64 {
        self.total_size
    }
}

impl<RBase: Read + Seek, RPatch: Read + Seek> Seek for BktrReader<RBase, RPatch> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => self.pos as i64 + off,
            SeekFrom::End(off) => self.total_size as i64 + off,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek before start of BKTR virtual stream",
            ));
        }

        self.pos = (new_pos as u64).min(self.total_size);
        Ok(self.pos)
    }
}

impl<RBase: Read + Seek, RPatch: Read + Seek> Read for BktrReader<RBase, RPatch> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.total_size || buf.is_empty() {
            return Ok(0);
        }

        let to_read = (buf.len() as u64).min(self.total_size - self.pos) as usize;
        let mut bytes_read = 0usize;

        while bytes_read < to_read {
            let virt_pos = self.pos + bytes_read as u64;
            let (reloc, next_virt) = self
                .reloc_block
                .get_relocation(virt_pos)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!("Virtual offset 0x{:X} out of relocation bounds", virt_pos),
                    )
                })?;

            let remaining = to_read - bytes_read;
            let reloc_avail = (next_virt - virt_pos) as usize;
            let mut chunk_bytes = remaining.min(reloc_avail);

            let chunk_virt_ofs = virt_pos - reloc.virt_offset;
            let phys_pos = reloc.phys_offset + chunk_virt_ofs;

            if reloc.is_patch {
                let (subsec, next_phys) = self
                    .subsec_block
                    .get_subsection(phys_pos)
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            format!("Patch physical offset 0x{:X} out of subsection bounds", phys_pos),
                        )
                    })?;

                let subsec_avail = (next_phys - phys_pos) as usize;
                chunk_bytes = chunk_bytes.min(subsec_avail);

                let sec_pos = self.patch_sec_offset + phys_pos;
                let sec_aligned_pos = sec_pos & !0x0F;
                let align_diff = (sec_pos - sec_aligned_pos) as usize;

                self.patch_reader
                    .seek(SeekFrom::Start(self.patch_nca_offset + sec_aligned_pos))?;
                let mut raw_bytes = vec![0u8; chunk_bytes + align_diff];
                self.patch_reader.read_exact(&mut raw_bytes)?;

                if let Some(key) = self.patch_key {
                    let mut ctr = [0u8; 16];
                    for j in 0..4 {
                        ctr[j] = self.patch_sec_ctr[7 - j];
                    }
                    ctr[4] = ((subsec.ctr_val >> 24) & 0xFF) as u8;
                    ctr[5] = ((subsec.ctr_val >> 16) & 0xFF) as u8;
                    ctr[6] = ((subsec.ctr_val >> 8) & 0xFF) as u8;
                    ctr[7] = (subsec.ctr_val & 0xFF) as u8;

                    let ofs_block = sec_aligned_pos >> 4;
                    for j in 0..8 {
                        ctr[15 - j] = ((ofs_block >> (j * 8)) & 0xFF) as u8;
                    }

                    use aes::cipher::{KeyIvInit, StreamCipher};
                    let mut cipher = ctr::Ctr64BE::<aes::Aes128>::new(&key.into(), &ctr.into());
                    cipher.apply_keystream(&mut raw_bytes);
                }

                buf[bytes_read..bytes_read + chunk_bytes]
                    .copy_from_slice(&raw_bytes[align_diff..align_diff + chunk_bytes]);
            } else {
                let base = self.base_reader.as_mut().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "RomFS data references Base Game, but no Base Game package was provided to merge.",
                    )
                })?;

                if self.base_crypt_type == 3 && let Some(key) = self.base_key {
                    let sec_pos = self.base_sec_offset + phys_pos;
                    let sec_aligned_pos = sec_pos & !0x0F;
                    let align_diff = (sec_pos - sec_aligned_pos) as usize;

                    base.seek(SeekFrom::Start(self.base_nca_offset + sec_aligned_pos))?;
                    let mut raw_bytes = vec![0u8; chunk_bytes + align_diff];
                    base.read_exact(&mut raw_bytes)?;

                    let mut ctr = [0u8; 16];
                    for j in 0..8 {
                        ctr[j] = self.base_sec_ctr[7 - j];
                    }
                    let ofs_block = sec_aligned_pos >> 4;
                    for j in 0..8 {
                        ctr[15 - j] = ((ofs_block >> (j * 8)) & 0xFF) as u8;
                    }

                    use aes::cipher::{KeyIvInit, StreamCipher};
                    let mut cipher = ctr::Ctr64BE::<aes::Aes128>::new(&key.into(), &ctr.into());
                    cipher.apply_keystream(&mut raw_bytes);

                    buf[bytes_read..bytes_read + chunk_bytes]
                        .copy_from_slice(&raw_bytes[align_diff..align_diff + chunk_bytes]);
                } else {
                    let abs_base_pos = self.base_nca_offset + self.base_sec_offset + phys_pos;
                    base.seek(SeekFrom::Start(abs_base_pos))?;
                    base.read_exact(&mut buf[bytes_read..bytes_read + chunk_bytes])?;
                }
            }

            bytes_read += chunk_bytes;
        }

        self.pos += bytes_read as u64;
        Ok(bytes_read)
    }
}

