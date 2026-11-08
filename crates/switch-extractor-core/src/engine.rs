use crate::error::{ExtractorError, Result};
use crate::keys::KeySet;
use crate::nacp::Nacp;
use crate::nca::{Nca, NcaSectionReader};
use crate::ncz::NczDecompressor;
use crate::pfs0::{Pfs0, Pfs0Builder};
use crate::romfs::RomFs;
use crate::xci::Xci;
use base64::Engine;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchGameInfo {
    pub file_path: Option<String>,
    pub format: String,
    pub file_size: u64,
    pub file_size_formatted: String,
    pub title_name: String,
    pub publisher: String,
    pub display_version: String,
    pub title_id: String,
    pub icon_base64: Option<String>,
    pub files_list: Vec<String>,
    pub has_keys: bool,
    pub is_compressed: bool,
    pub has_ticket: bool,
    pub ticket_decrypted: bool,
    pub is_update: bool,
    pub requires_base_game: bool,
}

impl Default for SwitchGameInfo {
    fn default() -> Self {
        Self {
            file_path: None,
            format: "Unknown".to_string(),
            file_size: 0,
            file_size_formatted: "0 B".to_string(),
            title_name: "Unknown Game".to_string(),
            publisher: "Unknown Publisher".to_string(),
            display_version: "1.0.0".to_string(),
            title_id: "Unknown".to_string(),
            icon_base64: None,
            files_list: Vec::new(),
            has_keys: false,
            is_compressed: false,
            has_ticket: false,
            ticket_decrypted: false,
            is_update: false,
            requires_base_game: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlcSummary {
    pub name: String,
    pub title_id: String,
    pub file_path: String,
    pub romfs_count: usize,
    pub nca_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionSummary {
    pub ncas: Vec<String>,
    pub romfs: Vec<String>,
    pub exefs: Vec<String>,
    pub metadata: Vec<String>,
    pub romfs_error: Option<String>,
    pub exefs_error: Option<String>,
    #[serde(default)]
    pub dlcs: Vec<DlcSummary>,
}

pub fn format_file_size(size: u64) -> String {
    if size >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if size >= 1024 * 1024 {
        format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else {
        format!("{} B", size)
    }
}

pub struct SwitchExtractor {
    pub keyset: KeySet,
}

impl SwitchExtractor {
    pub fn new(keyset: Option<KeySet>) -> Self {
        let keyset = keyset.unwrap_or_else(KeySet::auto_load);
        Self { keyset }
    }

    pub fn detect_format(&self, file_path: &Path) -> Result<String> {
        let ext = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut file = match File::open(file_path) {
            Ok(f) => f,
            Err(_) => return Ok("Unknown".to_string()),
        };

        let mut magic_16 = [0u8; 16];
        let bytes_read = file.read(&mut magic_16).unwrap_or(0);
        if bytes_read < 4 {
            return Ok("Unknown".to_string());
        }

        // Check PFS0
        if &magic_16[..4] == b"PFS0" {
            if ext == "nsz" {
                return Ok("NSZ".to_string());
            }
            if let Ok(pfs) = Pfs0::open(file_path, 0)
                && pfs.entries.iter().any(|e| e.name.to_lowercase().ends_with(".ncz")) {
                    return Ok("NSZ".to_string());
                }
            return Ok(if ext == "nsp" { "NSP".to_string() } else { "PFS0".to_string() });
        }

        // Check HFS0
        if &magic_16[..4] == b"HFS0" {
            return Ok("HFS0".to_string());
        }

        // Check IVFC / RomFS
        if &magic_16[..4] == b"IVFC" {
            return Ok("ROMFS".to_string());
        }
        if bytes_read >= 8 {
            let mut cur = std::io::Cursor::new(&magic_16[..8]);
            if let Ok(hdr_sz) = cur.read_u64::<LittleEndian>()
                && hdr_sz == 0x50 {
                    return Ok("ROMFS".to_string());
                }
        }

        // Check XCI (HEAD magic at 0x100 or 0x1100)
        let mut check_xci = || -> bool {
            if file.seek(SeekFrom::Start(0x100)).is_ok() {
                let mut m = [0u8; 4];
                if file.read_exact(&mut m).is_ok() && &m == b"HEAD" {
                    return true;
                }
            }
            if file.seek(SeekFrom::Start(0x1100)).is_ok() {
                let mut m = [0u8; 4];
                if file.read_exact(&mut m).is_ok() && &m == b"HEAD" {
                    return true;
                }
            }
            false
        };

        if check_xci() {
            if ext == "xcz" {
                return Ok("XCZ".to_string());
            }
            if let Ok(xci) = Xci::open(file_path)
                && let Some(sec) = xci.secure_partition()
                    && sec.entries.iter().any(|e| e.name.to_lowercase().ends_with(".ncz")) {
                        return Ok("XCZ".to_string());
                    }
            return Ok("XCI".to_string());
        }

        // Check NCZ (magic at 0x4008)
        if ext == "ncz" {
            return Ok("NCZ".to_string());
        }
        if file.seek(SeekFrom::Start(0x4008)).is_ok() {
            let mut sec_magic = [0u8; 8];
            if file.read_exact(&mut sec_magic).is_ok() && &sec_magic == b"NCZSECTN" {
                return Ok("NCZ".to_string());
            }
        }

        // Check NCA
        if ext == "nca" {
            return Ok("NCA".to_string());
        }
        if let Ok(nca) = Nca::open(file_path, self.keyset.clone(), 0)
            && matches!(&nca.magic, b"NCA3" | b"NCA2" | b"NCA0") {
                return Ok("NCA".to_string());
            }

        if !ext.is_empty() {
            Ok(ext.to_uppercase())
        } else {
            Ok("Unknown".to_string())
        }
    }

    pub fn load_container_tickets(&mut self, path: &Path, fmt: &str) -> usize {
        let mut loaded = 0;
        if matches!(fmt, "NSP" | "NSZ" | "PFS0") {
            if let Ok(mut pfs) = Pfs0::open(path, 0) {
                let tik_entries: Vec<_> = pfs
                    .entries
                    .iter()
                    .filter(|e| e.name.to_lowercase().ends_with(".tik"))
                    .cloned()
                    .collect();

                for ent in tik_entries {
                    if let Ok(tik_data) = pfs.read_entry(&ent)
                        && self.keyset.decrypt_ticket_title_key(&tik_data).is_some() {
                            loaded += 1;
                        }
                }
            }
        } else if matches!(fmt, "XCI" | "XCZ")
            && let Ok(mut xci) = Xci::open(path)
                && let Some(sec) = xci.secure_partition().cloned() {
                    for ent in sec.entries {
                        if ent.name.to_lowercase().ends_with(".tik") {
                            xci.file_mut().seek(SeekFrom::Start(ent.abs_offset)).ok();
                            let mut tik_data = vec![0u8; ent.size as usize];
                            if xci.file_mut().read_exact(&mut tik_data).is_ok()
                                && self.keyset.decrypt_ticket_title_key(&tik_data).is_some() {
                                    loaded += 1;
                                }
                        }
                    }
                }
        loaded
    }

    pub fn inspect(&mut self, file_path: &Path) -> Result<SwitchGameInfo> {
        let metadata = std::fs::metadata(file_path)?;
        let file_size = metadata.len();
        let fmt = self.detect_format(file_path)?;

        let mut info = SwitchGameInfo {
            file_path: Some(file_path.to_string_lossy().to_string()),
            format: fmt.clone(),
            file_size,
            file_size_formatted: format_file_size(file_size),
            has_keys: self.keyset.has_header_key(),
            is_compressed: matches!(fmt.as_str(), "NSZ" | "XCZ" | "NCZ"),
            ..Default::default()
        };

        // Load any tickets before proceeding
        let tik_count = self.load_container_tickets(file_path, &fmt);
        if tik_count > 0 {
            info.has_ticket = true;
            info.ticket_decrypted = true;
        }

        match fmt.as_str() {
            "NSP" | "NSZ" => self.inspect_nsp(file_path, &mut info)?,
            "XCI" | "XCZ" => self.inspect_xci(file_path, &mut info)?,
            "NCA" => self.inspect_nca(file_path, &mut info)?,
            "PFS0" | "HFS0" => {
                if let Ok(pfs) = Pfs0::open(file_path, 0) {
                    info.files_list = pfs
                        .entries
                        .iter()
                        .map(|e| format!("{} ({})", e.name, format_file_size(e.size)))
                        .collect();
                }
            }
            "ROMFS" => {
                if let Ok(romfs) = RomFs::open(file_path, 0) {
                    info.files_list = romfs
                        .files
                        .iter()
                        .take(150)
                        .map(|f| format!("{} ({})", f.full_path, format_file_size(f.size)))
                        .collect();
                }
            }
            _ => {}
        }

        if let Ok(Some((p_off, p_sz))) = self.find_program_nca(file_path, &fmt)
            && let Ok(mut f) = File::open(file_path)
            && let Ok(nca) = Nca::new(&mut f, self.keyset.clone(), p_off, p_sz)
        {
            if nca.sections.iter().any(|s| s.crypt_type == 4) {
                info.requires_base_game = true;
                info.is_update = true;
            }
        }
        if info.title_id.ends_with("800") {
            info.is_update = true;
        }

        Ok(info)
    }

    fn inspect_nsp(&mut self, path: &Path, info: &mut SwitchGameInfo) -> Result<()> {
        let pfs = Pfs0::open(path, 0)?;
        info.files_list = pfs
            .entries
            .iter()
            .map(|e| format!("{} ({})", e.name, format_file_size(e.size)))
            .collect();

        let mut control_entry = None;
        for e in &pfs.entries {
            let ename = e.name.to_lowercase();
            if ename.ends_with(".control.nca") || (ename.ends_with(".nca") && ename.contains("control")) {
                control_entry = Some(e.clone());
                break;
            }
        }

        if control_entry.is_none() && self.keyset.has_header_key() {
            for e in &pfs.entries {
                if e.name.to_lowercase().ends_with(".nca") && e.size < 60 * 1024 * 1024 {
                    let mut file = File::open(path)?;
                    if let Ok(nca) = Nca::new(&mut file, self.keyset.clone(), e.abs_offset, e.size)
                        && nca.content_type == 2 {
                            control_entry = Some(e.clone());
                            break;
                        }
                }
            }
        }

        if let Some(entry) = control_entry {
            self.extract_control_metadata(path, entry.abs_offset, entry.size, info);
        }

        Ok(())
    }

    fn inspect_xci(&mut self, path: &Path, info: &mut SwitchGameInfo) -> Result<()> {
        let xci = Xci::open(path)?;
        let mut files = Vec::new();

        if let Some(sec) = xci.secure_partition() {
            for e in &sec.entries {
                files.push(format!("secure/{} ({})", e.name, format_file_size(e.size)));
            }
        }

        for (part_name, part) in &xci.partitions {
            if part_name != "secure" {
                for e in &part.entries {
                    files.push(format!("{}/{} ({})", part_name, e.name, format_file_size(e.size)));
                }
            }
        }
        info.files_list = files;

        if let Some(sec) = xci.secure_partition() {
            let mut control_entry = None;
            for e in &sec.entries {
                let ename = e.name.to_lowercase();
                if ename.ends_with(".control.nca") || (ename.ends_with(".nca") && ename.contains("control")) {
                    control_entry = Some(e.clone());
                    break;
                }
            }

            if control_entry.is_none() && self.keyset.has_header_key() {
                for e in &sec.entries {
                    if e.name.to_lowercase().ends_with(".nca") && e.size < 60 * 1024 * 1024 {
                        let mut file = File::open(path)?;
                        if let Ok(nca) = Nca::new(&mut file, self.keyset.clone(), e.abs_offset, e.size)
                            && nca.content_type == 2 {
                                control_entry = Some(e.clone());
                                break;
                            }
                    }
                }
            }

            if let Some(entry) = control_entry {
                self.extract_control_metadata(path, entry.abs_offset, entry.size, info);
            }
        }

        Ok(())
    }

    fn inspect_nca(&mut self, path: &Path, info: &mut SwitchGameInfo) -> Result<()> {
        if let Ok(nca) = Nca::open(path, self.keyset.clone(), 0) {
            info.title_id = nca.title_id_hex();
            info.title_name = format!("{} Archive ({})", nca.content_type_name(), nca.title_id_hex());
            info.files_list = nca
                .sections
                .iter()
                .map(|s| {
                    format!(
                        "Section {} [{} / {}] ({} bytes)",
                        s.index,
                        s.fs_type_name(),
                        s.crypt_type_name(),
                        s.size
                    )
                })
                .collect();

            if nca.content_type == 2 {
                let sz = std::fs::metadata(path)?.len();
                self.extract_control_metadata(path, 0, sz, info);
            }
        } else {
            info.files_list = vec!["Encrypted NCA (Header key required to inspect)".to_string()];
        }
        Ok(())
    }

    fn extract_control_metadata(&mut self, path: &Path, nca_offset: u64, nca_size: u64, info: &mut SwitchGameInfo) {
        if !self.keyset.has_header_key() {
            return;
        }

        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let nca = match Nca::new(&mut file, self.keyset.clone(), nca_offset, nca_size) {
            Ok(n) => n,
            Err(_) => return,
        };

        info.title_id = nca.title_id_hex();

        let romfs_sec = match nca.sections.iter().find(|s| s.fs_type == 0) {
            Some(s) => s.clone(),
            None => return,
        };

        let key = nca.get_section_key(&romfs_sec);
        let mut reader = NcaSectionReader::new(&mut file, nca_offset, romfs_sec, key);

        let mut romfs = match RomFs::new(&mut reader, 0) {
            Ok(r) => r,
            Err(_) => return,
        };

        if let Some(nacp_file) = romfs.get_file("control.nacp").cloned()
            && let Ok(nacp_bytes) = romfs.read_file(&nacp_file)
                && let Some(nacp) = Nacp::parse(&nacp_bytes) {
                    info.title_name = nacp.get_preferred_title();
                    info.publisher = nacp.get_preferred_publisher();
                    if !nacp.display_version.is_empty() {
                        info.display_version = nacp.display_version.clone();
                    }
                    if nacp.title_id != 0 {
                        info.title_id = nacp.title_id_hex();
                    }
                }

        let preferred_icons = [
            "icon_americanenglish.dat",
            "icon_britishenglish.dat",
            "icon_japanese.dat",
        ];

        let mut icon_data: Option<Vec<u8>> = None;
        for &pref in &preferred_icons {
            if let Some(icon_entry) = romfs.get_file(pref).cloned()
                && let Ok(data) = romfs.read_file(&icon_entry) {
                    icon_data = Some(data);
                    break;
                }
        }

        if icon_data.is_none() {
            let candidate_files = romfs.files.clone();
            for f in &candidate_files {
                let fname = f.name.to_lowercase();
                if fname.starts_with("icon_") && fname.ends_with(".dat")
                    && let Ok(data) = romfs.read_file(f) {
                        icon_data = Some(data);
                        break;
                    }
            }
        }

        if let Some(data) = icon_data {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            info.icon_base64 = Some(format!("data:image/jpeg;base64,{}", b64));
        }
    }

    pub fn extract_container<F, C>(
        &mut self,
        file_path: &Path,
        out_dir: &Path,
        decompress_ncz: bool,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<Vec<PathBuf>>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        std::fs::create_dir_all(out_dir)?;
        let fmt = self.detect_format(file_path)?;
        let mut extracted = Vec::new();

        if matches!(fmt.as_str(), "NSP" | "NSZ" | "PFS0") {
            let mut pfs = Pfs0::open(file_path, 0)?;
            let total = pfs.entries.len();

            for (idx, entry) in pfs.entries.clone().iter().enumerate() {
                let is_ncz = entry.name.to_lowercase().ends_with(".ncz");
                let target_name = if is_ncz && decompress_ncz {
                    format!("{}.nca", &entry.name[..entry.name.len() - 4])
                } else {
                    entry.name.clone()
                };

                let out_file = out_dir.join(&target_name);

                let cb = |done: u64, tot: u64| {
                    if let Some(p) = progress_cb {
                        p(idx + 1, total, &target_name, done, tot);
                    }
                };

                if is_ncz && decompress_ncz {
                    let mut decomp = NczDecompressor::open(file_path, entry.abs_offset)?;
                    decomp.decompress(&out_file, true, Some(&cb), cancel_check)?;
                } else {
                    pfs.extract_entry(entry, &out_file, Some(&cb), cancel_check)?;
                }

                extracted.push(out_file);
            }
        } else if matches!(fmt.as_str(), "XCI" | "XCZ") {
            let xci = Xci::open(file_path)?;
            let sec = xci.secure_partition().cloned().ok_or_else(|| {
                ExtractorError::Format("XCI does not contain a secure partition.".to_string())
            })?;

            let total = sec.entries.len();
            for (idx, entry) in sec.entries.iter().enumerate() {
                let is_ncz = entry.name.to_lowercase().ends_with(".ncz");
                let target_name = if is_ncz && decompress_ncz {
                    format!("{}.nca", &entry.name[..entry.name.len() - 4])
                } else {
                    entry.name.clone()
                };

                let out_file = out_dir.join(&target_name);

                let cb = |done: u64, tot: u64| {
                    if let Some(p) = progress_cb {
                        p(idx + 1, total, &target_name, done, tot);
                    }
                };

                if is_ncz && decompress_ncz {
                    let mut decomp = NczDecompressor::open(file_path, entry.abs_offset)?;
                    decomp.decompress(&out_file, true, Some(&cb), cancel_check)?;
                } else {
                    let mut in_file = File::open(file_path)?;
                    in_file.seek(SeekFrom::Start(entry.abs_offset))?;
                    let mut out = File::create(&out_file)?;
                    let mut remaining = entry.size;
                    let mut written = 0u64;
                    let mut buf = vec![0u8; 1024 * 1024];

                    while remaining > 0 {
                        if let Some(cancel) = cancel_check
                            && cancel() {
                                drop(out);
                                let _ = std::fs::remove_file(&out_file);
                                return Err(ExtractorError::Cancelled);
                            }
                        let to_read = (remaining as usize).min(1024 * 1024);
                        in_file.read_exact(&mut buf[..to_read])?;
                        out.write_all(&buf[..to_read])?;
                        remaining -= to_read as u64;
                        written += to_read as u64;
                        cb(written, entry.size);
                    }
                }

                extracted.push(out_file);
            }
        } else if fmt == "NCZ" {
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unnamed");
            let out_file = out_dir.join(format!("{}.nca", stem));
            let mut decomp = NczDecompressor::open(file_path, 0)?;
            let cb = |done: u64, tot: u64| {
                if let Some(p) = progress_cb {
                    p(1, 1, out_file.to_string_lossy().as_ref(), done, tot);
                }
            };
            decomp.decompress(&out_file, true, Some(&cb), cancel_check)?;
            extracted.push(out_file);
        } else if fmt == "NCA" {
            let mut nca = Nca::open(file_path, self.keyset.clone(), 0)?;
            let secs = nca.sections.clone();
            for sec in &secs {
                let sec_file = out_dir.join(format!("section_{}_{}.bin", sec.index, sec.fs_type_name().to_lowercase()));
                let cb = |done: u64, tot: u64| {
                    if let Some(p) = progress_cb {
                        p(sec.index + 1, secs.len(), sec_file.to_string_lossy().as_ref(), done, tot);
                    }
                };
                nca.extract_section_raw(sec, &sec_file, true, Some(&cb), cancel_check)?;
                extracted.push(sec_file);
            }
        }

        Ok(extracted)
    }

    pub fn extract_romfs<F, C>(
        &mut self,
        file_path: &Path,
        out_dir: &Path,
        base_file_path: Option<&Path>,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<Vec<PathBuf>>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        if !self.keyset.has_header_key() {
            return Err(ExtractorError::MissingHeaderKey);
        }

        let fmt = self.detect_format(file_path)?;
        let romfs_dir = out_dir.join("romfs");
        std::fs::create_dir_all(&romfs_dir)?;

        self.load_container_tickets(file_path, &fmt);

        if fmt == "ROMFS" {
            let mut romfs = RomFs::open(file_path, 0)?;
            return romfs.extract_all(&romfs_dir, progress_cb, cancel_check);
        }

        let (patch_offset, patch_size) = self.find_program_nca(file_path, &fmt)?.ok_or_else(|| {
            ExtractorError::Format("Could not locate Program NCA in this package.".to_string())
        })?;

        let mut patch_file = File::open(file_path)?;
        let patch_nca = Nca::new(&mut patch_file, self.keyset.clone(), patch_offset, patch_size)?;

        let romfs_sec = patch_nca
            .sections
            .iter()
            .find(|s| s.fs_type == 0)
            .cloned()
            .ok_or_else(|| {
                ExtractorError::Format("Program NCA does not contain a RomFS section.".to_string())
            })?;

        if romfs_sec.crypt_type == 4 {
            let raw_header = &romfs_sec.raw_fs_header;
            if raw_header.len() < 0x140 {
                return Err(ExtractorError::Format("BKTR FS header is truncated.".to_string()));
            }

            let reloc_off = u64::from_le_bytes(raw_header[0x100..0x108].try_into().unwrap());
            let reloc_sz = u64::from_le_bytes(raw_header[0x108..0x110].try_into().unwrap());
            let subsec_off = u64::from_le_bytes(raw_header[0x120..0x128].try_into().unwrap());
            let subsec_sz = u64::from_le_bytes(raw_header[0x128..0x130].try_into().unwrap());

            let patch_key = patch_nca.get_section_key(&romfs_sec);

            let mut patch_f_for_reloc = File::open(file_path)?;
            let mut raw_reader = NcaSectionReader::new_raw(
                &mut patch_f_for_reloc,
                patch_offset,
                romfs_sec.clone(),
                patch_key,
            );
            raw_reader.seek(SeekFrom::Start(reloc_off))?;
            let mut reloc_data = vec![0u8; reloc_sz as usize];
            raw_reader.read_exact(&mut reloc_data)?;
            let reloc_block = crate::bktr::BktrRelocationBlock::parse(&reloc_data)?;

            raw_reader.seek(SeekFrom::Start(subsec_off))?;
            let mut subsec_data = vec![0u8; subsec_sz as usize];
            raw_reader.read_exact(&mut subsec_data)?;
            let subsec_block = crate::bktr::BktrSubsectionBlock::parse(&subsec_data)?;

            let (base_reader, base_nca_off, base_sec_off, base_sec_ctr, base_key, base_crypt_type) =
                if let Some(base_p) = base_file_path {
                    let base_fmt = self.detect_format(base_p)?;
                    self.load_container_tickets(base_p, &base_fmt);

                    if base_fmt == "ROMFS" {
                        let base_f = File::open(base_p)?;
                        (Some(base_f), 0u64, 0u64, [0u8; 8], None, 1u8)
                    } else {
                        let (b_nca_off, b_nca_sz) = self.find_program_nca(base_p, &base_fmt)?.ok_or_else(|| {
                            ExtractorError::Format("Could not locate Program NCA in Base Game package.".to_string())
                        })?;
                        let mut b_file = File::open(base_p)?;
                        let base_nca = Nca::new(&mut b_file, self.keyset.clone(), b_nca_off, b_nca_sz)?;
                        let b_romfs_sec = base_nca
                            .sections
                            .iter()
                            .find(|s| s.fs_type == 0)
                            .cloned()
                            .ok_or_else(|| {
                                ExtractorError::Format("Base Game Program NCA does not contain a RomFS section.".to_string())
                            })?;

                        let b_key = base_nca.get_section_key(&b_romfs_sec);
                        let base_f = File::open(base_p)?;
                        (
                            Some(base_f),
                            b_nca_off,
                            b_romfs_sec.offset,
                            b_romfs_sec.section_ctr,
                            b_key,
                            b_romfs_sec.crypt_type,
                        )
                    }
                } else {
                    (None, 0, 0, [0u8; 8], None, 1)
                };

            let patch_f_for_bktr = File::open(file_path)?;
            let bktr_reader = crate::bktr::BktrReader::new(
                base_reader,
                patch_f_for_bktr,
                base_nca_off,
                base_sec_off,
                base_sec_ctr,
                base_key,
                base_crypt_type,
                patch_offset,
                romfs_sec.offset,
                romfs_sec.section_ctr,
                patch_key,
                reloc_block,
                subsec_block,
            );

            let mut romfs = RomFs::new(bktr_reader, romfs_sec.content_offset)?;
            return romfs.extract_all(&romfs_dir, progress_cb, cancel_check);
        }

        let key = patch_nca.get_section_key(&romfs_sec);
        let mut reader = NcaSectionReader::new(&mut patch_file, patch_offset, romfs_sec, key);

        let mut romfs = RomFs::new(&mut reader, 0)?;
        romfs.extract_all(&romfs_dir, progress_cb, cancel_check)
    }

    pub fn extract_exefs<F, C>(
        &mut self,
        file_path: &Path,
        out_dir: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<Vec<PathBuf>>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        if !self.keyset.has_header_key() {
            return Err(ExtractorError::MissingHeaderKey);
        }

        let fmt = self.detect_format(file_path)?;
        let exefs_dir = out_dir.join("exefs");
        std::fs::create_dir_all(&exefs_dir)?;

        self.load_container_tickets(file_path, &fmt);

        let (offset, size) = self.find_program_nca(file_path, &fmt)?.ok_or_else(|| {
            ExtractorError::Format("Could not locate Program NCA in this package.".to_string())
        })?;

        let mut file = File::open(file_path)?;
        let nca = Nca::new(&mut file, self.keyset.clone(), offset, size)?;

        let exefs_sec = nca
            .sections
            .iter()
            .find(|s| s.fs_type == 1)
            .cloned()
            .ok_or_else(|| {
                ExtractorError::Format("Program NCA does not contain an ExeFS section.".to_string())
            })?;

        let key = nca.get_section_key(&exefs_sec);
        let mut reader = NcaSectionReader::new(&mut file, offset, exefs_sec, key);

        let mut pfs0 = Pfs0::new(&mut reader, 0)?;
        pfs0.extract_all(&exefs_dir, progress_cb, cancel_check)
    }

    pub fn extract_dlc<F, C>(
        &mut self,
        dlc_path: &Path,
        dlc_out_dir: &Path,
        extract_container_ncas: bool,
        extract_romfs_assets: bool,
        extract_metadata_icon: bool,
        stage_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<DlcSummary>
    where
        F: Fn(&str, usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        std::fs::create_dir_all(dlc_out_dir)?;
        let fmt = self.detect_format(dlc_path)?;
        self.load_container_tickets(dlc_path, &fmt);

        let dlc_info = self.inspect(dlc_path).unwrap_or_else(|_| {
            let mut info = SwitchGameInfo::default();
            info.title_name = dlc_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "DLC".to_string());
            info
        });

        let mut dlc_summary = DlcSummary {
            name: dlc_info.title_name.clone(),
            title_id: dlc_info.title_id.clone(),
            file_path: dlc_path.to_string_lossy().to_string(),
            romfs_count: 0,
            nca_count: 0,
            error: None,
        };

        if extract_metadata_icon {
            let meta_dir = dlc_out_dir.join("metadata");
            let _ = std::fs::create_dir_all(&meta_dir);
            let info_json_path = meta_dir.join("dlc_info.json");
            if let Ok(json_str) = serde_json::to_string_pretty(&dlc_info) {
                let _ = std::fs::write(&info_json_path, json_str);
            }
        }

        if extract_container_ncas {
            let nca_dir = dlc_out_dir.join("ncas");
            let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
                if let Some(scb) = stage_cb {
                    scb(&format!("DLC [{}]: NCAs", dlc_info.title_name), cur, tot, name, done, sz);
                }
            };
            match self.extract_container(dlc_path, &nca_dir, true, Some(&cb), cancel_check) {
                Ok(extracted) => dlc_summary.nca_count = extracted.len(),
                Err(e) => dlc_summary.error = Some(format!("NCA extraction error: {}", e)),
            }
        }

        if extract_romfs_assets && self.keyset.has_header_key() {
            let romfs_dir = dlc_out_dir.join("romfs");
            let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
                if let Some(scb) = stage_cb {
                    scb(&format!("DLC [{}]: RomFS", dlc_info.title_name), cur, tot, name, done, sz);
                }
            };

            if matches!(fmt.as_str(), "NSP" | "NSZ") {
                if let Ok(pfs) = Pfs0::open(dlc_path, 0) {
                    for entry in &pfs.entries {
                        let name_low = entry.name.to_lowercase();
                        if (name_low.ends_with(".nca") || name_low.ends_with(".ncz")) && entry.size > 0x1000 {
                            let mut f = match File::open(dlc_path) {
                                Ok(f) => f,
                                Err(_) => continue,
                            };
                            let nca = match Nca::new(&mut f, self.keyset.clone(), entry.abs_offset, entry.size) {
                                Ok(n) => n,
                                Err(_) => continue,
                            };
                            if let Some(romfs_sec) = nca.sections.iter().find(|s| s.fs_type == 0).cloned() {
                                let key = nca.get_section_key(&romfs_sec);
                                let mut reader = NcaSectionReader::new(&mut f, entry.abs_offset, romfs_sec, key);
                                if let Ok(mut romfs) = RomFs::new(&mut reader, 0) {
                                    if let Ok(files) = romfs.extract_all(&romfs_dir, Some(&cb), cancel_check) {
                                        dlc_summary.romfs_count += files.len();
                                    }
                                }
                            }
                        }
                    }
                }
            } else if fmt == "XCI" {
                if let Ok(xci) = Xci::open(dlc_path) {
                    if let Some(sec) = xci.secure_partition() {
                        for entry in &sec.entries {
                            let name_low = entry.name.to_lowercase();
                            if name_low.ends_with(".nca") && entry.size > 0x1000 {
                                let mut f = match File::open(dlc_path) {
                                    Ok(f) => f,
                                    Err(_) => continue,
                                };
                                let nca = match Nca::new(&mut f, self.keyset.clone(), entry.abs_offset, entry.size) {
                                    Ok(n) => n,
                                    Err(_) => continue,
                                };
                                if let Some(romfs_sec) = nca.sections.iter().find(|s| s.fs_type == 0).cloned() {
                                    let key = nca.get_section_key(&romfs_sec);
                                    let mut reader = NcaSectionReader::new(&mut f, entry.abs_offset, romfs_sec, key);
                                    if let Ok(mut romfs) = RomFs::new(&mut reader, 0) {
                                        if let Ok(files) = romfs.extract_all(&romfs_dir, Some(&cb), cancel_check) {
                                            dlc_summary.romfs_count += files.len();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if fmt == "NCA" {
                let sz = std::fs::metadata(dlc_path).map(|m| m.len()).unwrap_or(0);
                if let Ok(mut f) = File::open(dlc_path) {
                    if let Ok(nca) = Nca::new(&mut f, self.keyset.clone(), 0, sz) {
                        if let Some(romfs_sec) = nca.sections.iter().find(|s| s.fs_type == 0).cloned() {
                            let key = nca.get_section_key(&romfs_sec);
                            let mut reader = NcaSectionReader::new(&mut f, 0, romfs_sec, key);
                            if let Ok(mut romfs) = RomFs::new(&mut reader, 0) {
                                if let Ok(files) = romfs.extract_all(&romfs_dir, Some(&cb), cancel_check) {
                                    dlc_summary.romfs_count += files.len();
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(dlc_summary)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn extract_all<F, C>(
        &mut self,
        file_path: &Path,
        out_dir: &Path,
        base_file_path: Option<&Path>,
        update_file_path: Option<&Path>,
        dlc_file_paths: &[PathBuf],
        extract_container_ncas: bool,
        extract_romfs_assets: bool,
        extract_exefs_binaries: bool,
        extract_metadata_icon: bool,
        stage_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<ExtractionSummary>
    where
        F: Fn(&str, usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        std::fs::create_dir_all(out_dir)?;

        let mut summary = ExtractionSummary {
            ncas: Vec::new(),
            romfs: Vec::new(),
            exefs: Vec::new(),
            metadata: Vec::new(),
            romfs_error: None,
            exefs_error: None,
            dlcs: Vec::new(),
        };

        // Determine if we have an Update package and a Base Game package
        // Priority 1: update_file_path is explicitly passed
        // Priority 2: file_path is an update and base_file_path is passed
        let (actual_base_path, actual_update_path) = if let Some(up) = update_file_path {
            (Some(file_path), Some(up))
        } else if let Ok(info) = self.inspect(file_path) && (info.is_update || info.requires_base_game) && base_file_path.is_some() {
            (base_file_path, Some(file_path))
        } else {
            (Some(file_path), None)
        };

        let primary_path = actual_update_path.unwrap_or(file_path);
        let base_ref_path = actual_base_path;

        let fmt = self.detect_format(primary_path)?;
        self.load_container_tickets(primary_path, &fmt);
        if let Some(bp) = base_ref_path {
            if let Ok(bfmt) = self.detect_format(bp) {
                self.load_container_tickets(bp, &bfmt);
            }
        }

        // 1. Metadata and Icon
        if extract_metadata_icon {
            let meta_dir = out_dir.join("metadata");
            let _ = std::fs::create_dir_all(&meta_dir);

            if let Ok(info) = self.inspect(primary_path) {
                if let Some(ref icon_b64) = info.icon_base64
                    && let Some(idx) = icon_b64.find(',')
                    && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&icon_b64[idx + 1..]) {
                    let icon_path = meta_dir.join("icon.jpg");
                    if std::fs::write(&icon_path, bytes).is_ok() {
                        summary.metadata.push(icon_path.to_string_lossy().to_string());
                    }
                }

                let info_json_path = meta_dir.join("game_info.json");
                if let Ok(json_str) = serde_json::to_string_pretty(&info)
                    && std::fs::write(&info_json_path, json_str).is_ok() {
                    summary.metadata.push(info_json_path.to_string_lossy().to_string());
                }
            }
        }

        // 2. Container Unpack (.nca files)
        if extract_container_ncas {
            let nca_dir = out_dir.join("ncas");
            let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
                if let Some(scb) = stage_cb {
                    scb("Extracting NCAs", cur, tot, name, done, sz);
                }
            };

            let extracted = self.extract_container(primary_path, &nca_dir, true, Some(&cb), cancel_check)?;
            summary.ncas = extracted.into_iter().map(|p| p.to_string_lossy().to_string()).collect();

            // Also unpack base game NCAs if an update was merged
            if let Some(bp) = base_ref_path && actual_update_path.is_some() && bp != primary_path {
                let base_nca_dir = out_dir.join("ncas_base");
                let _ = self.extract_container(bp, &base_nca_dir, true, Some(&cb), cancel_check);
            }
        }

        // 3. RomFS Assets (BKTR merged if update present)
        if extract_romfs_assets {
            if !self.keyset.has_header_key() {
                summary.romfs_error = Some("Skipped: prod.keys with header_key not found.".to_string());
            } else {
                let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
                    if let Some(scb) = stage_cb {
                        scb("Extracting RomFS Assets", cur, tot, name, done, sz);
                    }
                };

                match self.extract_romfs(primary_path, out_dir, base_ref_path, Some(&cb), cancel_check) {
                    Ok(paths) => {
                        summary.romfs = paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
                    }
                    Err(e) => {
                        summary.romfs_error = Some(e.to_string());
                    }
                }
            }
        }

        // 4. ExeFS Binaries
        if extract_exefs_binaries {
            if !self.keyset.has_header_key() {
                summary.exefs_error = Some("Skipped: prod.keys with header_key not found.".to_string());
            } else {
                let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
                    if let Some(scb) = stage_cb {
                        scb("Extracting ExeFS Binaries", cur, tot, name, done, sz);
                    }
                };

                let mut exefs_res = self.extract_exefs(primary_path, out_dir, Some(&cb), cancel_check);
                // If update package had no ExeFS (or failed), fallback to base game ExeFS
                if exefs_res.is_err() && let Some(bp) = base_ref_path && bp != primary_path {
                    exefs_res = self.extract_exefs(bp, out_dir, Some(&cb), cancel_check);
                }

                match exefs_res {
                    Ok(paths) => {
                        summary.exefs = paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect();
                    }
                    Err(e) => {
                        summary.exefs_error = Some(e.to_string());
                    }
                }
            }
        }

        // 5. Extract all selected DLCs
        if !dlc_file_paths.is_empty() {
            let dlc_base_dir = out_dir.join("dlc");
            let _ = std::fs::create_dir_all(&dlc_base_dir);

            for (idx, dlc_p) in dlc_file_paths.iter().enumerate() {
                if let Some(cancel) = cancel_check && cancel() {
                    return Err(ExtractorError::Cancelled);
                }

                let dlc_stem = dlc_p.file_stem().and_then(|s| s.to_str()).unwrap_or("dlc");
                let safe_stem = dlc_stem.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
                let dlc_folder_name = format!("DLC_{:02}_{}", idx + 1, safe_stem);
                let dlc_out = dlc_base_dir.join(&dlc_folder_name);

                match self.extract_dlc(
                    dlc_p,
                    &dlc_out,
                    extract_container_ncas,
                    extract_romfs_assets,
                    extract_metadata_icon,
                    stage_cb,
                    cancel_check,
                ) {
                    Ok(dlc_summary) => {
                        summary.dlcs.push(dlc_summary);
                    }
                    Err(e) => {
                        summary.dlcs.push(DlcSummary {
                            name: dlc_stem.to_string(),
                            title_id: "Unknown".to_string(),
                            file_path: dlc_p.to_string_lossy().to_string(),
                            romfs_count: 0,
                            nca_count: 0,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }

        Ok(summary)
    }

    pub fn convert_xci_to_nsp<F, C>(
        &mut self,
        xci_path: &Path,
        output_nsp_path: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<u64>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        let mut xci = Xci::open(xci_path)?;
        xci.convert_to_nsp(output_nsp_path, progress_cb, cancel_check)
    }

    pub fn build_pfs0<F, C>(
        &mut self,
        files: &[(String, PathBuf)],
        output_nsp_path: &Path,
        progress_cb: Option<&F>,
        cancel_check: Option<&C>,
    ) -> Result<u64>
    where
        F: Fn(usize, usize, &str, u64, u64),
        C: Fn() -> bool,
    {
        Pfs0Builder::build(files, output_nsp_path, progress_cb, cancel_check)
    }

    pub fn find_program_nca(&mut self, path: &Path, fmt: &str) -> Result<Option<(u64, u64)>> {
        if matches!(fmt, "NSP" | "NSZ") {
            let pfs = Pfs0::open(path, 0)?;
            let mut nca_entries: Vec<_> = pfs
                .entries
                .iter()
                .filter(|e| {
                    let n = e.name.to_lowercase();
                    n.ends_with(".nca") || n.ends_with(".ncz")
                })
                .cloned()
                .collect();

            if nca_entries.is_empty() {
                return Ok(None);
            }

            nca_entries.sort_by_key(|b| std::cmp::Reverse(b.size));

            let mut file = File::open(path)?;
            for ent in &nca_entries {
                if let Ok(nca) = Nca::new(&mut file, self.keyset.clone(), ent.abs_offset, ent.size)
                    && nca.content_type == 0 {
                        return Ok(Some((ent.abs_offset, ent.size)));
                    }
            }

            let largest = &nca_entries[0];
            return Ok(Some((largest.abs_offset, largest.size)));
        } else if matches!(fmt, "XCI" | "XCZ") {
            let xci = Xci::open(path)?;
            let sec = match xci.secure_partition() {
                Some(s) => s,
                None => return Ok(None),
            };

            let mut nca_entries: Vec<_> = sec
                .entries
                .iter()
                .filter(|e| {
                    let n = e.name.to_lowercase();
                    n.ends_with(".nca") || n.ends_with(".ncz")
                })
                .cloned()
                .collect();

            if nca_entries.is_empty() {
                return Ok(None);
            }

            nca_entries.sort_by_key(|b| std::cmp::Reverse(b.size));

            let mut file = File::open(path)?;
            for ent in &nca_entries {
                if let Ok(nca) = Nca::new(&mut file, self.keyset.clone(), ent.abs_offset, ent.size)
                    && nca.content_type == 0 {
                        return Ok(Some((ent.abs_offset, ent.size)));
                    }
            }

            let largest = &nca_entries[0];
            return Ok(Some((largest.abs_offset, largest.size)));
        } else if fmt == "NCA" {
            let sz = std::fs::metadata(path)?.len();
            return Ok(Some((0, sz)));
        }

        Ok(None)
    }
}
