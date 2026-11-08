use crate::error::{ExtractorError, Result};
use crate::keys::KeySet;
use aes::cipher::KeyInit;
use aes::Aes128;
use byteorder::{LittleEndian, ReadBytesExt};
use ctr::cipher::{KeyIvInit, StreamCipher};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use xts_mode::Xts128;

type Aes128Ctr64BE = ctr::Ctr64BE<Aes128>;

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB

fn get_tweak_big_endian(sector_num: u128) -> [u8; 16] {
    sector_num.to_be_bytes()
}

pub const CONTENT_TYPES: [&str; 6] = [
    "Program",
    "Meta",
    "Control",
    "Manual",
    "SubSDK",
    "PublicData",
];

#[derive(Debug, Clone)]
pub struct NcaSection {
    pub index: usize,
    pub media_start: u32,
    pub media_end: u32,
    pub offset: u64,
    pub size: u64,
    pub fs_type: u8, // 0: RomFS, 1: PartitionFS
    pub crypt_type: u8, // 1: None, 2: AES-XTS, 3: AES-CTR, 4: AES-BKTR
    pub section_ctr: [u8; 8],
    pub raw_fs_header: Vec<u8>,
    pub content_offset: u64,
    pub content_size: u64,
}

impl NcaSection {
    pub fn fs_type_name(&self) -> &str {
        match self.fs_type {
            0 => "RomFS",
            1 => "PartitionFS",
            _ => "Unknown",
        }
    }

    pub fn crypt_type_name(&self) -> &str {
        match self.crypt_type {
            1 => "None",
            2 => "AES-XTS",
            3 => "AES-CTR",
            4 => "AES-BKTR",
            _ => "Unknown",
        }
    }
}

pub struct Nca<R> {
    reader: R,
    pub base_offset: u64,
    pub total_size: u64,
    pub keyset: KeySet,
    pub magic: [u8; 4],
    pub content_type: u8,
    pub key_generation: usize,
    pub kaek_index: usize,
    pub title_id: u64,
    pub rights_id: [u8; 16],
    pub has_rights_id: bool,
    pub decrypted_key_area: Vec<[u8; 16]>,
    pub sections: Vec<NcaSection>,
}

impl Nca<File> {
    pub fn open<P: AsRef<Path>>(path: P, keyset: KeySet, base_offset: u64) -> Result<Self> {
        let file = File::open(path)?;
        let size = file.metadata()?.len();
        Self::new(file, keyset, base_offset, size)
    }
}

impl<R: Read + Seek> Nca<R> {
    pub fn new(mut reader: R, keyset: KeySet, base_offset: u64, total_size: u64) -> Result<Self> {
        reader.seek(SeekFrom::Start(base_offset))?;
        let mut raw_0x400 = vec![0u8; 0x400];
        reader.read_exact(&mut raw_0x400)?;

        let mut body = raw_0x400[0x200..0x400].to_vec();

        // Check if unencrypted
        let is_plain = matches!(&body[..4], b"NCA3" | b"NCA2" | b"NCA0");
        if !is_plain {
            if !keyset.has_header_key() {
                return Err(ExtractorError::MissingHeaderKey);
            }
            let hkey = keyset.get_header_key().unwrap();
            let c1 = Aes128::new_from_slice(&hkey[..16]).map_err(|e| ExtractorError::Crypto(e.to_string()))?;
            let c2 = Aes128::new_from_slice(&hkey[16..]).map_err(|e| ExtractorError::Crypto(e.to_string()))?;
            let xts = Xts128::new(c1, c2);

            // Sector 1 for bytes 0x200..0x400
            xts.decrypt_area(&mut body, 0x200, 1, get_tweak_big_endian);
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&body[..4]);
        if !matches!(&magic, b"NCA3" | b"NCA2" | b"NCA0") {
            return Err(ExtractorError::InvalidMagic {
                magic: String::from_utf8_lossy(&magic).to_string(),
                expected: "NCA3, NCA2, or NCA0".to_string(),
            });
        }

        let content_type = body[5];
        let kaek_index = body[7] as usize;

        let crypto1 = body[6];
        let crypto2 = if body.len() > 0x20 { body[0x20] } else { 0 };
        let key_gen = crypto1.max(crypto2) as usize;
        let key_generation = if key_gen > 0 { key_gen - 1 } else { 0 };

        let mut title_id_bytes = [0u8; 8];
        title_id_bytes.copy_from_slice(&body[0x10..0x18]);
        let title_id = u64::from_le_bytes(title_id_bytes);

        let mut rights_id = [0u8; 16];
        rights_id.copy_from_slice(&body[0x30..0x40]);
        let has_rights_id = rights_id.iter().any(|&b| b != 0);

        let mut decrypted_key_area = Vec::new();
        if !has_rights_id {
            decrypted_key_area = keyset.decrypt_key_area(&body[0x100..0x140], key_generation, "application");
        }

        // Section table is at offset 0x40 of body
        let sec_table_raw = &body[0x40..0x80];

        // Read the 4 FsHeaders from 0x400 to 0xC00
        reader.seek(SeekFrom::Start(base_offset + 0x400))?;
        let mut raw_fs_headers = vec![0u8; 0x800];
        reader.read_exact(&mut raw_fs_headers)?;

        let mut sections = Vec::new();

        let hkey_opt = keyset.get_header_key();

        for i in 0..4 {
            let ent = &sec_table_raw[i * 16..(i + 1) * 16];
            let mut cursor = std::io::Cursor::new(ent);
            let m_start = cursor.read_u32::<LittleEndian>()?;
            let m_end = cursor.read_u32::<LittleEndian>()?;

            if m_start == 0 && m_end == 0 {
                continue;
            }

            let mut dec_fs = raw_fs_headers[i * 0x200..(i + 1) * 0x200].to_vec();

            // Decrypt FsHeader with AES-XTS (sector 2 + i)
            if let Some(hkey) = hkey_opt
                && let (Ok(c1), Ok(c2)) = (Aes128::new_from_slice(&hkey[..16]), Aes128::new_from_slice(&hkey[16..])) {
                    let xts = Xts128::new(c1, c2);
                    xts.decrypt_area(&mut dec_fs, 0x200, (2 + i) as u128, get_tweak_big_endian);
                }

            let part_type = dec_fs[2];
            let fs_type_raw = dec_fs[3];
            let crypt_type = dec_fs[4];

            let mut section_ctr = [0u8; 8];
            section_ctr.copy_from_slice(&dec_fs[0x140..0x148]);

            let mut content_off = 0u64;
            let mut content_sz = (m_end as u64 - m_start as u64) * 0x200;

            // PartitionFS (ExeFS)
            if (part_type == 1 || fs_type_raw == 1 || fs_type_raw == 2)
                && dec_fs.len() >= 0x50 {
                    let mut cur = std::io::Cursor::new(&dec_fs[0x40..0x50]);
                    let pfs_off = cur.read_u64::<LittleEndian>()?;
                    let pfs_sz = cur.read_u64::<LittleEndian>()?;
                    if pfs_sz > 0 {
                        content_off = pfs_off;
                        content_sz = pfs_sz;
                    }
                }

            // RomFS (IVFC)
            if &dec_fs[0x08..0x0C] == b"IVFC" && dec_fs.len() >= 0x20 {
                let mut cur = std::io::Cursor::new(&dec_fs[0x14..0x18]);
                let num_levels = cur.read_u32::<LittleEndian>()? as usize;
                if (1..=7).contains(&num_levels) {
                    for lvl in (0..num_levels).rev() {
                        let lvl_off = 0x18 + lvl * 24;
                        if dec_fs.len() >= lvl_off + 16 {
                            let mut l_cur = std::io::Cursor::new(&dec_fs[lvl_off..lvl_off + 16]);
                            let log_off = l_cur.read_u64::<LittleEndian>()?;
                            let lvl_sz = l_cur.read_u64::<LittleEndian>()?;
                            if lvl_sz > 0 {
                                content_off = log_off;
                                content_sz = lvl_sz;
                                break;
                            }
                        }
                    }
                }
            }

            let normalized_fs_type = if &dec_fs[0x08..0x0C] == b"IVFC" || part_type == 0 {
                0 // RomFS
            } else {
                1 // PartitionFS
            };

            let sec = NcaSection {
                index: i,
                media_start: m_start,
                media_end: m_end,
                offset: (m_start as u64) * 0x200,
                size: (m_end as u64 - m_start as u64) * 0x200,
                fs_type: normalized_fs_type,
                crypt_type,
                section_ctr,
                raw_fs_header: dec_fs,
                content_offset: content_off,
                content_size: content_sz,
            };

            sections.push(sec);
        }

        Ok(Self {
            reader,
            base_offset,
            total_size,
            keyset,
            magic,
            content_type,
            key_generation,
            kaek_index,
            title_id,
            rights_id,
            has_rights_id,
            decrypted_key_area,
            sections,
        })
    }

    pub fn content_type_name(&self) -> &str {
        if (self.content_type as usize) < CONTENT_TYPES.len() {
            CONTENT_TYPES[self.content_type as usize]
        } else {
            "Unknown"
        }
    }

    pub fn title_id_hex(&self) -> String {
        format!("{:016X}", self.title_id)
    }

    pub fn get_section_key(&self, section: &NcaSection) -> Option<[u8; 16]> {
        if section.crypt_type == 1 {
            return None; // Plaintext
        }

        if self.has_rights_id {
            return self.keyset.get_title_key(&self.rights_id, Some(self.key_generation));
        }

        if self.decrypted_key_area.len() > 2 {
            let key2 = self.decrypted_key_area[2];
            if key2.iter().any(|&b| b != 0) {
                return Some(key2);
            }
        }

        if self.decrypted_key_area.len() > self.kaek_index {
            let key_kaek = self.decrypted_key_area[self.kaek_index];
            if key_kaek.iter().any(|&b| b != 0) {
                return Some(key_kaek);
            }
        }

        None
    }

    pub fn create_ctr_for_offset(section: &NcaSection, abs_nca_offset: u64) -> [u8; 16] {
        let mut ctr = [0u8; 16];
        // Upper 8 bytes: reversed section_ctr
        #[allow(clippy::needless_range_loop)]
        for j in 0..8 {
            ctr[j] = section.section_ctr[7 - j];
        }
        // Lower 8 bytes: offset >> 4 (big-endian)
        let ofs = abs_nca_offset >> 4;
        #[allow(clippy::needless_range_loop)]
        for j in 0..8 {
            ctr[15 - j] = ((ofs >> (j * 8)) & 0xFF) as u8;
        }
        ctr
    }

    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn extract_section_raw<F, C>(
        &mut self,
        section: &NcaSection,
        out_path: &Path,
        decrypt: bool,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<u64>
    where
        F: Fn(u64, u64),
        C: Fn() -> bool,
    {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let key = if decrypt { self.get_section_key(section) } else { None };
        let should_decrypt = decrypt && (section.crypt_type == 3) && key.is_some();

        if decrypt && (section.crypt_type == 2 || section.crypt_type == 3) && key.is_none() {
            let rid_str = if self.has_rights_id {
                hex::encode(self.rights_id)
            } else {
                "None".to_string()
            };
            return Err(ExtractorError::MissingTitleKey(rid_str));
        }

        let sec_abs_start = self.base_offset + section.offset;
        let sec_size = section.size;

        self.reader.seek(SeekFrom::Start(sec_abs_start))?;
        let mut out_file = File::create(out_path)?;
        let mut remaining = sec_size;
        let mut written = 0u64;

        let mut cipher_opt = if should_decrypt {
            let initial_ctr = Self::create_ctr_for_offset(section, section.offset);
            Some(Aes128Ctr64BE::new(
                &key.unwrap().into(),
                &initial_ctr.into(),
            ))
        } else {
            None
        };

        let mut buf = vec![0u8; CHUNK_SIZE];

        while remaining > 0 {
            if let Some(cancel) = cancel_check
                && cancel() {
                    drop(out_file);
                    let _ = std::fs::remove_file(out_path);
                    return Err(ExtractorError::Cancelled);
                }

            let to_read = (remaining as usize).min(CHUNK_SIZE);
            self.reader.read_exact(&mut buf[..to_read])?;

            if let Some(ref mut c) = cipher_opt {
                c.apply_keystream(&mut buf[..to_read]);
            }

            out_file.write_all(&buf[..to_read])?;
            remaining -= to_read as u64;
            written += to_read as u64;

            if let Some(cb) = progress_cb {
                cb(written, sec_size);
            }
        }

        out_file.flush()?;
        Ok(written)
    }
}

pub struct NcaSectionReader<'a, R: Read + Seek> {
    reader: &'a mut R,
    base_offset: u64,
    section: NcaSection,
    key: Option<[u8; 16]>,
    content_offset: u64,
    content_size: u64,
    pos: u64,
}

impl<'a, R: Read + Seek> NcaSectionReader<'a, R> {
    pub fn new(
        reader: &'a mut R,
        base_offset: u64,
        section: NcaSection,
        key: Option<[u8; 16]>,
    ) -> Self {
        let content_offset = section.content_offset;
        let content_size = section.content_size;
        Self {
            reader,
            base_offset,
            section,
            key,
            content_offset,
            content_size,
            pos: 0,
        }
    }

    pub fn new_raw(
        reader: &'a mut R,
        base_offset: u64,
        section: NcaSection,
        key: Option<[u8; 16]>,
    ) -> Self {
        let content_size = section.size;
        Self {
            reader,
            base_offset,
            section,
            key,
            content_offset: 0,
            content_size,
            pos: 0,
        }
    }

    pub fn size(&self) -> u64 {
        self.content_size
    }
}

impl<'a, R: Read + Seek> Seek for NcaSectionReader<'a, R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => self.pos as i64 + off,
            SeekFrom::End(off) => self.content_size as i64 + off,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek before start of section",
            ));
        }

        self.pos = (new_pos as u64).min(self.content_size);
        Ok(self.pos)
    }
}

impl<'a, R: Read + Seek> Read for NcaSectionReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.content_size {
            return Ok(0);
        }

        let to_read = (buf.len() as u64).min(self.content_size - self.pos) as usize;
        if to_read == 0 {
            return Ok(0);
        }

        let abs_pos = self.section.offset + self.content_offset + self.pos;
        let aligned_pos = abs_pos & !0x0F;
        let align_diff = (abs_pos - aligned_pos) as usize;

        self.reader.seek(SeekFrom::Start(self.base_offset + aligned_pos))?;
        let mut raw_bytes = vec![0u8; to_read + align_diff];
        self.reader.read_exact(&mut raw_bytes)?;

        if (self.section.crypt_type == 3 || self.section.crypt_type == 4) && let Some(key) = self.key {
            let mut ctr = [0u8; 16];
            #[allow(clippy::needless_range_loop)]
            for j in 0..8 {
                ctr[j] = self.section.section_ctr[7 - j];
            }
            let block_num = aligned_pos >> 4;
            #[allow(clippy::needless_range_loop)]
            for j in 0..8 {
                ctr[15 - j] = ((block_num >> (j * 8)) & 0xFF) as u8;
            }

            let mut cipher = Aes128Ctr64BE::new(&key.into(), &ctr.into());
            cipher.apply_keystream(&mut raw_bytes);
        }

        let data = &raw_bytes[align_diff..align_diff + to_read];
        buf[..to_read].copy_from_slice(data);
        self.pos += to_read as u64;

        Ok(to_read)
    }
}
