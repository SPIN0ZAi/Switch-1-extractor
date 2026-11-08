"""
Nintendo Switch XCI and XCZ GameCard Image Parser and Extractor.
Parses Gamecard header, root HFS0, and partitions (secure, update, normal, logo).
"""

import os
import struct
from pathlib import Path
from typing import BinaryIO, Callable, Dict, List, Optional, Union
from switch_extractor.pfs0 import Pfs0, Pfs0Entry


class XciPartition:
    def __init__(self, name: str, abs_offset: int, size: int, hfs0: Pfs0):
        self.name = name
        self.abs_offset = abs_offset
        self.size = size
        self.hfs0 = hfs0

    def __repr__(self):
        return f"<XciPartition name='{self.name}' files={len(self.hfs0.entries)} offset=0x{self.abs_offset:X}>"


class Xci:
    """Parser for Nintendo Switch GameCard (.xci, .xcz) images."""

    CHUNK_SIZE = 1024 * 1024  # 1 MB chunk

    def __init__(self, file_or_path: Union[str, Path, BinaryIO]):
        if isinstance(file_or_path, (str, Path)):
            self.file_path = Path(file_or_path)
            self._file = open(self.file_path, "rb")
            self._should_close = True
        else:
            self.file_path = None
            self._file = file_or_path
            self._should_close = False

        self.header_offset = 0
        self.root_hfs0_offset = 0
        self.partitions: Dict[str, XciPartition] = {}
        self.package_id = 0
        self.rom_size_id = 0
        self.valid_data_end = 0

        self._parse()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        for part in self.partitions.values():
            part.hfs0.close()
        if self._should_close and self._file:
            self._file.close()

    def _find_header(self):
        """Locates the HEAD magic in the XCI file."""
        # Standard untrimmed XCI has CardHeader starting at 0x1000, HEAD magic at 0x1100
        self._file.seek(0x1100)
        if self._file.read(4) == b"HEAD":
            self.header_offset = 0x1000
            return

        # Trimmed XCI might start directly at CardHeader, HEAD magic at 0x100
        self._file.seek(0x100)
        if self._file.read(4) == b"HEAD":
            self.header_offset = 0
            return

        # Direct start (offset 0)
        self._file.seek(0)
        if self._file.read(4) == b"HEAD":
            self.header_offset = -0x100
            return

        raise ValueError("Could not find Gamecard 'HEAD' magic in XCI file. Not a valid Nintendo Switch XCI.")

    def _parse(self):
        self._find_header()

        # Read CardHeader fields (starting right after HEAD magic, at header_offset + 0x104)
        self._file.seek(self.header_offset + 0x104)
        rom_area_start_page, backup_area = struct.unpack("<II", self._file.read(8))
        title_key_dec, rom_size, card_ver, flags = struct.unpack("<BBBB", self._file.read(4))
        self.rom_size_id = rom_size

        package_id, valid_data_end = struct.unpack("<QI", self._file.read(12))
        self.package_id = package_id
        self.valid_data_end = valid_data_end * 0x200

        # PartitionFsHeaderAddress is at header_offset + 0x130
        self._file.seek(self.header_offset + 0x130)
        pfs_header_addr, pfs_header_size = struct.unpack("<QQ", self._file.read(16))

        # Determine Root HFS0 offset
        candidate_offsets = []
        if pfs_header_addr > 0:
            candidate_offsets.append(pfs_header_addr)
            if self.header_offset > 0:
                candidate_offsets.append(self.header_offset + pfs_header_addr)

        # Standard well-known root HFS0 offsets
        candidate_offsets.extend([0xF000, 0x10000, 0x14000, 0xE000, 0x200])

        root_offset = None
        for cand in candidate_offsets:
            self._file.seek(cand)
            if self._file.read(4) == b"HFS0":
                root_offset = cand
                break

        if root_offset is None:
            raise ValueError("Found XCI HEAD magic, but could not locate Root HFS0 partition table.")

        self.root_hfs0_offset = root_offset

        # Parse Root HFS0
        root_hfs0 = Pfs0(self._file, base_offset=self.root_hfs0_offset)

        # Each entry in root HFS0 is a partition (e.g. 'update', 'normal', 'secure', 'logo')
        for entry in root_hfs0.entries:
            part_name = entry.name.lower()
            part_offset = entry.abs_offset
            part_size = entry.size

            # Verify partition has HFS0 magic
            self._file.seek(part_offset)
            if self._file.read(4) == b"HFS0":
                part_hfs0 = Pfs0(self._file, base_offset=part_offset)
                self.partitions[part_name] = XciPartition(part_name, part_offset, part_size, part_hfs0)

    @property
    def secure_partition(self) -> Optional[XciPartition]:
        """Returns the 'secure' partition containing the main game NCAs."""
        return self.partitions.get("secure")

    def get_game_nca_entries(self) -> List[Pfs0Entry]:
        """Retrieves list of all NCA entries inside the secure partition."""
        sec = self.secure_partition
        if sec:
            return sec.hfs0.entries
        return []

    def extract_secure_partition(
        self,
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None
    ) -> List[Path]:
        """
        Extracts all files from the 'secure' partition (the game's NCAs).
        This is the most common action when unpacking an XCI.
        """
        sec = self.secure_partition
        if not sec:
            raise ValueError("XCI does not contain a 'secure' partition.")
        return sec.hfs0.extract_all(out_dir, progress_callback=progress_callback, cancel_check=cancel_check)

    def extract_all_partitions(
        self,
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[str, int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None
    ) -> Dict[str, List[Path]]:
        """
        Extracts all partitions (update, normal, secure, logo) into separate subdirectories.
        progress_callback args: (partition_name, file_idx, total_files, file_name, bytes_done, bytes_total)
        """
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        results = {}

        for part_name, part in self.partitions.items():
            part_dir = out_dir / part_name
            part_dir.mkdir(parents=True, exist_ok=True)

            def part_cb(idx, total, fname, done, ftotal):
                if progress_callback:
                    progress_callback(part_name, idx, total, fname, done, ftotal)

            extracted = part.hfs0.extract_all(part_dir, progress_callback=part_cb, cancel_check=cancel_check)
            results[part_name] = extracted

        return results

