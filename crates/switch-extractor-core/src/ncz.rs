use crate::error::{ExtractorError, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use ctr::cipher::{KeyIvInit, StreamCipher};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const NCA_HEADER_SIZE: u64 = 0x4000; // 16 KB
const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB

#[derive(Debug, Clone)]
pub struct NczSection {
    pub offset: u64,
    pub size: u64,
    pub crypto_type: u64, // 1: None, 2: AES-CTR
    pub crypto_key: [u8; 16],
    pub crypto_counter: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct NczBlockHeader {
    pub block_size: usize,
    pub num_blocks: usize,
    pub decompressed_size: u64,
    pub block_sizes: Vec<u32>,
}

pub struct NczDecompressor<R> {
    reader: R,
    pub base_offset: u64,
    pub total_size: u64,
    pub sections: Vec<NczSection>,
    pub block_header: Option<NczBlockHeader>,
    pub payload_offset: u64,
}

impl NczDecompressor<File> {
    pub fn open<P: AsRef<Path>>(path: P, base_offset: u64) -> Result<Self> {
        let file = File::open(path)?;
        let size = file.metadata()?.len();
        Self::new(file, base_offset, size)
    }
}

impl<R: Read + Seek> NczDecompressor<R> {
    pub fn new(mut reader: R, base_offset: u64, total_size: u64) -> Result<Self> {
        reader.seek(SeekFrom::Start(base_offset + NCA_HEADER_SIZE))?;

        let section_count = reader.read_u64::<LittleEndian>()? as usize;
        let mut sections = Vec::with_capacity(section_count);

        for _ in 0..section_count {
            let mut sec_raw = [0u8; 72];
            reader.read_exact(&mut sec_raw)?;

            let magic = &sec_raw[..8];
            if magic != b"NCZSECTN" {
                return Err(ExtractorError::InvalidMagic {
                    magic: String::from_utf8_lossy(magic).to_string(),
                    expected: "NCZSECTN".to_string(),
                });
            }

            let mut cursor = std::io::Cursor::new(&sec_raw[8..40]);
            let offset = cursor.read_u64::<LittleEndian>()?;
            let size = cursor.read_u64::<LittleEndian>()?;
            let crypto_type = cursor.read_u64::<LittleEndian>()?;
            let _reserved = cursor.read_u64::<LittleEndian>()?;

            let mut crypto_key = [0u8; 16];
            crypto_key.copy_from_slice(&sec_raw[40..56]);

            let mut crypto_counter = [0u8; 16];
            crypto_counter.copy_from_slice(&sec_raw[56..72]);

            sections.push(NczSection {
                offset,
                size,
                crypto_type,
                crypto_key,
                crypto_counter,
            });
        }

        // Check for optional NCZBLOCK header
        let pos = reader.stream_position()?;
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;

        let (block_header, payload_offset) = if &magic == b"NCZBLOCK" {
            let _ver = reader.read_u8()?;
            let _b_type = reader.read_u8()?;
            let _unused = reader.read_u8()?;
            let exp = reader.read_u8()?;
            let block_size = 1usize << exp;

            let num_blocks = reader.read_u32::<LittleEndian>()? as usize;
            let decompressed_size = reader.read_u64::<LittleEndian>()?;

            let mut block_sizes = Vec::with_capacity(num_blocks);
            for _ in 0..num_blocks {
                block_sizes.push(reader.read_u32::<LittleEndian>()?);
            }

            let p_off = reader.stream_position()?;
            (
                Some(NczBlockHeader {
                    block_size,
                    num_blocks,
                    decompressed_size,
                    block_sizes,
                }),
                p_off,
            )
        } else {
            (None, pos)
        };

        Ok(Self {
            reader,
            base_offset,
            total_size,
            sections,
            block_header,
            payload_offset,
        })
    }

    pub fn decompress<F, C>(
        &mut self,
        out_path: &Path,
        reencrypt: bool,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<PathBuf>
    where
        F: Fn(u64, u64),
        C: Fn() -> bool,
    {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 1. Copy original 0x4000 bytes (original NCA header)
        self.reader.seek(SeekFrom::Start(self.base_offset))?;
        let mut nca_header = vec![0u8; NCA_HEADER_SIZE as usize];
        self.reader.read_exact(&mut nca_header)?;

        let mut out_file = File::create(out_path)?;
        out_file.write_all(&nca_header)?;

        if self.block_header.is_some() {
            self.decompress_blocks(&mut out_file, reencrypt, progress_cb, cancel_check, out_path)?;
        } else {
            self.decompress_solid(&mut out_file, reencrypt, progress_cb, cancel_check, out_path)?;
        }

        out_file.flush()?;
        Ok(out_path.to_path_buf())
    }

    fn decompress_solid<F, C>(
        &mut self,
        out_f: &mut File,
        reencrypt: bool,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
        out_path: &Path,
    ) -> Result<()>
    where
        F: Fn(u64, u64),
        C: Fn() -> bool,
    {
        self.reader.seek(SeekFrom::Start(self.payload_offset))?;

        let est_total: u64 = if !self.sections.is_empty() {
            self.sections.iter().map(|s| s.size).sum::<u64>() + NCA_HEADER_SIZE
        } else {
            0
        };

        let mut decoder = zstd::stream::read::Decoder::new(&mut self.reader)?;
        let mut current_offset = NCA_HEADER_SIZE;
        let mut buf = vec![0u8; CHUNK_SIZE];

        loop {
            if let Some(cancel) = cancel_check
                && cancel() {
                    let _ = std::fs::remove_file(out_path);
                    return Err(ExtractorError::Cancelled);
                }

            let n = decoder.read(&mut buf)?;
            if n == 0 {
                break;
            }

            if reencrypt && !self.sections.is_empty() {
                Self::apply_reencryption(&self.sections, &mut buf[..n], current_offset);
            }

            out_f.write_all(&buf[..n])?;
            current_offset += n as u64;

            if let Some(cb) = progress_cb {
                cb(current_offset, est_total);
            }
        }

        Ok(())
    }

    fn decompress_blocks<F, C>(
        &mut self,
        out_f: &mut File,
        reencrypt: bool,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
        out_path: &Path,
    ) -> Result<()>
    where
        F: Fn(u64, u64),
        C: Fn() -> bool,
    {
        let header = self.block_header.as_ref().unwrap().clone();
        self.reader.seek(SeekFrom::Start(self.payload_offset))?;
        let mut current_offset = NCA_HEADER_SIZE;
        let total_decomp = header.decompressed_size + NCA_HEADER_SIZE;

        for &b_sz in &header.block_sizes {
            if let Some(cancel) = cancel_check
                && cancel() {
                    let _ = std::fs::remove_file(out_path);
                    return Err(ExtractorError::Cancelled);
                }

            let mut chunk = vec![0u8; b_sz as usize];
            self.reader.read_exact(&mut chunk)?;

            let mut block_data = if b_sz as usize == header.block_size {
                chunk
            } else {
                zstd::bulk::decompress(&chunk, header.block_size)?
            };

            if reencrypt && !self.sections.is_empty() {
                Self::apply_reencryption(&self.sections, &mut block_data, current_offset);
            }

            out_f.write_all(&block_data)?;
            current_offset += block_data.len() as u64;

            if let Some(cb) = progress_cb {
                cb(current_offset, total_decomp);
            }
        }

        Ok(())
    }
}

impl<R> NczDecompressor<R> {
    pub fn apply_reencryption(sections: &[NczSection], data: &mut [u8], current_offset: u64) {
        let data_len = data.len() as u64;
        let data_end = current_offset + data_len;

        for sec in sections {
            if sec.crypto_type != 2 {
                continue;
            }

            let sec_start = sec.offset;
            let sec_end = sec.offset + sec.size;

            let overlap_start = current_offset.max(sec_start);
            let overlap_end = data_end.min(sec_end);

            if overlap_start < overlap_end {
                let rel_offset = overlap_start - sec_start;
                let slice_start = (overlap_start - current_offset) as usize;
                let slice_end = (overlap_end - current_offset) as usize;
                let slice_data = &mut data[slice_start..slice_end];

                let mut ctr = sec.crypto_counter;
                let block_num = rel_offset >> 4;
                for j in 0..8 {
                    ctr[15 - j] = ((block_num >> (j * 8)) & 0xFF) as u8;
                }

                let rem = (rel_offset % 16) as usize;
                let mut cipher = ctr::Ctr64BE::<aes::Aes128>::new(&sec.crypto_key.into(), &ctr.into());
                if rem > 0 {
                    let mut dummy = [0u8; 16];
                    cipher.apply_keystream(&mut dummy[..rem]);
                }

                cipher.apply_keystream(slice_data);
            }
        }
    }
}
