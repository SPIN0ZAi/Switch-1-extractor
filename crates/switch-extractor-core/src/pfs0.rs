use crate::error::{ExtractorError, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunk

#[derive(Debug, Clone)]
pub struct Pfs0Entry {
    pub name: String,
    pub data_offset: u64,
    pub size: u64,
    pub abs_offset: u64,
    pub sha256: Option<[u8; 32]>,
}

#[derive(Debug)]
pub struct Pfs0<R> {
    reader: R,
    pub base_offset: u64,
    pub is_hfs0: bool,
    pub header_size: u64,
    pub data_start: u64,
    pub entries: Vec<Pfs0Entry>,
    entries_by_name: HashMap<String, usize>,
}

impl Pfs0<File> {
    pub fn open<P: AsRef<Path>>(path: P, base_offset: u64) -> Result<Self> {
        let file = File::open(path)?;
        Self::new(file, base_offset)
    }
}

impl<R: Read + Seek> Pfs0<R> {
    pub fn new(mut reader: R, base_offset: u64) -> Result<Self> {
        reader.seek(SeekFrom::Start(base_offset))?;

        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        let (is_hfs0, entry_size) = match &magic {
            b"PFS0" => (false, 0x18),
            b"HFS0" => (true, 0x40),
            _ => {
                return Err(ExtractorError::InvalidMagic {
                    magic: String::from_utf8_lossy(&magic).to_string(),
                    expected: "PFS0 or HFS0".to_string(),
                });
            }
        };

        let file_count = reader.read_u32::<LittleEndian>()? as usize;
        let string_table_size = reader.read_u32::<LittleEndian>()? as usize;
        let _reserved = reader.read_u32::<LittleEndian>()?;

        let mut entry_table_bytes = vec![0u8; file_count * entry_size];
        reader.read_exact(&mut entry_table_bytes)?;

        let mut string_table = vec![0u8; string_table_size];
        reader.read_exact(&mut string_table)?;

        let header_size = 0x10 + (file_count * entry_size) as u64 + string_table_size as u64;
        let data_start = base_offset + header_size;

        let get_string = |offset: usize| -> String {
            if offset >= string_table.len() {
                return String::new();
            }
            let slice = &string_table[offset..];
            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            String::from_utf8_lossy(&slice[..end]).to_string()
        };

        let mut entries = Vec::with_capacity(file_count);
        let mut entries_by_name = HashMap::with_capacity(file_count);

        for i in 0..file_count {
            let ent_slice = &entry_table_bytes[i * entry_size..(i + 1) * entry_size];
            let mut cursor = std::io::Cursor::new(ent_slice);

            let (data_off, file_sz, name_off, sha256) = if !is_hfs0 {
                let data_off = cursor.read_u64::<LittleEndian>()?;
                let file_sz = cursor.read_u64::<LittleEndian>()?;
                let name_off = cursor.read_u32::<LittleEndian>()? as usize;
                (data_off, file_sz, name_off, None)
            } else {
                let data_off = cursor.read_u64::<LittleEndian>()?;
                let file_sz = cursor.read_u64::<LittleEndian>()?;
                let name_off = cursor.read_u32::<LittleEndian>()? as usize;
                let _hash_sz = cursor.read_u32::<LittleEndian>()?;
                let _res = cursor.read_u64::<LittleEndian>()?;
                let mut sha = [0u8; 32];
                cursor.read_exact(&mut sha)?;
                (data_off, file_sz, name_off, Some(sha))
            };

            let name = get_string(name_off);
            let abs_off = data_start + data_off;
            let entry = Pfs0Entry {
                name: name.clone(),
                data_offset: data_off,
                size: file_sz,
                abs_offset: abs_off,
                sha256,
            };

            entries_by_name.insert(name, entries.len());
            entries.push(entry);
        }

        Ok(Self {
            reader,
            base_offset,
            is_hfs0,
            header_size,
            data_start,
            entries,
            entries_by_name,
        })
    }

    pub fn get_entry(&self, name: &str) -> Option<&Pfs0Entry> {
        self.entries_by_name.get(name).map(|&idx| &self.entries[idx])
    }

    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn read_entry(&mut self, entry: &Pfs0Entry) -> Result<Vec<u8>> {
        self.reader.seek(SeekFrom::Start(entry.abs_offset))?;
        let mut buf = vec![0u8; entry.size as usize];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn extract_entry<F, C>(
        &mut self,
        entry: &Pfs0Entry,
        out_path: &Path,
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

        self.reader.seek(SeekFrom::Start(entry.abs_offset))?;
        let mut out_file = File::create(out_path)?;
        let mut remaining = entry.size;
        let mut written = 0u64;
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
            out_file.write_all(&buf[..to_read])?;

            remaining -= to_read as u64;
            written += to_read as u64;

            if let Some(cb) = progress_cb {
                cb(written, entry.size);
            }
        }

        out_file.flush()?;
        Ok(written)
    }

    pub fn extract_all<F, C>(
        &mut self,
        out_dir: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<Vec<PathBuf>>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        std::fs::create_dir_all(out_dir)?;
        let total = self.entries.len();
        let mut extracted = Vec::with_capacity(total);

        for (idx, entry) in self.entries.clone().iter().enumerate() {
            let out_file = out_dir.join(&entry.name);
            let name_clone = entry.name.clone();

            let file_cb = |done: u64, tot: u64| {
                if let Some(cb) = progress_cb {
                    cb(idx + 1, total, &name_clone, done, tot);
                }
            };

            self.extract_entry(entry, &out_file, Some(&file_cb), cancel_check)?;
            extracted.push(out_file);
        }

        Ok(extracted)
    }
}

pub struct Pfs0Builder;

impl Pfs0Builder {
    pub fn build<F, C>(
        files: &[(String, PathBuf)],
        out_path: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<u64>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        if files.is_empty() {
            return Err(ExtractorError::Format("No files to pack into PFS0".to_string()));
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file_count = files.len();
        let entry_size = 0x18;
        let mut string_table = Vec::new();
        let mut entries_meta = Vec::with_capacity(file_count);
        let mut current_data_offset = 0u64;

        for (name, path) in files {
            let name_bytes = name.as_bytes();
            let name_offset = string_table.len() as u32;
            string_table.extend_from_slice(name_bytes);
            string_table.push(0);

            let file_sz = std::fs::metadata(path)?.len();
            entries_meta.push((current_data_offset, file_sz, name_offset));
            current_data_offset += file_sz;
        }

        let mut out_file = File::create(out_path)?;

        // Write Header
        out_file.write_all(b"PFS0")?;
        out_file.write_u32::<LittleEndian>(file_count as u32)?;
        out_file.write_u32::<LittleEndian>(string_table.len() as u32)?;
        out_file.write_u32::<LittleEndian>(0)?; // reserved

        for (data_off, file_sz, name_off) in entries_meta {
            out_file.write_u64::<LittleEndian>(data_off)?;
            out_file.write_u64::<LittleEndian>(file_sz)?;
            out_file.write_u32::<LittleEndian>(name_off)?;
            out_file.write_u32::<LittleEndian>(0)?; // reserved
        }

        out_file.write_all(&string_table)?;

        // Stream file contents
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut total_written = 0x10 + (file_count * entry_size) as u64 + string_table.len() as u64;

        for (idx, (name, path)) in files.iter().enumerate() {
            let mut in_file = File::open(path)?;
            let file_sz = in_file.metadata()?.len();
            let mut written_file = 0u64;

            while written_file < file_sz {
                if let Some(cancel) = cancel_check
                    && cancel() {
                        drop(out_file);
                        let _ = std::fs::remove_file(out_path);
                        return Err(ExtractorError::Cancelled);
                    }

                let to_read = ((file_sz - written_file) as usize).min(CHUNK_SIZE);
                in_file.read_exact(&mut buf[..to_read])?;
                out_file.write_all(&buf[..to_read])?;
                written_file += to_read as u64;
                total_written += to_read as u64;

                if let Some(cb) = progress_cb {
                    cb(idx + 1, file_count, name, written_file, file_sz);
                }
            }
        }

        out_file.flush()?;
        Ok(total_written)
    }
}
