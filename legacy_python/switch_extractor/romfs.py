"""
Nintendo Switch RomFS (Read-Only Filesystem) Parser and Extractor.
Extracts directory hierarchies and game asset files (models, audio, textures, scripts).
"""

import os
import struct
from pathlib import Path
from typing import BinaryIO, Callable, Dict, List, Optional, Tuple, Union


class RomFsFileEntry:
    def __init__(self, name: str, parent_dir_offset: int, data_offset: int, size: int, full_path: str = ""):
        self.name = name
        self.parent_dir_offset = parent_dir_offset
        self.data_offset = data_offset
        self.size = size
        self.full_path = full_path

    def __repr__(self):
        return f"<RomFsFile '{self.full_path}' size={self.size:,} bytes>"


class RomFsDirEntry:
    def __init__(self, name: str, offset: int, parent_offset: int, full_path: str = ""):
        self.name = name
        self.offset = offset
        self.parent_offset = parent_offset
        self.full_path = full_path


class RomFs:
    """Parser and extractor for Nintendo Switch RomFS archives."""

    CHUNK_SIZE = 1024 * 1024  # 1 MB

    def __init__(self, file_or_stream: Union[str, Path, BinaryIO], base_offset: int = 0):
        self.base_offset = base_offset

        if isinstance(file_or_stream, (str, Path)):
            self.file_path = Path(file_or_stream)
            self._file = open(self.file_path, "rb")
            self._should_close = True
        else:
            self.file_path = None
            self._file = file_or_stream
            self._should_close = False

        self.directories: Dict[int, RomFsDirEntry] = {}
        self.files: List[RomFsFileEntry] = []
        self._files_by_path: Dict[str, RomFsFileEntry] = {}
        self.file_data_offset = 0

        self._parse()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._should_close and self._file:
            self._file.close()

    def _parse(self):
        self._file.seek(self.base_offset)
        magic_or_sz = self._file.read(8)

        # Check if wrapped in IVFC
        if magic_or_sz[:4] == b"IVFC":
            # Level 3 logical offset is at offset 0x3C
            self._file.seek(self.base_offset + 0x3C)
            l3_off, l3_sz = struct.unpack("<QQ", self._file.read(16))
            romfs_header_offset = self.base_offset + l3_off
            self._file.seek(romfs_header_offset)
            magic_or_sz = self._file.read(8)
        else:
            romfs_header_offset = self.base_offset

        # Check header size (should be 0x50 bytes)
        hdr_sz = struct.unpack("<Q", magic_or_sz)[0]
        if hdr_sz != 0x50:
            raise ValueError(f"Invalid RomFS header size: 0x{hdr_sz:X}. Expected 0x50.")

        # Read remaining 9 u64 fields (0x48 bytes)
        hdr_raw = self._file.read(0x48)
        (
            dir_hash_off,
            dir_hash_sz,
            dir_table_off,
            dir_table_sz,
            file_hash_off,
            file_hash_sz,
            file_table_off,
            file_table_sz,
            file_data_off,
        ) = struct.unpack("<QQQQQQQQQ", hdr_raw)

        self.file_data_offset = romfs_header_offset + file_data_off

        # 1. Read and parse Directory Table
        self._file.seek(romfs_header_offset + dir_table_off)
        dir_table_data = self._file.read(dir_table_sz)

        pos = 0
        while pos < dir_table_sz:
            if pos + 0x18 > dir_table_sz:
                break
            parent_off, sibling_off, child_dir_off, child_file_off, next_hash, name_len = struct.unpack(
                "<IIIIII", dir_table_data[pos : pos + 0x18]
            )
            name_bytes = dir_table_data[pos + 0x18 : pos + 0x18 + name_len]
            name = name_bytes.decode("utf-8", errors="replace")

            self.directories[pos] = RomFsDirEntry(name, pos, parent_off)

            # Advance pos aligned to 4 bytes
            entry_sz = (0x18 + name_len + 3) & ~3
            if entry_sz <= 0:
                break
            pos += entry_sz

        # Reconstruct directory full paths
        def resolve_dir_path(d_offset: int) -> str:
            d = self.directories.get(d_offset)
            if not d:
                return ""
            if d.full_path:
                return d.full_path
            if d.parent_offset == 0xFFFFFFFF or d.parent_offset == d_offset or d.parent_offset not in self.directories:
                d.full_path = d.name
            else:
                parent_p = resolve_dir_path(d.parent_offset)
                d.full_path = f"{parent_p}/{d.name}".strip("/")
            return d.full_path

        for d_off in self.directories:
            resolve_dir_path(d_off)

        # 2. Read and parse File Table
        self._file.seek(romfs_header_offset + file_table_off)
        file_table_data = self._file.read(file_table_sz)

        pos = 0
        while pos < file_table_sz:
            if pos + 0x20 > file_table_sz:
                break
            parent_off, sibling_off, data_off, data_sz, next_hash, name_len = struct.unpack(
                "<IIQQII", file_table_data[pos : pos + 0x20]
            )
            name_bytes = file_table_data[pos + 0x20 : pos + 0x20 + name_len]
            name = name_bytes.decode("utf-8", errors="replace")

            parent_dir = self.directories.get(parent_off)
            dir_path = parent_dir.full_path if parent_dir else ""
            full_path = f"{dir_path}/{name}".strip("/")

            entry = RomFsFileEntry(name, parent_off, data_off, data_sz, full_path)
            self.files.append(entry)
            self._files_by_path[full_path.lower()] = entry

            entry_sz = (0x20 + name_len + 3) & ~3
            if entry_sz <= 0:
                break
            pos += entry_sz

    def get_file(self, path: str) -> Optional[RomFsFileEntry]:
        clean = path.strip("/").replace("\\", "/").lower()
        return self._files_by_path.get(clean)

    def read_file(self, entry_or_path: Union[str, RomFsFileEntry]) -> bytes:
        """Reads file data into memory."""
        if isinstance(entry_or_path, str):
            target = self.get_file(entry_or_path)
            if not target:
                raise KeyError(f"File '{entry_or_path}' not found in RomFS.")
        else:
            target = entry_or_path

        self._file.seek(self.file_data_offset + target.data_offset)
        return self._file.read(target.size)

    def extract_entry(
        self,
        entry: RomFsFileEntry,
        out_path: Union[str, Path],
        progress_callback: Optional[Callable[[int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> int:
        """Streams a single file from RomFS to disk."""
        out_path = Path(out_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        self._file.seek(self.file_data_offset + entry.data_offset)
        remaining = entry.size
        written = 0

        with open(out_path, "wb") as out_f:
            while remaining > 0:
                if cancel_check and cancel_check():
                    out_f.close()
                    if out_path.exists():
                        out_path.unlink()
                    raise InterruptedError("Extraction cancelled by user.")

                to_read = min(self.CHUNK_SIZE, remaining)
                chunk = self._file.read(to_read)
                if not chunk:
                    break
                out_f.write(chunk)
                remaining -= len(chunk)
                written += len(chunk)
                if progress_callback:
                    progress_callback(written, entry.size)

        return written

    def extract_all(
        self,
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> List[Path]:
        """
        Extracts all files in the RomFS preserving the directory structure.
        progress_callback: (file_index, total_files, file_path, bytes_done, bytes_total)
        """
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        extracted = []
        total_files = len(self.files)

        for idx, entry in enumerate(self.files):
            # Target path relative to out_dir
            dest_file = out_dir / entry.full_path.replace("/", os.sep)

            def f_cb(done, tot):
                if progress_callback:
                    progress_callback(idx + 1, total_files, entry.full_path, done, tot)

            self.extract_entry(entry, dest_file, progress_callback=f_cb, cancel_check=cancel_check)
            extracted.append(dest_file)

        return extracted

