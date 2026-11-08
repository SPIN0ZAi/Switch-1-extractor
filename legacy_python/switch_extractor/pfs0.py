"""
Nintendo Switch PFS0 and HFS0 Partition File System Parser and Extractor.
Used for NSP, ExeFS, XCI partitions (root, secure, update, normal, logo), etc.
"""

import os
import struct
from typing import BinaryIO, Callable, Dict, List, Optional, Union
from pathlib import Path


class Pfs0Entry:
    def __init__(self, name: str, data_offset: int, size: int, abs_offset: int, sha256: Optional[bytes] = None):
        self.name = name
        self.data_offset = data_offset  # Offset relative to data start
        self.size = size
        self.abs_offset = abs_offset    # Absolute offset in container
        self.sha256 = sha256

    def __repr__(self):
        return f"<Pfs0Entry name='{self.name}' size={self.size:,} bytes offset=0x{self.abs_offset:X}>"


class Pfs0:
    """Parser for PFS0 (NSP, ExeFS) and HFS0 (XCI partitions) containers."""

    CHUNK_SIZE = 1024 * 1024  # 1 MB chunk for streaming

    def __init__(self, file_or_path: Union[str, Path, BinaryIO], base_offset: int = 0):
        self.base_offset = base_offset
        self.entries: List[Pfs0Entry] = []
        self._entries_by_name: Dict[str, Pfs0Entry] = {}
        self.is_hfs0 = False
        self.header_size = 0
        self.data_start = 0

        if isinstance(file_or_path, (str, Path)):
            self.file_path = Path(file_or_path)
            self._file = open(self.file_path, "rb")
            self._should_close = True
        else:
            self.file_path = None
            self._file = file_or_path
            self._should_close = False

        self._parse_header()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._should_close and self._file:
            self._file.close()

    def _parse_header(self):
        self._file.seek(self.base_offset)
        magic = self._file.read(4)

        if magic == b"PFS0":
            self.is_hfs0 = False
            entry_size = 0x18
        elif magic == b"HFS0":
            self.is_hfs0 = True
            entry_size = 0x40
        else:
            raise ValueError(f"Invalid magic: {magic!r}. Expected b'PFS0' or b'HFS0' at 0x{self.base_offset:X}")

        file_count, string_table_size, _ = struct.unpack("<III", self._file.read(12))

        # Read raw entry table
        entry_table_bytes = self._file.read(file_count * entry_size)
        string_table_bytes = self._file.read(string_table_size)

        self.header_size = 0x10 + (file_count * entry_size) + string_table_size
        self.data_start = self.base_offset + self.header_size

        # Parse string table helper
        def get_string(offset: int) -> str:
            end = string_table_bytes.find(b"\x00", offset)
            if end == -1:
                end = len(string_table_bytes)
            return string_table_bytes[offset:end].decode("utf-8", errors="replace")

        # Parse entries
        for i in range(file_count):
            entry_raw = entry_table_bytes[i * entry_size : (i + 1) * entry_size]
            if not self.is_hfs0:
                data_off, file_sz, name_off, _ = struct.unpack("<QQII", entry_raw[:24])
                sha256 = None
            else:
                data_off, file_sz, name_off, hash_sz, _, sha256 = struct.unpack("<QQIIQ32s", entry_raw[:64])

            name = get_string(name_off)
            abs_off = self.data_start + data_off
            entry = Pfs0Entry(name, data_off, file_sz, abs_off, sha256)
            self.entries.append(entry)
            self._entries_by_name[name] = entry

    def get_entry(self, name: str) -> Optional[Pfs0Entry]:
        return self._entries_by_name.get(name)

    def read_file(self, entry: Union[str, Pfs0Entry]) -> bytes:
        """Reads the full binary contents of a specific entry into memory."""
        if isinstance(entry, str):
            target = self.get_entry(entry)
            if not target:
                raise KeyError(f"Entry '{entry}' not found in partition.")
        else:
            target = entry

        self._file.seek(target.abs_offset)
        return self._file.read(target.size)

    def extract_entry(
        self,
        entry: Union[str, Pfs0Entry],
        out_path: Union[str, Path],
        progress_callback: Optional[Callable[[int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None
    ) -> int:
        """
        Extracts a single entry to disk using streaming chunks to conserve RAM.
        Returns total bytes written.
        """
        if isinstance(entry, str):
            target = self.get_entry(entry)
            if not target:
                raise KeyError(f"Entry '{entry}' not found in partition.")
        else:
            target = entry

        out_path = Path(out_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        self._file.seek(target.abs_offset)
        remaining = target.size
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
                    progress_callback(written, target.size)

        return written

    def extract_all(
        self,
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None
    ) -> List[Path]:
        """
        Extracts all entries to the destination directory.
        progress_callback args: (file_index, total_files, file_name, file_bytes_done, file_total_bytes)
        """
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        extracted_paths = []

        total_files = len(self.entries)
        for idx, entry in enumerate(self.entries):
            out_file = out_dir / entry.name

            def entry_cb(done, total):
                if progress_callback:
                    progress_callback(idx + 1, total_files, entry.name, done, total)

            self.extract_entry(entry, out_file, progress_callback=entry_cb, cancel_check=cancel_check)
            extracted_paths.append(out_file)

        return extracted_paths

