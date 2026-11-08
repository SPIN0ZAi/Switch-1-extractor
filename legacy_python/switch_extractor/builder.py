"""
Nintendo Switch Package Builder & Converter.
Provides:
- PFS0 / NSP Repacker (packs loose NCAs/tickets/certs into an installable .nsp)
- Direct XCI to NSP Converter (converts GameCard dumps to eShop installable packages)
"""

import os
import struct
from pathlib import Path
from typing import Callable, Dict, List, Optional, Union

from switch_extractor.pfs0 import Pfs0
from switch_extractor.xci import Xci


def build_pfs0(
    input_files: Union[str, Path, List[Path], Dict[str, Path]],
    output_path: Union[str, Path],
    progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
    cancel_check: Optional[Callable[[], bool]] = None,
) -> Path:
    """
    Packs a directory or list of files into a standard Nintendo Switch PFS0 (.nsp) package.
    """
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    files_to_pack: List[tuple[str, Path]] = []

    if isinstance(input_files, (str, Path)):
        in_dir = Path(input_files)
        if in_dir.is_dir():
            for p in sorted(in_dir.iterdir()):
                if p.is_file():
                    files_to_pack.append((p.name, p))
        elif in_dir.is_file():
            files_to_pack.append((in_dir.name, in_dir))
    elif isinstance(input_files, dict):
        for name, p in input_files.items():
            files_to_pack.append((name, Path(p)))
    elif isinstance(input_files, list):
        for p in input_files:
            path_obj = Path(p)
            files_to_pack.append((path_obj.name, path_obj))

    if not files_to_pack:
        raise ValueError("No files provided to pack into PFS0/NSP.")

    # 1. Build string table & file entries
    file_count = len(files_to_pack)
    entry_size = 0x18  # 24 bytes per PFS0 entry
    string_table = b""
    entries_meta = []
    current_data_offset = 0

    for name, path in files_to_pack:
        name_bytes = name.encode("utf-8") + b"\x00"
        name_offset = len(string_table)
        string_table += name_bytes

        file_sz = path.stat().st_size
        entries_meta.append((current_data_offset, file_sz, name_offset))
        current_data_offset += file_sz

    header_size = 0x10 + (file_count * entry_size) + len(string_table)

    # 2. Write PFS0 Header
    with open(output_path, "wb") as out_f:
        out_f.write(struct.pack("<4sIII", b"PFS0", file_count, len(string_table), 0))

        for data_off, file_sz, name_off in entries_meta:
            out_f.write(struct.pack("<QQII", data_off, file_sz, name_off, 0))

        out_f.write(string_table)

        # 3. Stream file contents
        chunk_size = 1024 * 1024  # 1 MB
        total_files = len(files_to_pack)

        for idx, (name, path) in enumerate(files_to_pack, 1):
            file_sz = path.stat().st_size
            written = 0

            with open(path, "rb") as in_f:
                while True:
                    if cancel_check and cancel_check():
                        out_f.close()
                        if output_path.exists():
                            output_path.unlink()
                        raise InterruptedError("Packing cancelled by user.")

                    chunk = in_f.read(chunk_size)
                    if not chunk:
                        break
                    out_f.write(chunk)
                    written += len(chunk)
                    if progress_callback:
                        progress_callback(idx, total_files, name, written, file_sz)

    return output_path


def convert_xci_to_nsp(
    xci_path: Union[str, Path],
    output_nsp_path: Union[str, Path],
    progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
    cancel_check: Optional[Callable[[], bool]] = None,
) -> Path:
    """
    Directly converts a Nintendo Switch GameCard (.xci) dump into an installable .nsp package
    by extracting the secure partition on-the-fly and wrapping it into a standard PFS0 container.
    Does not require saving intermediate files to disk!
    """
    xci_path = Path(xci_path)
    output_nsp_path = Path(output_nsp_path)
    output_nsp_path.parent.mkdir(parents=True, exist_ok=True)

    with Xci(xci_path) as xci:
        sec = xci.secure_partition
        if not sec:
            raise ValueError("XCI does not contain a secure partition.")

        entries = sec.hfs0.entries
        file_count = len(entries)
        entry_size = 0x18  # PFS0 entry size

        # Build string table
        string_table = b""
        pfs0_entries = []
        current_data_offset = 0

        for entry in entries:
            name_bytes = entry.name.encode("utf-8") + b"\x00"
            name_offset = len(string_table)
            string_table += name_bytes

            pfs0_entries.append((current_data_offset, entry.size, name_offset))
            current_data_offset += entry.size

        header_size = 0x10 + (file_count * entry_size) + len(string_table)

        with open(output_nsp_path, "wb") as out_f:
            # Write PFS0 Header
            out_f.write(struct.pack("<4sIII", b"PFS0", file_count, len(string_table), 0))

            for data_off, file_sz, name_off in pfs0_entries:
                out_f.write(struct.pack("<QQII", data_off, file_sz, name_off, 0))

            out_f.write(string_table)

            # Stream NCAs directly from XCI secure partition to the new NSP
            chunk_size = 1024 * 1024  # 1 MB
            for idx, entry in enumerate(entries, 1):
                sec.hfs0._file.seek(entry.abs_offset)
                remaining = entry.size
                written = 0

                while remaining > 0:
                    if cancel_check and cancel_check():
                        out_f.close()
                        if output_nsp_path.exists():
                            output_nsp_path.unlink()
                        raise InterruptedError("XCI to NSP conversion cancelled.")

                    to_read = min(chunk_size, remaining)
                    chunk = sec.hfs0._file.read(to_read)
                    if not chunk:
                        break
                    out_f.write(chunk)
                    remaining -= len(chunk)
                    written += len(chunk)
                    if progress_callback:
                        progress_callback(idx, file_count, entry.name, written, entry.size)

    return output_nsp_path

