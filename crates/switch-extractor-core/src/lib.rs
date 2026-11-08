pub mod bktr;
pub mod engine;
pub mod error;
pub mod keys;
pub mod nacp;
pub mod nca;
pub mod ncz;
pub mod pfs0;
pub mod romfs;
pub mod xci;

pub use bktr::{BktrReader, BktrRelocationBlock, BktrSubsectionBlock};
pub use engine::{DlcSummary, ExtractionSummary, SwitchExtractor, SwitchGameInfo};
pub use error::{ExtractorError, Result};
pub use keys::KeySet;
pub use nacp::Nacp;
pub use nca::Nca;
pub use ncz::NczDecompressor;
pub use pfs0::{Pfs0, Pfs0Builder, Pfs0Entry};
pub use romfs::{RomFs, RomFsFileEntry};
pub use xci::Xci;

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::KeyInit;
    use aes::Aes128;
    use xts_mode::Xts128;

    fn get_tweak_big_endian(sector_num: u128) -> [u8; 16] {
        sector_num.to_be_bytes()
    }

    #[test]
    fn test_nca_xts_decryption() {
        let hkey = hex::decode("aeaab1ca08adf9bef12991f369e3c567d6881e4e4a6a47a51f6e4877062d542d").unwrap();
        let key1 = &hkey[..16];
        let key2 = &hkey[16..];

        let c1 = Aes128::new_from_slice(key1).unwrap();
        let c2 = Aes128::new_from_slice(key2).unwrap();
        let cipher = Xts128::new(c1, c2);

        // Raw 0x200 block of Odyssey NCA
        let raw_block = hex::decode("5d8e6b5d83c2776bce2af563c5dd0c3d").unwrap();
        let mut block_512 = vec![0u8; 0x200];
        block_512[..16].copy_from_slice(&raw_block);

        let mut test_be = block_512.clone();
        cipher.decrypt_area(&mut test_be, 0x200, 1, get_tweak_big_endian);
        assert_eq!(&test_be[..4], b"NCA3");
    }

    #[test]
    fn test_keyset_autoload() {
        let ks = KeySet::auto_load();
        assert!(ks.count() > 0);
        assert!(ks.has_header_key());
    }

    #[test]
    fn test_pfs0_pack_and_unpack() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("test1.txt");
        let file2 = dir.path().join("test2.bin");
        std::fs::write(&file1, b"Hello Nintendo Switch!").unwrap();
        std::fs::write(&file2, vec![0x42; 2048]).unwrap();

        let nsp_path = dir.path().join("game.nsp");
        let files = vec![
            ("test1.txt".to_string(), file1.clone()),
            ("test2.bin".to_string(), file2.clone()),
        ];

        let written = Pfs0Builder::build(&files, &nsp_path, None::<&fn(usize, usize, &str, u64, u64)>, None::<&fn() -> bool>).unwrap();
        assert!(written > 2048);

        let mut pfs = Pfs0::open(&nsp_path, 0).unwrap();
        assert_eq!(pfs.entries.len(), 2);
        assert_eq!(pfs.entries[0].name, "test1.txt");
        assert_eq!(pfs.entries[1].name, "test2.bin");

        let content1 = pfs.read_entry(&pfs.entries[0].clone()).unwrap();
        assert_eq!(content1, b"Hello Nintendo Switch!");
    }

    #[test]
    fn test_nacp_parsing() {
        let mut nacp_buf = vec![0u8; 0x4000];
        // Language 0: AmericanEnglish title
        let title = b"Super Mario Odyssey\0";
        nacp_buf[..title.len()].copy_from_slice(title);
        // Language 0: Publisher
        let pub_name = b"Nintendo\0";
        nacp_buf[0x200..0x200 + pub_name.len()].copy_from_slice(pub_name);
        // Version string at 0x3060
        let ver = b"1.3.0\0";
        nacp_buf[0x3060..0x3060 + ver.len()].copy_from_slice(ver);
        // Title ID at 0x3070
        let tid: u64 = 0x0100000000010000;
        nacp_buf[0x3070..0x3078].copy_from_slice(&tid.to_le_bytes());

        let nacp = Nacp::parse(&nacp_buf).expect("NACP parse failed");
        assert_eq!(nacp.get_preferred_title(), "Super Mario Odyssey");
        assert_eq!(nacp.get_preferred_publisher(), "Nintendo");
        assert_eq!(nacp.display_version, "1.3.0");
        assert_eq!(nacp.title_id_hex(), "0100000000010000");
    }

    #[test]
    fn test_ncz_reencryption_symmetry() {
        let sections = vec![crate::ncz::NczSection {
            offset: 0x4000,
            size: 0x1000,
            crypto_type: 2, // AES-CTR
            crypto_key: [0x11; 16],
            crypto_counter: [0x22; 16],
        }];

        let original = vec![0xAB; 0x1000];
        let mut buffer = original.clone();

        // Encrypt with apply_reencryption
        NczDecompressor::<()>::apply_reencryption(&sections, &mut buffer, 0x4000);
        assert_ne!(buffer, original);

        // Decrypt with apply_reencryption (AES-CTR is symmetric)
        NczDecompressor::<()>::apply_reencryption(&sections, &mut buffer, 0x4000);
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_engine_detect_format() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("dummy.nsp");
        let files = vec![("dummy.txt".to_string(), {
            let p = dir.path().join("dummy.txt");
            std::fs::write(&p, b"PFS0 test").unwrap();
            p
        })];
        Pfs0Builder::build(&files, &file1, None::<&fn(usize, usize, &str, u64, u64)>, None::<&fn() -> bool>).unwrap();

        let engine = SwitchExtractor::new(None);
        let fmt = engine.detect_format(&file1).unwrap();
        assert_eq!(fmt, "NSP");
    }

    #[test]
    fn test_inspect_real_update_bktr() {
        let p = std::path::Path::new(r"G:\SW GAMES\Super Mario Odyssey [0100000000010800][v393216](axekin.com).nsp");
        if !p.exists() {
            println!("Real update test file not found, skipping");
            return;
        }

        let mut engine = SwitchExtractor::new(None);
        let info = engine.inspect(p).unwrap();
        println!("Inspected update info: {:?}", info.title_name);
        println!("Files: {:?}", info.files_list);

        let (off, sz) = engine.find_program_nca(p, "NSP").unwrap().unwrap();
        let mut f = std::fs::File::open(p).unwrap();
        let nca = Nca::new(&mut f, engine.keyset.clone(), off, sz).unwrap();
        for sec in &nca.sections {
            println!(
                "Section {}: fs_type={}, crypt_type={}, offset=0x{:X}, size=0x{:X}, content_off=0x{:X}, content_sz=0x{:X}",
                sec.index, sec.fs_type, sec.crypt_type, sec.offset, sec.size, sec.content_offset, sec.content_size
            );
            if sec.crypt_type == 4 {
                println!("BKTR raw_fs_header len={}", sec.raw_fs_header.len());
                let h = &sec.raw_fs_header;
                println!("raw_fs_header[0x0..0x50]: {:02X?}", &h[0x0..0x50]);
                println!("raw_fs_header[0x100..0x150]: {:02X?}", &h[0x100..0x150]);
                let reloc_off = u64::from_le_bytes(h[0x100..0x108].try_into().unwrap());
                let reloc_sz = u64::from_le_bytes(h[0x108..0x110].try_into().unwrap());
                let reloc_magic = &h[0x110..0x114];
                let subsec_off = u64::from_le_bytes(h[0x120..0x128].try_into().unwrap());
                let subsec_sz = u64::from_le_bytes(h[0x128..0x130].try_into().unwrap());
                let subsec_magic = &h[0x130..0x134];
                println!("Reloc: off=0x{:X}, sz=0x{:X}, magic={:?}", reloc_off, reloc_sz, std::str::from_utf8(reloc_magic));
                println!("Subsec: off=0x{:X}, sz=0x{:X}, magic={:?}", subsec_off, subsec_sz, std::str::from_utf8(subsec_magic));

                let key = nca.get_section_key(sec).expect("Key missing");
                let mut f2 = std::fs::File::open(p).unwrap();
                let mut reader = crate::nca::NcaSectionReader::new_raw(&mut f2, off, sec.clone(), Some(key));
                use std::io::{Seek, SeekFrom, Read};
                reader.seek(SeekFrom::Start(reloc_off)).unwrap();
                let mut reloc_data = vec![0u8; reloc_sz as usize];
                reader.read_exact(&mut reloc_data).unwrap();

                reader.seek(SeekFrom::Start(subsec_off)).unwrap();
                let mut subsec_data = vec![0u8; subsec_sz as usize];
                reader.read_exact(&mut subsec_data).unwrap();

                let reloc_block = BktrRelocationBlock::parse(&reloc_data).unwrap();
                let subsec_block = BktrSubsectionBlock::parse(&subsec_data).unwrap();

                let bktr_reader = BktrReader::new(
                    None::<std::fs::File>,
                    f2,
                    0, 0, [0u8; 8], None, 0,
                    off, sec.offset, sec.section_ctr, Some(key),
                    reloc_block, subsec_block,
                );

                let romfs = RomFs::new(bktr_reader, sec.content_offset).unwrap();
                println!("SUCCESS! RomFs parsed! Files count: {}", romfs.files.len());
                for (i, file) in romfs.files.iter().take(10).enumerate() {
                    println!("  File #{}: {} ({} bytes)", i, file.full_path, file.size);
                }
            }
        }
    }

    #[test]
    fn test_bktr_bayonetta3_base_and_update_merge() {
        let base_p = std::path::Path::new(r"G:\SW GAMES\Bayonetta 3 [01004A4010FEA000][USA][v0](Dumped)(axekin.com)\Bayonetta 3 [01004A4010FEA000][USA][v0](Dumped)(axekin.fr).xci");
        let update_p = std::path::Path::new(r"G:\SW GAMES\Bayonetta 3 [01004A4010FEA000][USA][v0](Dumped)(axekin.com)\Bayonetta 3 [01004A4010FEA800][v131072](axekin.fr)\Bayonetta 3 [01004A4010FEA800][v131072](axekin.fr).nsp");

        if !base_p.exists() || !update_p.exists() {
            println!("Bayonetta 3 test files not found, skipping");
            return;
        }

        let mut engine = SwitchExtractor::new(None);
        let update_info = engine.inspect(update_p).unwrap();
        println!("Update info: title={}, is_update={}, requires_base={}", update_info.title_name, update_info.is_update, update_info.requires_base_game);
        assert!(update_info.is_update);
        assert!(update_info.requires_base_game);

        let (u_off, u_sz) = engine.find_program_nca(update_p, "NSP").unwrap().unwrap();
        let mut uf = std::fs::File::open(update_p).unwrap();
        let u_nca = Nca::new(&mut uf, engine.keyset.clone(), u_off, u_sz).unwrap();
        let u_romfs = u_nca.sections.iter().find(|s| s.fs_type == 0).unwrap();
        let u_key = u_nca.get_section_key(u_romfs).unwrap();

        let raw_h = &u_romfs.raw_fs_header;
        let reloc_off = u64::from_le_bytes(raw_h[0x100..0x108].try_into().unwrap());
        let reloc_sz = u64::from_le_bytes(raw_h[0x108..0x110].try_into().unwrap());
        let mut u_raw = crate::nca::NcaSectionReader::new_raw(&mut uf, u_off, u_romfs.clone(), Some(u_key));
        use std::io::{Seek, SeekFrom, Read};
        u_raw.seek(SeekFrom::Start(reloc_off)).unwrap();
        let mut reloc_data = vec![0u8; reloc_sz as usize];
        u_raw.read_exact(&mut reloc_data).unwrap();
        let reloc_block = crate::bktr::BktrRelocationBlock::parse(&reloc_data).unwrap();

        println!("Reloc block: total_size=0x{:X}", reloc_block.total_size);
        let target_virt = u_romfs.content_offset;
        let (reloc, next_v) = reloc_block.get_relocation(target_virt).unwrap();
        println!("Reloc at 0x{:X}: virt=0x{:X}, phys=0x{:X}, is_patch={}, next_virt=0x{:X}",
            target_virt, reloc.virt_offset, reloc.phys_offset, reloc.is_patch, next_v
        );

        let out_dir = tempfile::tempdir().unwrap();
        use std::sync::atomic::{AtomicUsize, Ordering};
        let extracted_count = AtomicUsize::new(0);

        let cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
            if cur <= 2 {
                println!("Extracting file #{}/{}: {} ({}/{} bytes)", cur, tot, name, done, sz);
            }
            extracted_count.store(cur, Ordering::SeqCst);
        };
        let cancel = || extracted_count.load(Ordering::SeqCst) >= 2;

        let romfs_res = engine.extract_romfs(
            update_p,
            out_dir.path(),
            Some(base_p),
            Some(&cb),
            Some(&cancel),
        );

        match romfs_res {
            Ok(paths) => {
                println!("SUCCESS! Merged RomFS extracted {} files!", paths.len());
                assert!(!paths.is_empty());
            }
            Err(crate::ExtractorError::Cancelled) => {
                println!("SUCCESS! Merged RomFS extraction confirmed working (cancelled after test threshold)!");
            }
            Err(e) => {
                panic!("Failed to extract merged RomFS: {}", e);
            }
        }
    }
}
