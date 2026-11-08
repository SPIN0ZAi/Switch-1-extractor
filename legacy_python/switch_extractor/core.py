"""
Nintendo Switch Extractor Engine (Core).
Coordinates inspection, extraction, RomFS/ExeFS unpacking, and decompression
across all Nintendo Switch container and archive formats.
"""

import json
import os
import shutil
import struct
import tempfile
from pathlib import Path
from typing import BinaryIO, Callable, Dict, List, Optional, Tuple, Union

from switch_extractor.keys import KeySet
from switch_extractor.pfs0 import Pfs0, Pfs0Entry
from switch_extractor.xci import Xci
from switch_extractor.nca import Nca, NcaSection
from switch_extractor.ncz import NczDecompressor
from switch_extractor.romfs import RomFs
from switch_extractor.nacp import Nacp, LANGUAGE_NAMES


class SwitchGameInfo:
    def __init__(self):
        self.file_path: Optional[Path] = None
        self.format: str = "Unknown"
        self.file_size: int = 0
        self.title_name: str = "Unknown Game"
        self.publisher: str = "Unknown Publisher"
        self.display_version: str = "1.0.0"
        self.title_id: str = "Unknown"
        self.icon_bytes: Optional[bytes] = None
        self.files_list: List[str] = []
        self.has_keys: bool = False
        self.is_compressed: bool = False
        self.content_types: Dict[str, str] = {}
        self.has_ticket: bool = False
        self.ticket_decrypted: bool = False

    def to_dict(self) -> dict:
        return {
            "title_name": self.title_name,
            "publisher": self.publisher,
            "version": self.display_version,
            "title_id": self.title_id,
            "format": self.format,
            "file_size": self.file_size,
            "file_size_formatted": f"{self.file_size / (1024 * 1024 * 1024):.2f} GB"
            if self.file_size >= 1024**3
            else f"{self.file_size / (1024 * 1024):.2f} MB",
            "is_compressed": self.is_compressed,
            "has_ticket": self.has_ticket,
            "ticket_decrypted": self.ticket_decrypted,
            "files_count": len(self.files_list),
            "files": self.files_list,
        }


class SwitchExtractor:
    """Unified engine for extracting and inspecting Nintendo Switch game files."""

    def __init__(self, keyset: Optional[KeySet] = None):
        self.keyset = keyset or KeySet()

    def detect_format(self, file_path: Union[str, Path]) -> str:
        """Identifies format based on extension and internal magic bytes."""
        path = Path(file_path)
        ext = path.suffix.lower()

        if not path.is_file():
            return "Unknown"

        with open(path, "rb") as f:
            magic_16 = f.read(16)

        # Check for direct magic
        if magic_16[:4] == b"PFS0":
            if ext == ".nsz":
                return "NSZ"
            try:
                with Pfs0(path) as p:
                    if any(e.name.lower().endswith(".ncz") for e in p.entries):
                        return "NSZ"
            except Exception:
                pass
            return "NSP" if ext in (".nsp", "") else "PFS0"

        if magic_16[:4] == b"HFS0":
            return "HFS0"

        if magic_16[:4] == b"IVFC" or (len(magic_16) >= 8 and struct.unpack("<Q", magic_16[:8])[0] == 0x50):
            return "ROMFS"

        # Check XCI HEAD magic (at 0x100 or 0x1100)
        try:
            with open(path, "rb") as f:
                f.seek(0x100)
                m1 = f.read(4)
                f.seek(0x1100)
                m2 = f.read(4)
            if m1 == b"HEAD" or m2 == b"HEAD":
                if ext == ".xcz":
                    return "XCZ"
                try:
                    with Xci(path) as x:
                        sec = x.secure_partition
                        if sec and any(e.name.lower().endswith(".ncz") for e in sec.hfs0.entries):
                            return "XCZ"
                except Exception:
                    pass
                return "XCI"
        except Exception:
            pass

        # Check NCZ (magic at 0x4000)
        if ext == ".ncz":
            return "NCZ"
        try:
            with open(path, "rb") as f:
                f.seek(0x4000)
                raw_cnt = f.read(8)
                f.seek(0x4008)
                m_sec = f.read(8)
            if m_sec == b"NCZSECTN":
                return "NCZ"
        except Exception:
            pass

        # Check NCA
        if ext == ".nca":
            return "NCA"
        try:
            with Nca(path, keyset=self.keyset) as _:
                return "NCA"
        except Exception:
            pass

        return ext.upper().lstrip(".") or "Unknown"

    def _load_container_tickets(self, path: Path, fmt: str) -> int:
        """Parses and caches all tickets (.tik) found in the container."""
        loaded = 0
        if fmt in ("NSP", "NSZ", "PFS0"):
            try:
                with Pfs0(path) as p:
                    for e in p.entries:
                        if e.name.lower().endswith(".tik"):
                            tik_data = p.read_file(e)
                            res = self.keyset.decrypt_ticket_title_key(tik_data)
                            if res:
                                loaded += 1
            except Exception:
                pass
        elif fmt in ("XCI", "XCZ"):
            try:
                with Xci(path) as x:
                    sec = x.secure_partition
                    if sec:
                        for e in sec.hfs0.entries:
                            if e.name.lower().endswith(".tik"):
                                tik_data = sec.hfs0.read_file(e)
                                res = self.keyset.decrypt_ticket_title_key(tik_data)
                                if res:
                                    loaded += 1
            except Exception:
                pass
        return loaded

    def inspect(self, file_path: Union[str, Path]) -> SwitchGameInfo:
        """Inspects game file, loads tickets/keys, and extracts metadata/icon."""
        path = Path(file_path)
        info = SwitchGameInfo()
        info.file_path = path
        info.file_size = path.stat().st_size
        info.has_keys = self.keyset.has_header_key
        fmt = self.detect_format(path)
        info.format = fmt
        info.is_compressed = fmt in ("NSZ", "XCZ", "NCZ")

        # Load any tickets before proceeding
        tik_count = self._load_container_tickets(path, fmt)
        if tik_count > 0:
            info.has_ticket = True
            info.ticket_decrypted = True

        if fmt in ("NSP", "NSZ"):
            self._inspect_nsp(path, info)
        elif fmt in ("XCI", "XCZ"):
            self._inspect_xci(path, info)
        elif fmt == "NCA":
            self._inspect_nca(path, info)
        elif fmt in ("PFS0", "HFS0"):
            with Pfs0(path) as p:
                info.files_list = [f"{e.name} ({e.size:,} bytes)" for e in p.entries]
        elif fmt == "ROMFS":
            with RomFs(path) as r:
                info.files_list = [f"{f.full_path} ({f.size:,} bytes)" for f in r.files[:100]]

        return info

    def _inspect_nsp(self, path: Path, info: SwitchGameInfo):
        with Pfs0(path) as p:
            info.files_list = [f"{e.name} ({e.size:,} bytes)" for e in p.entries]

            # Find Control NCA
            control_entry = None
            for e in p.entries:
                ename = e.name.lower()
                if ename.endswith(".control.nca") or (ename.endswith(".nca") and "control" in ename):
                    control_entry = e
                    break

            if not control_entry and self.keyset.has_header_key:
                for e in p.entries:
                    if e.name.lower().endswith(".nca") and e.size < 60 * 1024 * 1024:
                        try:
                            with Nca(p._file, keyset=self.keyset, base_offset=e.abs_offset, size=e.size) as nca:
                                if nca.content_type == 2:  # Control
                                    control_entry = e
                                    break
                        except Exception:
                            pass

            if control_entry:
                self._extract_control_metadata(p._file, control_entry.abs_offset, control_entry.size, info)

    def _inspect_xci(self, path: Path, info: SwitchGameInfo):
        with Xci(path) as x:
            sec = x.secure_partition
            if sec:
                info.files_list = [f"secure/{e.name} ({e.size:,} bytes)" for e in sec.hfs0.entries]
            for part_name, part in x.partitions.items():
                if part_name != "secure":
                    info.files_list.extend([f"{part_name}/{e.name} ({e.size:,} bytes)" for e in part.hfs0.entries])

            if sec:
                control_entry = None
                for e in sec.hfs0.entries:
                    ename = e.name.lower()
                    if ename.endswith(".control.nca") or (ename.endswith(".nca") and "control" in ename):
                        control_entry = e
                        break

                if not control_entry and self.keyset.has_header_key:
                    for e in sec.hfs0.entries:
                        if e.name.lower().endswith(".nca") and e.size < 60 * 1024 * 1024:
                            try:
                                with Nca(sec.hfs0._file, keyset=self.keyset, base_offset=e.abs_offset, size=e.size) as nca:
                                    if nca.content_type == 2:
                                        control_entry = e
                                        break
                            except Exception:
                                pass

                if control_entry:
                    self._extract_control_metadata(sec.hfs0._file, control_entry.abs_offset, control_entry.size, info)

    def _inspect_nca(self, path: Path, info: SwitchGameInfo):
        try:
            with Nca(path, keyset=self.keyset) as nca:
                info.title_id = nca.title_id_hex
                info.title_name = f"{nca.content_type_name} Archive ({nca.title_id_hex})"
                info.files_list = [repr(s) for s in nca.sections]
                if nca.content_type == 2:  # Control
                    self._extract_control_metadata(nca._file, nca.base_offset, nca.total_size, info)
        except Exception as e:
            info.files_list = [f"Encrypted NCA (Header key required to inspect: {e})"]

    def _extract_control_metadata(self, file_obj: BinaryIO, nca_offset: int, nca_size: int, info: SwitchGameInfo):
        """Extracts NACP (title, version) and Icon from a Control NCA."""
        if not self.keyset.has_header_key:
            return

        try:
            with Nca(file_obj, keyset=self.keyset, base_offset=nca_offset, size=nca_size) as nca:
                info.title_id = nca.title_id_hex
                romfs_sec = next((s for s in nca.sections if s.fs_type == 0), None)
                if not romfs_sec:
                    return

                stream = nca.get_section_reader(romfs_sec)
                with RomFs(stream) as romfs:
                    nacp_file = romfs.get_file("control.nacp")
                    if nacp_file:
                        nacp_bytes = romfs.read_file(nacp_file)
                        nacp = Nacp(nacp_bytes)
                        info.title_name = nacp.get_preferred_title()
                        info.publisher = nacp.get_preferred_publisher()
                        info.display_version = nacp.display_version or "1.0.0"
                        if nacp.title_id != 0:
                            info.title_id = nacp.title_id_hex

                    for lang in ("AmericanEnglish", "BritishEnglish", "Japanese"):
                        icon_file = romfs.get_file(f"icon_{lang}.dat")
                        if icon_file:
                            info.icon_bytes = romfs.read_file(icon_file)
                            break
                    if not info.icon_bytes:
                        for f in romfs.files:
                            if f.name.startswith("icon_") and f.name.endswith(".dat"):
                                info.icon_bytes = romfs.read_file(f)
                                break
        except Exception:
            pass

    # =========================================================================
    # Extraction Methods
    # =========================================================================

    def extract_container(
        self,
        file_path: Union[str, Path],
        out_dir: Union[str, Path],
        decompress_ncz: bool = True,
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> List[Path]:
        """
        Unpacks all files from the container (NSP, NSZ, XCI, XCZ, PFS0, HFS0).
        If decompress_ncz is True, decompresses .ncz files into standard .nca files.
        Works without keys for raw container extraction!
        """
        path = Path(file_path)
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        fmt = self.detect_format(path)
        extracted = []

        if fmt in ("NSP", "NSZ", "PFS0"):
            with Pfs0(path) as p:
                total_files = len(p.entries)
                for idx, entry in enumerate(p.entries):
                    is_ncz = entry.name.lower().endswith(".ncz")
                    target_name = (
                        entry.name[:-4] + ".nca" if (is_ncz and decompress_ncz) else entry.name
                    )
                    out_file = out_dir / target_name

                    def file_cb(done, tot):
                        if progress_callback:
                            progress_callback(idx + 1, total_files, target_name, done, tot)

                    if is_ncz and decompress_ncz:
                        with NczDecompressor(p._file, base_offset=entry.abs_offset, size=entry.size) as decomp:
                            decomp.decompress(out_file, reencrypt=True, progress_callback=file_cb, cancel_check=cancel_check)
                    else:
                        p.extract_entry(entry, out_file, progress_callback=file_cb, cancel_check=cancel_check)

                    extracted.append(out_file)

        elif fmt in ("XCI", "XCZ"):
            with Xci(path) as x:
                sec = x.secure_partition
                if not sec:
                    raise ValueError("XCI does not contain a secure partition.")

                total_files = len(sec.hfs0.entries)
                for idx, entry in enumerate(sec.hfs0.entries):
                    is_ncz = entry.name.lower().endswith(".ncz")
                    target_name = (
                        entry.name[:-4] + ".nca" if (is_ncz and decompress_ncz) else entry.name
                    )
                    out_file = out_dir / target_name

                    def file_cb(done, tot):
                        if progress_callback:
                            progress_callback(idx + 1, total_files, target_name, done, tot)

                    if is_ncz and decompress_ncz:
                        with NczDecompressor(sec.hfs0._file, base_offset=entry.abs_offset, size=entry.size) as decomp:
                            decomp.decompress(out_file, reencrypt=True, progress_callback=file_cb, cancel_check=cancel_check)
                    else:
                        sec.hfs0.extract_entry(entry, out_file, progress_callback=file_cb, cancel_check=cancel_check)

                    extracted.append(out_file)

        elif fmt == "NCZ":
            out_file = out_dir / (path.stem + ".nca")
            with NczDecompressor(path) as decomp:
                decomp.decompress(out_file, reencrypt=True, progress_callback=lambda d, t: progress_callback(1, 1, out_file.name, d, t) if progress_callback else None, cancel_check=cancel_check)
            extracted.append(out_file)

        elif fmt == "NCA":
            with Nca(path, keyset=self.keyset) as nca:
                for sec in nca.sections:
                    sec_file = out_dir / f"section_{sec.index}_{sec.fs_type_name.lower()}.bin"
                    nca.extract_section_raw(sec, sec_file, decrypt=True, cancel_check=cancel_check)
                    extracted.append(sec_file)

        return extracted

    def extract_romfs(
        self,
        file_path: Union[str, Path],
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> List[Path]:
        """
        Decrypts Program NCA and extracts all RomFS game assets (models, textures, audio, scripts)
        into out_dir/romfs. Requires prod.keys (and title key if Rights ID based).
        """
        if not self.keyset.has_header_key:
            raise PermissionError("prod.keys with header_key is required to decrypt and extract RomFS assets.")

        path = Path(file_path)
        out_dir = Path(out_dir)
        romfs_dir = out_dir / "romfs"
        romfs_dir.mkdir(parents=True, exist_ok=True)
        fmt = self.detect_format(path)

        # Preload any tickets inside container
        self._load_container_tickets(path, fmt)

        if fmt == "ROMFS":
            with RomFs(path) as r:
                return r.extract_all(romfs_dir, progress_callback=progress_callback, cancel_check=cancel_check)

        program_nca_tuple = self._find_program_nca(path, fmt)
        if not program_nca_tuple:
            raise ValueError("Could not find or decrypt the Program NCA in this game package.")

        file_obj, offset, size = program_nca_tuple

        with Nca(file_obj, keyset=self.keyset, base_offset=offset, size=size) as nca:
            romfs_sec = next((s for s in nca.sections if s.fs_type == 0), None)
            if not romfs_sec:
                raise ValueError("Program NCA does not contain a RomFS section.")

            if romfs_sec.crypt_type == 4:
                raise ValueError("This package is a Game Update (BKTR delta). RomFS extraction requires merging with the Base Game.")

            stream = nca.get_section_reader(romfs_sec)
            with RomFs(stream) as romfs:
                return romfs.extract_all(romfs_dir, progress_callback=progress_callback, cancel_check=cancel_check)

    def extract_exefs(
        self,
        file_path: Union[str, Path],
        out_dir: Union[str, Path],
        progress_callback: Optional[Callable[[int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> List[Path]:
        """
        Decrypts Program NCA and extracts ExeFS binaries (main, main.npdm, rtld, subsdk*)
        into out_dir/exefs. Requires prod.keys.
        """
        if not self.keyset.has_header_key:
            raise PermissionError("prod.keys with header_key is required to decrypt and extract ExeFS binaries.")

        path = Path(file_path)
        out_dir = Path(out_dir)
        exefs_dir = out_dir / "exefs"
        exefs_dir.mkdir(parents=True, exist_ok=True)
        fmt = self.detect_format(path)

        # Preload tickets
        self._load_container_tickets(path, fmt)

        program_nca_tuple = self._find_program_nca(path, fmt)
        if not program_nca_tuple:
            raise ValueError("Could not find or decrypt the Program NCA in this game package.")

        file_obj, offset, size = program_nca_tuple

        with Nca(file_obj, keyset=self.keyset, base_offset=offset, size=size) as nca:
            exefs_sec = next((s for s in nca.sections if s.fs_type == 1), None)
            if not exefs_sec:
                raise ValueError("Program NCA does not contain an ExeFS (PartitionFS) section.")

            stream = nca.get_section_reader(exefs_sec)
            with Pfs0(stream) as pfs0:
                return pfs0.extract_all(exefs_dir, progress_callback=progress_callback, cancel_check=cancel_check)

    def extract_all(
        self,
        file_path: Union[str, Path],
        out_dir: Union[str, Path],
        extract_container_ncas: bool = True,
        extract_romfs_assets: bool = True,
        extract_exefs_binaries: bool = True,
        extract_metadata_icon: bool = True,
        progress_callback: Optional[Callable[[str, int, int, str, int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> Dict[str, Union[List[Path], str]]:
        """
        Performs full multi-level extraction.
        Surfaces all errors and status clearly.
        """
        path = Path(file_path)
        out_dir = Path(out_dir)
        fmt = self.detect_format(path)
        results = {}

        # 0. Always parse all tickets in container first
        tiks = self._load_container_tickets(path, fmt)

        # 1. Metadata and Icon
        if extract_metadata_icon:
            info = self.inspect(path)
            meta_dir = out_dir / "metadata"
            meta_dir.mkdir(parents=True, exist_ok=True)
            if info.icon_bytes:
                icon_path = meta_dir / "icon.jpg"
                with open(icon_path, "wb") as f:
                    f.write(info.icon_bytes)
                results["icon"] = [icon_path]
            info_json = meta_dir / "game_info.json"
            with open(info_json, "w", encoding="utf-8") as f:
                json.dump(info.to_dict(), f, indent=4)
            results["metadata"] = [info_json]

        # 2. Container Unpack (.nca files)
        if extract_container_ncas:
            def c_cb(idx, tot, fname, done, ftotal):
                if progress_callback:
                    progress_callback("Extracting NCAs", idx, tot, fname, done, ftotal)

            nca_dir = out_dir / "ncas"
            results["ncas"] = self.extract_container(path, nca_dir, decompress_ncz=True, progress_callback=c_cb, cancel_check=cancel_check)

        # 3. RomFS Assets
        if extract_romfs_assets:
            if not self.keyset.has_header_key:
                results["romfs_error"] = "Skipped: prod.keys with header_key not found."
            else:
                def r_cb(idx, tot, fname, done, ftotal):
                    if progress_callback:
                        progress_callback("Extracting RomFS Assets", idx, tot, fname, done, ftotal)

                try:
                    results["romfs"] = self.extract_romfs(path, out_dir, progress_callback=r_cb, cancel_check=cancel_check)
                except Exception as e:
                    results["romfs_error"] = str(e)

        # 4. ExeFS Executables
        if extract_exefs_binaries:
            if not self.keyset.has_header_key:
                results["exefs_error"] = "Skipped: prod.keys with header_key not found."
            else:
                def e_cb(idx, tot, fname, done, ftotal):
                    if progress_callback:
                        progress_callback("Extracting ExeFS Binaries", idx, tot, fname, done, ftotal)

                try:
                    results["exefs"] = self.extract_exefs(path, out_dir, progress_callback=e_cb, cancel_check=cancel_check)
                except Exception as e:
                    results["exefs_error"] = str(e)

        return results

    def _find_program_nca(self, path: Path, fmt: str) -> Optional[Tuple[BinaryIO, int, int]]:
        """Helper to locate the Program NCA within a package."""
        if fmt in ("NSP", "NSZ"):
            p = Pfs0(path)
            nca_entries = [e for e in p.entries if e.name.lower().endswith((".nca", ".ncz"))]
            if not nca_entries:
                p.close()
                return None

            nca_entries.sort(key=lambda x: x.size, reverse=True)

            for ent in nca_entries:
                try:
                    with Nca(p._file, keyset=self.keyset, base_offset=ent.abs_offset, size=ent.size) as nca:
                        if nca.content_type == 0:  # Program
                            return (p._file, ent.abs_offset, ent.size)
                except Exception:
                    pass

            largest = nca_entries[0]
            return (p._file, largest.abs_offset, largest.size)

        elif fmt in ("XCI", "XCZ"):
            x = Xci(path)
            sec = x.secure_partition
            if not sec:
                x.close()
                return None

            nca_entries = [e for e in sec.hfs0.entries if e.name.lower().endswith((".nca", ".ncz"))]
            if not nca_entries:
                x.close()
                return None

            nca_entries.sort(key=lambda x: x.size, reverse=True)

            for ent in nca_entries:
                try:
                    with Nca(sec.hfs0._file, keyset=self.keyset, base_offset=ent.abs_offset, size=ent.size) as nca:
                        if nca.content_type == 0:
                            return (sec.hfs0._file, ent.abs_offset, ent.size)
                except Exception:
                    pass

            largest = nca_entries[0]
            return (sec.hfs0._file, largest.abs_offset, largest.size)

        elif fmt == "NCA":
            f = open(path, "rb")
            return (f, 0, path.stat().st_size)

        return None
