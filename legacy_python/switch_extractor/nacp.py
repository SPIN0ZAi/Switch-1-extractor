"""
Nintendo Switch NACP and Icon Metadata Parser.
Extracts localized game title, developer/publisher, display version, Title ID, and game icons.
"""

import struct
from typing import Dict, List, Optional, Tuple


LANGUAGE_NAMES = [
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
]


class NacpLanguageEntry:
    def __init__(self, language: str, title: str, publisher: str):
        self.language = language
        self.title = title
        self.publisher = publisher

    def __repr__(self):
        return f"<NacpTitle [{self.language}]: '{self.title}' by '{self.publisher}'>"


class Nacp:
    """Parser for Nintendo Application Control Property (control.nacp) data."""

    def __init__(self, data: bytes):
        self.titles: Dict[str, NacpLanguageEntry] = {}
        self.display_version = ""
        self.title_id = 0

        self._parse(data)

    def _parse(self, data: bytes):
        if len(data) < 0x3080:
            return

        # 16 language blocks, 0x300 bytes each
        for i in range(16):
            block = data[i * 0x300 : (i + 1) * 0x300]
            raw_title = block[:0x200].split(b"\x00", 1)[0]
            raw_pub = block[0x200:0x300].split(b"\x00", 1)[0]

            title = raw_title.decode("utf-8", errors="replace").strip()
            pub = raw_pub.decode("utf-8", errors="replace").strip()

            lang_name = LANGUAGE_NAMES[i]
            if title or pub:
                self.titles[lang_name] = NacpLanguageEntry(lang_name, title, pub)

        # Version string at 0x3060 (16 bytes)
        raw_ver = data[0x3060:0x3070].split(b"\x00", 1)[0]
        self.display_version = raw_ver.decode("ascii", errors="replace").strip()

        # Title ID at 0x3070 (8 bytes)
        title_id_val, = struct.unpack("<Q", data[0x3070:0x3078])
        self.title_id = title_id_val

    @property
    def title_id_hex(self) -> str:
        return f"{self.title_id:016X}"

    def get_preferred_title(self) -> str:
        """Returns English title if available, otherwise first non-empty title."""
        for pref in ("AmericanEnglish", "BritishEnglish", "Japanese"):
            if pref in self.titles and self.titles[pref].title:
                return self.titles[pref].title
        for entry in self.titles.values():
            if entry.title:
                return entry.title
        return "Unknown Title"

    def get_preferred_publisher(self) -> str:
        """Returns English publisher if available, otherwise first non-empty publisher."""
        for pref in ("AmericanEnglish", "BritishEnglish", "Japanese"):
            if pref in self.titles and self.titles[pref].publisher:
                return self.titles[pref].publisher
        for entry in self.titles.values():
            if entry.publisher:
                return entry.publisher
        return "Unknown Publisher"

