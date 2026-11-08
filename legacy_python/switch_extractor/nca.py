"""
Nintendo Switch NCA (Nintendo Content Archive) Parser and Decryptor.
Handles header decryption (AES-XTS), key derivation, section parsing, and streaming section decryption.
"""

import io
import os
import struct
from pathlib import Path
from typing import BinaryIO, Callable, Dict, List, Optional, Tuple, Union
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

from switch_extractor.keys import KeySet
from switch_extractor.pfs0 import Pfs0


CONTENT_TYPES = {
    0: "Program",
    1: "Meta",
    2: "Control",
    3: "Manual",
    4: "SubSDK",
    5: "PublicData",
}

FS_TYPES = {
    0: "RomFS",
    1: "PartitionFS",
}

CRYPT_TYPES = {
    1: "None",
    2: "AES-XTS",
    3: "AES-CTR",
    4: "AES-BKTR",
}


class NcaSection:
    def __init__(
        self,
        index: int,
        media_start: int,
        media_end: int,
        fs_type: int,
        crypt_type: int,
        section_ctr: bytes,
        raw_fs_header: bytes,
        content_offset: int,
        content_size: int,
    ):
        self.index = index
        self.media_start = media_start
        self.media_end = media_end
        self.offset = media_start * 0x200
        self.size = (media_end - media_start) * 0x200
        self.fs_type = fs_type
        self.crypt_type = crypt_type
        self.section_ctr = section_ctr
        self.raw_fs_header = raw_fs_header
        self.content_offset = content_offset  # Logical offset inside section (e.g. RomFS/PFS0 start)
        self.content_size = content_size

    @property
    def fs_type_name(self) -> str:
        return FS_TYPES.get(self.fs_type, f"Unknown({self.fs_type})")

    @property
    def crypt_type_name(self) -> str:
        return CRYPT_TYPES.get(self.crypt_type, f"Unknown({self.crypt_type})")

    def __repr__(self):
        return f"<NcaSection {self.index}: {self.fs_type_name} ({self.crypt_type_name}) size={self.size:,} bytes>"


class Nca:
    """Parser and decryptor for Nintendo Content Archives (.nca)."""

    CHUNK_SIZE = 1024 * 1024  # 1 MB

    def __init__(
        self,
        file_or_path: Union[str, Path, BinaryIO],
        keyset: Optional[KeySet] = None,
        base_offset: int = 0,
        size: Optional[int] = None
    ):
        self.base_offset = base_offset
        self.keyset = keyset or KeySet()

        if isinstance(file_or_path, (str, Path)):
            self.file_path = Path(file_or_path)
            self._file = open(self.file_path, "rb")
            self._should_close = True
            self.total_size = size or self.file_path.stat().st_size
        else:
            self.file_path = None
            self._file = file_or_path
            self._should_close = False
            self.total_size = size or 0

        self.magic = b""
        self.content_type = 0
        self.key_generation = 0
        self.kaek_index = 0
        self.title_id = 0
        self.rights_id = b"\x00" * 16
        self.has_rights_id = False
        self.decrypted_key_area: List[bytes] = []

        self.sections: List[NcaSection] = []

        self._parse()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._should_close and self._file:
            self._file.close()

    @property
    def content_type_name(self) -> str:
        return CONTENT_TYPES.get(self.content_type, f"Unknown({self.content_type})")

    @property
    def title_id_hex(self) -> str:
        return f"{self.title_id:016X}"

    def _decrypt_header_block(self, enc_header: bytes) -> bytes:
        """Decrypts the NCA header (bytes 0x200..0x400) using AES-XTS-128."""
        if not self.keyset.has_header_key:
            return enc_header

        header_key = self.keyset.get_header_key()
        # Sector 1 for bytes 0x200..0x400 (big-endian tweak in Nintendo Switch specification)
        tweak = (1).to_bytes(16, byteorder="big")
        cipher = Cipher(algorithms.AES(header_key), modes.XTS(tweak))
        decryptor = cipher.decryptor()
        return decryptor.update(enc_header) + decryptor.finalize()

    def _decrypt_fs_header(self, enc_fs_header: bytes, section_idx: int) -> bytes:
        """Decrypts an NCA FsHeader (at 0x400 + i * 0x200) using AES-XTS-128."""
        if not self.keyset.has_header_key:
            return enc_fs_header

        header_key = self.keyset.get_header_key()
        # Sector index is (0x400 + section_idx * 0x200) >> 9 -> 2 + section_idx (big-endian tweak)
        sector_idx = 2 + section_idx
        tweak = sector_idx.to_bytes(16, byteorder="big")
        cipher = Cipher(algorithms.AES(header_key), modes.XTS(tweak))
        decryptor = cipher.decryptor()
        return decryptor.update(enc_fs_header) + decryptor.finalize()

    def _parse(self):
        self._file.seek(self.base_offset)
        raw_0x400 = self._file.read(0x400)
        if len(raw_0x400) < 0x400:
            raise ValueError("File is too small to be a valid NCA archive.")

        # Check if bytes 0x200..0x204 already contain NCA magic (unencrypted)
        body = raw_0x400[0x200:0x400]
        if body[:4] in (b"NCA3", b"NCA2", b"NCA0"):
            dec_body = body
        else:
            dec_body = self._decrypt_header_block(body)

        magic = dec_body[:4]
        if magic not in (b"NCA3", b"NCA2", b"NCA0"):
            raise ValueError(f"Invalid NCA magic: {magic!r}. Missing header_key or corrupted file.")

        self.magic = magic
        dist_type = dec_body[4]
        self.content_type = dec_body[5]
        self.kaek_index = dec_body[7]

        # Key generation calculation:
        # At 0x206 (offset 6 in dec_body): legacy crypto_type
        # At 0x220 (offset 0x20 in dec_body): modern crypto_type2
        crypto1 = dec_body[6]
        crypto2 = dec_body[0x20] if len(dec_body) > 0x20 else 0
        key_gen = max(crypto1, crypto2)
        self.key_generation = max(0, key_gen - 1) if key_gen > 0 else 0

        title_id, content_idx = struct.unpack("<QI", dec_body[0x10:0x1C])
        self.title_id = title_id
        self.rights_id = dec_body[0x30:0x40]
        self.has_rights_id = any(b != 0 for b in self.rights_id)

        # Parse key area (0x100 bytes into dec_body -> 0x300 in file)
        enc_key_area = dec_body[0x100:0x140]
        if not self.has_rights_id:
            # Standard application crypto (Key Area)
            self.decrypted_key_area = self.keyset.decrypt_key_area(
                enc_key_area, self.key_generation, "application"
            )

        # Section table is at offset 0x40 of dec_body (0x240 in file)
        sec_table_raw = dec_body[0x40:0x80]

        # Read the 4 FsHeaders from 0x400 to 0xC00
        self._file.seek(self.base_offset + 0x400)
        raw_fs_headers = self._file.read(0x800)

        for i in range(4):
            ent = sec_table_raw[i * 16 : (i + 1) * 16]
            m_start, m_end = struct.unpack("<II", ent[:8])
            if m_start == 0 and m_end == 0:
                continue

            fs_raw = raw_fs_headers[i * 0x200 : (i + 1) * 0x200]
            dec_fs = self._decrypt_fs_header(fs_raw, i)

            version, part_type, fs_type, crypt_type = struct.unpack("<HBBB", dec_fs[:5])
            section_ctr = dec_fs[0x140:0x148]

            content_off = 0
            content_sz = (m_end - m_start) * 0x200

            # PartitionFS (ExeFS): part_type == 1 or fs_type in (1, 2)
            if part_type == 1 or fs_type in (1, 2):
                if len(dec_fs) >= 0x50:
                    pfs_off, pfs_sz = struct.unpack("<QQ", dec_fs[0x40:0x50])
                    if pfs_sz > 0:
                        content_off = pfs_off
                        content_sz = pfs_sz

            # RomFS (IVFC): dec_fs starts with 'IVFC' at 0x08 or part_type == 0 / fs_type == 3
            if dec_fs[0x08:0x0C] == b"IVFC" and len(dec_fs) >= 0x20:
                num_levels, = struct.unpack("<I", dec_fs[0x14:0x18])
                if 1 <= num_levels <= 7:
                    # Scan backwards for the first level with non-zero size (RomFS data level)
                    for lvl in range(num_levels - 1, -1, -1):
                        lvl_off = 0x18 + lvl * 24
                        if len(dec_fs) >= lvl_off + 16:
                            log_off, lvl_sz = struct.unpack("<QQ", dec_fs[lvl_off : lvl_off + 16])
                            if lvl_sz > 0:
                                content_off = log_off
                                content_sz = lvl_sz
                                break

            # Correct fs_type for NcaSection classification:
            # 0: RomFS, 1: PartitionFS
            normalized_fs_type = 0 if (dec_fs[0x08:0x0C] == b"IVFC" or part_type == 0) else 1

            sec = NcaSection(
                index=i,
                media_start=m_start,
                media_end=m_end,
                fs_type=normalized_fs_type,
                crypt_type=crypt_type,
                section_ctr=section_ctr,
                raw_fs_header=dec_fs,
                content_offset=content_off,
                content_size=content_sz,
            )
            self.sections.append(sec)

    def get_section_key(self, section: NcaSection) -> Optional[bytes]:
        """Resolves the 16-byte decryption key for this section."""
        if section.crypt_type == 1:
            return None  # Plaintext
        if self.has_rights_id:
            return self.keyset.get_title_key(self.rights_id, self.key_generation)
        if self.decrypted_key_area and len(self.decrypted_key_area) > 2:
            key2 = self.decrypted_key_area[2]
            if any(b != 0 for b in key2):
                return key2
        if self.decrypted_key_area and len(self.decrypted_key_area) > self.kaek_index:
            key_kaek = self.decrypted_key_area[self.kaek_index]
            if any(b != 0 for b in key_kaek):
                return key_kaek
        return None

    def create_ctr_for_offset(self, section: NcaSection, abs_nca_offset: int) -> bytes:
        """Constructs the 16-byte AES-CTR IV/counter for an absolute offset in the NCA."""
        ctr = bytearray(16)
        # Upper 8 bytes: reversed section_ctr
        for j in range(8):
            ctr[j] = section.section_ctr[7 - j]
        # Lower 8 bytes: offset >> 4 (big-endian)
        ofs = abs_nca_offset >> 4
        for j in range(8):
            ctr[15 - j] = (ofs >> (j * 8)) & 0xFF
        return bytes(ctr)

    def extract_section_raw(
        self,
        section: NcaSection,
        out_path: Union[str, Path],
        decrypt: bool = True,
        progress_callback: Optional[Callable[[int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None,
    ) -> int:
        """Extracts a section to disk. If decrypt is True, streams through AES-CTR decryptor."""
        out_path = Path(out_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        key = self.get_section_key(section) if decrypt else None
        should_decrypt = decrypt and (section.crypt_type == 3) and (key is not None)

        if decrypt and section.crypt_type in (2, 3) and key is None:
            raise PermissionError(
                f"Missing key to decrypt Section {section.index} ({section.fs_type_name}). "
                f"Rights ID: {self.rights_id.hex() if self.has_rights_id else 'None'}"
            )

        sec_abs_start = self.base_offset + section.offset
        sec_size = section.size

        self._file.seek(sec_abs_start)
        remaining = sec_size
        written = 0

        if should_decrypt:
            initial_ctr = self.create_ctr_for_offset(section, section.offset)
            cipher = Cipher(algorithms.AES(key), modes.CTR(initial_ctr))
            decryptor = cipher.decryptor()
        else:
            decryptor = None

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

                if decryptor:
                    chunk = decryptor.update(chunk)

                out_f.write(chunk)
                remaining -= len(chunk)
                written += len(chunk)
                if progress_callback:
                    progress_callback(written, sec_size)

        return written

    def get_section_reader(self, section: NcaSection) -> "NcaSectionReader":
        """Returns a seekable/readable stream over the decrypted section content."""
        key = self.get_section_key(section)
        if section.crypt_type in (2, 3) and key is None:
            raise PermissionError(
                f"Cannot decrypt {section.fs_type_name} (Section {section.index}): "
                f"Missing Title Key for Rights ID: {self.rights_id.hex() if self.has_rights_id else 'None'}."
            )
        return NcaSectionReader(self._file, self.base_offset, section, key)


class NcaSectionReader(io.RawIOBase):
    """Provides seekable and readable stream over an NCA section with on-the-fly AES-CTR decryption."""

    def __init__(self, raw_file: BinaryIO, base_offset: int, section: NcaSection, key: Optional[bytes]):
        self.raw_file = raw_file
        self.base_offset = base_offset
        self.section = section
        self.key = key
        self.content_offset = section.content_offset
        self.content_size = section.content_size
        self.pos = 0

    def readable(self) -> bool:
        return True

    def seekable(self) -> bool:
        return True

    def seek(self, offset: int, whence: int = io.SEEK_SET) -> int:
        if whence == io.SEEK_SET:
            self.pos = offset
        elif whence == io.SEEK_CUR:
            self.pos += offset
        elif whence == io.SEEK_END:
            self.pos = self.content_size + offset
        self.pos = max(0, min(self.pos, self.content_size))
        return self.pos

    def tell(self) -> int:
        return self.pos

    def readinto(self, b) -> int:
        if self.pos >= self.content_size:
            return 0

        to_read = min(len(b), self.content_size - self.pos)
        if to_read <= 0:
            return 0

        # Physical offset inside NCA for current position
        abs_pos = self.section.offset + self.content_offset + self.pos
        aligned_pos = abs_pos & ~0xF
        align_diff = abs_pos - aligned_pos

        self.raw_file.seek(self.base_offset + aligned_pos)
        raw_bytes = self.raw_file.read(to_read + align_diff)

        if self.section.crypt_type == 3 and self.key:
            ctr = bytearray(16)
            for j in range(8):
                ctr[j] = self.section.section_ctr[7 - j]
            block_num = aligned_pos >> 4
            for j in range(8):
                ctr[15 - j] = (block_num >> (j * 8)) & 0xFF

            cipher = Cipher(algorithms.AES(self.key), modes.CTR(bytes(ctr)))
            decryptor = cipher.decryptor()
            decrypted = decryptor.update(raw_bytes)
            data = decrypted[align_diff : align_diff + to_read]
        else:
            data = raw_bytes[align_diff : align_diff + to_read]

        b[: len(data)] = data
        self.pos += len(data)
        return len(data)
