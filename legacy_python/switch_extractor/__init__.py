"""
Nintendo Switch 1 Game Extractor & Decryptor Package.
Supports .XCI, .XCZ, .NSP, .NSZ, .NCA, .NCZ, .PFS0, .HFS0, and .ROMFS.
"""

from switch_extractor.keys import KeySet
from switch_extractor.pfs0 import Pfs0, Pfs0Entry
from switch_extractor.xci import Xci, XciPartition
from switch_extractor.nca import Nca, NcaSection
from switch_extractor.ncz import NczDecompressor
from switch_extractor.romfs import RomFs
from switch_extractor.nacp import Nacp
from switch_extractor.core import SwitchExtractor, SwitchGameInfo

__version__ = "1.0.0"
__all__ = [
    "KeySet",
    "Pfs0",
    "Pfs0Entry",
    "Xci",
    "XciPartition",
    "Nca",
    "NcaSection",
    "NczDecompressor",
    "RomFs",
    "Nacp",
    "SwitchExtractor",
    "SwitchGameInfo",
]

