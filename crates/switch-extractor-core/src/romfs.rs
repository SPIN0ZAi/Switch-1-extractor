use crate::error::{ExtractorError, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB chunk

#[derive(Debug, Clone)]
pub struct RomFsFileEntry {
    pub name: String,
    pub parent_dir_offset: u32,
    pub data_offset: u64,
    pub size: u64,
    pub full_path: String,
}

#[derive(Debug, Clone)]
pub struct RomFsDirEntry {
    pub name: String,
    pub offset: u32,
    pub parent_offset: u32,
    pub full_path: String,
}

pub struct RomFs<R> {
    reader: R,
    pub base_offset: u64,
    pub file_data_offset: u64,
    pub directories: HashMap<u32, RomFsDirEntry>,
    pub files: Vec<RomFsFileEntry>,
    files_by_path: HashMap<String, usize>,
}

impl RomFs<File> {
    pub fn open<P: AsRef<Path>>(path: P, base_offset: u64) -> Result<Self> {
        let file = File::open(path)?;
        Self::new(file, base_offset)
    }
}

impl<R: Read + Seek> RomFs<R> {
    pub fn new(mut reader: R, base_offset: u64) -> Result<Self> {
        reader.seek(SeekFrom::Start(base_offset))?;
        let mut magic_or_sz = [0u8; 8];
        reader.read_exact(&mut magic_or_sz)?;

        let romfs_header_offset = if &magic_or_sz[..4] == b"IVFC" {
            // Level 3 logical offset is at offset 0x3C in IVFC
            reader.seek(SeekFrom::Start(base_offset + 0x3C))?;
            let l3_off = reader.read_u64::<LittleEndian>()?;
            let _l3_sz = reader.read_u64::<LittleEndian>()?;
            base_offset + l3_off
        } else {
            base_offset
        };

        reader.seek(SeekFrom::Start(romfs_header_offset))?;
        let hdr_sz = reader.read_u64::<LittleEndian>()?;
        if hdr_sz != 0x50 {
            return Err(ExtractorError::InvalidRomFsHeader(format!(
                "Invalid RomFS header size: 0x{:X}. Expected 0x50.",
                hdr_sz
            )));
        }

        let _dir_hash_off = reader.read_u64::<LittleEndian>()?;
        let _dir_hash_sz = reader.read_u64::<LittleEndian>()?;
        let dir_table_off = reader.read_u64::<LittleEndian>()?;
        let dir_table_sz = reader.read_u64::<LittleEndian>()?;
        let _file_hash_off = reader.read_u64::<LittleEndian>()?;
        let _file_hash_sz = reader.read_u64::<LittleEndian>()?;
        let file_table_off = reader.read_u64::<LittleEndian>()?;
        let file_table_sz = reader.read_u64::<LittleEndian>()?;
        let file_data_off = reader.read_u64::<LittleEndian>()?;

        let file_data_offset = romfs_header_offset + file_data_off;

        // 1. Read and parse Directory Table
        reader.seek(SeekFrom::Start(romfs_header_offset + dir_table_off))?;
        let mut dir_table_data = vec![0u8; dir_table_sz as usize];
        reader.read_exact(&mut dir_table_data)?;

        let mut directories = HashMap::new();
        let mut pos = 0usize;
        let d_len = dir_table_data.len();

        while pos < d_len {
            if pos + 0x18 > d_len {
                break;
            }
            let mut cursor = std::io::Cursor::new(&dir_table_data[pos..pos + 0x18]);
            let parent_off = cursor.read_u32::<LittleEndian>()?;
            let _sibling_off = cursor.read_u32::<LittleEndian>()?;
            let _child_dir_off = cursor.read_u32::<LittleEndian>()?;
            let _child_file_off = cursor.read_u32::<LittleEndian>()?;
            let _next_hash = cursor.read_u32::<LittleEndian>()?;
            let name_len = cursor.read_u32::<LittleEndian>()? as usize;

            if pos + 0x18 + name_len > d_len {
                break;
            }

            let name_bytes = &dir_table_data[pos + 0x18..pos + 0x18 + name_len];
            let name = String::from_utf8_lossy(name_bytes).to_string();

            directories.insert(
                pos as u32,
                RomFsDirEntry {
                    name,
                    offset: pos as u32,
                    parent_offset: parent_off,
                    full_path: String::new(),
                },
            );

            let entry_sz = (0x18 + name_len + 3) & !3;
            if entry_sz == 0 {
                break;
            }
            pos += entry_sz;
        }

        // Reconstruct directory full paths
        fn resolve_dir(dirs: &mut HashMap<u32, RomFsDirEntry>, offset: u32) -> String {
            if let Some(d) = dirs.get(&offset) {
                if !d.full_path.is_empty() {
                    return d.full_path.clone();
                }
                if d.parent_offset == 0xFFFF_FFFF || d.parent_offset == offset || !dirs.contains_key(&d.parent_offset) {
                    let name = d.name.clone();
                    if let Some(entry) = dirs.get_mut(&offset) {
                        entry.full_path = name.clone();
                    }
                    return name;
                }
            } else {
                return String::new();
            }

            let parent_off = dirs.get(&offset).unwrap().parent_offset;
            let parent_path = resolve_dir(dirs, parent_off);
            let dir_name = dirs.get(&offset).unwrap().name.clone();

            let full = if parent_path.is_empty() {
                dir_name
            } else {
                format!("{}/{}", parent_path.trim_matches('/'), dir_name.trim_matches('/'))
            };

            if let Some(entry) = dirs.get_mut(&offset) {
                entry.full_path = full.clone();
            }
            full
        }

        let dir_offsets: Vec<u32> = directories.keys().cloned().collect();
        for off in dir_offsets {
            resolve_dir(&mut directories, off);
        }

        // 2. Read and parse File Table
        reader.seek(SeekFrom::Start(romfs_header_offset + file_table_off))?;
        let mut file_table_data = vec![0u8; file_table_sz as usize];
        reader.read_exact(&mut file_table_data)?;

        let mut files = Vec::new();
        let mut files_by_path = HashMap::new();
        let f_len = file_table_data.len();
        pos = 0;

        while pos < f_len {
            if pos + 0x20 > f_len {
                break;
            }
            let mut cursor = std::io::Cursor::new(&file_table_data[pos..pos + 0x20]);
            let parent_off = cursor.read_u32::<LittleEndian>()?;
            let _sibling_off = cursor.read_u32::<LittleEndian>()?;
            let data_off = cursor.read_u64::<LittleEndian>()?;
            let data_sz = cursor.read_u64::<LittleEndian>()?;
            let _next_hash = cursor.read_u32::<LittleEndian>()?;
            let name_len = cursor.read_u32::<LittleEndian>()? as usize;

            if pos + 0x20 + name_len > f_len {
                break;
            }

            let name_bytes = &file_table_data[pos + 0x20..pos + 0x20 + name_len];
            let name = String::from_utf8_lossy(name_bytes).to_string();

            let dir_path = directories
                .get(&parent_off)
                .map(|d| d.full_path.as_str())
                .unwrap_or("");

            let full_path = if dir_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", dir_path.trim_matches('/'), name.trim_matches('/'))
            };

            let entry = RomFsFileEntry {
                name,
                parent_dir_offset: parent_off,
                data_offset: data_off,
                size: data_sz,
                full_path: full_path.clone(),
            };

            let idx = files.len();
            files_by_path.insert(full_path.to_lowercase(), idx);
            files.push(entry);

            let entry_sz = (0x20 + name_len + 3) & !3;
            if entry_sz == 0 {
                break;
            }
            pos += entry_sz;
        }

        Ok(Self {
            reader,
            base_offset,
            file_data_offset,
            directories,
            files,
            files_by_path,
        })
    }

    pub fn get_file(&self, path: &str) -> Option<&RomFsFileEntry> {
        let clean = path.trim_matches('/').replace('\\', "/").to_lowercase();
        self.files_by_path.get(&clean).map(|&idx| &self.files[idx])
    }

    pub fn read_file(&mut self, entry: &RomFsFileEntry) -> Result<Vec<u8>> {
        self.reader
            .seek(SeekFrom::Start(self.file_data_offset + entry.data_offset))?;
        let mut buf = vec![0u8; entry.size as usize];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn extract_entry<F, C>(
        &mut self,
        entry: &RomFsFileEntry,
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

        self.reader
            .seek(SeekFrom::Start(self.file_data_offset + entry.data_offset))?;
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
        let total_files = self.files.len();
        let mut extracted = Vec::with_capacity(total_files);

        let files = self.files.clone();
        for (idx, entry) in files.iter().enumerate() {
            let relative_clean = entry.full_path.replace('/', std::path::MAIN_SEPARATOR_STR);
            let dest_file = out_dir.join(relative_clean);

            let file_cb = |done: u64, total: u64| {
                if let Some(cb) = progress_cb {
                    cb(idx + 1, total_files, &entry.full_path, done, total);
                }
            };

            self.extract_entry(entry, &dest_file, Some(&file_cb), cancel_check)?;
            extracted.push(dest_file);
        }

        Ok(extracted)
    }
}
