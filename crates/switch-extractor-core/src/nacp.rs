use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const LANGUAGE_NAMES: [&str; 16] = [
    "AmericanEnglish",
    "BritishEnglish",
    "Japanese",
    "French",
    "German",
    "LatinAmericanSpanish",
    "Spanish",
    "Italian",
    "Dutch",
    "CanadianFrench",
    "Portuguese",
    "Russian",
    "Korean",
    "TraditionalChinese",
    "SimplifiedChinese",
    "BrazilianPortuguese",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NacpLanguageEntry {
    pub language: String,
    pub title: String,
    pub publisher: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nacp {
    pub titles: HashMap<String, NacpLanguageEntry>,
    pub display_version: String,
    pub title_id: u64,
}

impl Nacp {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 0x3080 {
            return None;
        }

        let mut titles = HashMap::new();

        // 16 language blocks, 0x300 bytes each
        for (i, &lang_name) in LANGUAGE_NAMES.iter().enumerate() {
            let block_start = i * 0x300;
            let block_end = block_start + 0x300;
            if block_end > data.len() {
                break;
            }

            let block = &data[block_start..block_end];
            let raw_title = &block[..0x200];
            let raw_pub = &block[0x200..0x300];

            let title_len = raw_title.iter().position(|&b| b == 0).unwrap_or(0x200);
            let pub_len = raw_pub.iter().position(|&b| b == 0).unwrap_or(0x100);

            let title = String::from_utf8_lossy(&raw_title[..title_len]).trim().to_string();
            let pub_str = String::from_utf8_lossy(&raw_pub[..pub_len]).trim().to_string();

            if !title.is_empty() || !pub_str.is_empty() {
                titles.insert(
                    lang_name.to_string(),
                    NacpLanguageEntry {
                        language: lang_name.to_string(),
                        title,
                        publisher: pub_str,
                    },
                );
            }
        }

        // Version string at 0x3060 (16 bytes)
        let ver_raw = &data[0x3060..0x3070];
        let ver_len = ver_raw.iter().position(|&b| b == 0).unwrap_or(16);
        let display_version = String::from_utf8_lossy(&ver_raw[..ver_len]).trim().to_string();

        // Title ID at 0x3070 (8 bytes)
        let mut tid_cursor = std::io::Cursor::new(&data[0x3070..0x3078]);
        let title_id = tid_cursor.read_u64::<LittleEndian>().unwrap_or(0);

        Some(Self {
            titles,
            display_version,
            title_id,
        })
    }

    pub fn title_id_hex(&self) -> String {
        format!("{:016X}", self.title_id)
    }

    pub fn get_preferred_title(&self) -> String {
        for pref in ["AmericanEnglish", "BritishEnglish", "Japanese"] {
            if let Some(entry) = self.titles.get(pref)
                && !entry.title.is_empty() {
                    return entry.title.clone();
                }
        }
        for entry in self.titles.values() {
            if !entry.title.is_empty() {
                return entry.title.clone();
            }
        }
        "Unknown Title".to_string()
    }

    pub fn get_preferred_publisher(&self) -> String {
        for pref in ["AmericanEnglish", "BritishEnglish", "Japanese"] {
            if let Some(entry) = self.titles.get(pref)
                && !entry.publisher.is_empty() {
                    return entry.publisher.clone();
                }
        }
        for entry in self.titles.values() {
            if !entry.publisher.is_empty() {
                return entry.publisher.clone();
            }
        }
        "Unknown Publisher".to_string()
    }
}
