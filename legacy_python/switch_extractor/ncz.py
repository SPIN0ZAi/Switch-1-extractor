"""
Nintendo Switch NCZ Decompressor Module (for NSZ, XCZ, and NCZ files).
Handles zstandard decompression with both solid streams and block-compressed streams (NCZBLOCK),
and restores exact original NCA files using NCZSECTN re-encryption metadata.
"""

import os
import struct
from pathlib import Path
from typing import BinaryIO, Callable, List, Optional, Tuple, Union
import zstandard as zstd
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes


class NczSection:
    def __init__(self, offset: int, size: int, crypto_type: int, crypto_key: bytes, crypto_counter: bytes):
        self.offset = offset
        self.size = size
        self.crypto_type = crypto_type  # 1: None, 2: AES-CTR
        self.crypto_key = crypto_key
        self.crypto_counter = crypto_counter


class NczBlockHeader:
    def __init__(self, block_size: int, num_blocks: int, decompressed_size: int, block_sizes: List[int]):
        self.block_size = block_size
        self.num_blocks = num_blocks
        self.decompressed_size = decompressed_size
        self.block_sizes = block_sizes


class NczDecompressor:
    """Decompresses an NCZ file (or stream) into a standard original NCA file."""

    NCA_HEADER_SIZE = 0x4000  # 16 KB
    CHUNK_SIZE = 1024 * 1024  # 1 MB

    def __init__(self, file_or_path: Union[str, Path, BinaryIO], base_offset: int = 0, size: Optional[int] = None):
        self.base_offset = base_offset
        self.total_size = size

        if isinstance(file_or_path, (str, Path)):
            self.file_path = Path(file_or_path)
            self._file = open(self.file_path, "rb")
            self._should_close = True
            if self.total_size is None:
                self.total_size = self.file_path.stat().st_size
        else:
            self.file_path = None
            self._file = file_or_path
            self._should_close = False

        self.sections: List[NczSection] = []
        self.block_header: Optional[NczBlockHeader] = None
        self.payload_offset = 0

        self._parse_headers()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._should_close and self._file:
            self._file.close()

    def _parse_headers(self):
        self._file.seek(self.base_offset + self.NCA_HEADER_SIZE)
        raw_count = self._file.read(8)
        if len(raw_count) < 8:
            raise ValueError("Corrupt NCZ: File too small for section count.")

        section_count = struct.unpack("<Q", raw_count)[0]

        for _ in range(section_count):
            sec_raw = self._file.read(72)
            if len(sec_raw) < 72:
                raise ValueError("Corrupt NCZ: Incomplete section entry.")

            magic = sec_raw[:8]
            if magic != b"NCZSECTN":
                raise ValueError(f"Corrupt NCZ: Expected NCZSECTN magic, got {magic!r}")

            offset, size, crypto_type, _ = struct.unpack("<QQQQ", sec_raw[8:40])
            crypto_key = sec_raw[40:56]
            crypto_counter = sec_raw[56:72]

            self.sections.append(NczSection(offset, size, crypto_type, crypto_key, crypto_counter))

        # Check for optional NCZBLOCK header
        pos = self._file.tell()
        magic = self._file.read(8)
        if magic == b"NCZBLOCK":
            ver, b_type, unused, exp = struct.unpack("<BBBB", self._file.read(4))
            num_blocks, = struct.unpack("<I", self._file.read(4))
            decompressed_size, = struct.unpack("<Q", self._file.read(8))
            block_size = 1 << exp

            block_sizes = []
            for _ in range(num_blocks):
                bsz, = struct.unpack("<I", self._file.read(4))
                block_sizes.append(bsz)

            self.block_header = NczBlockHeader(block_size, num_blocks, decompressed_size, block_sizes)
            self.payload_offset = self._file.tell()
        else:
            # Solid stream; rewind the 8 bytes
            self._file.seek(pos)
            self.payload_offset = pos

    def decompress(
        self,
        out_path: Union[str, Path],
        reencrypt: bool = True,
        progress_callback: Optional[Callable[[int, int], None]] = None,
        cancel_check: Optional[Callable[[], bool]] = None
    ) -> int:
        """
        Decompresses the NCZ to out_path.
        If reencrypt is True, restores exact original byte-for-byte encrypted NCA.
        Returns total bytes written.
        """
        out_path = Path(out_path)
        out_path.parent.mkdir(parents=True, exist_ok=True)

        # 1. Copy initial 0x4000 bytes (original NCA header)
        self._file.seek(self.base_offset)
        nca_header = self._file.read(self.NCA_HEADER_SIZE)

        with open(out_path, "wb") as out_f:
            out_f.write(nca_header)
            bytes_written = len(nca_header)

            if self.block_header:
                self._decompress_blocks(out_f, reencrypt, progress_callback, cancel_check)
            else:
                self._decompress_solid(out_f, reencrypt, progress_callback, cancel_check)

        return out_path.stat().st_size

    def _decompress_solid(self, out_f: BinaryIO, reencrypt: bool, progress_callback, cancel_check):
        self._file.seek(self.payload_offset)
        dctx = zstd.ZstdDecompressor()

        # Build re-encryption cipher if sections require it
        current_offset = self.NCA_HEADER_SIZE
        # Set up streaming decompressor
        decompressor = dctx.decompressobj()

        # Calculate approximate total decompressed size if possible
        est_total = sum(s.size for s in self.sections) if self.sections else 0
        if est_total > 0:
            est_total += self.NCA_HEADER_SIZE

        # Prepare section state map
        sec_idx = 0

        while True:
            if cancel_check and cancel_check():
                raise InterruptedError("Decompression cancelled by user.")

            chunk = self._file.read(self.CHUNK_SIZE)
            if not chunk:
                break

            decompressed = decompressor.decompress(chunk)
            if decompressed:
                if reencrypt and self.sections:
                    decompressed = self._apply_reencryption(decompressed, current_offset)
                out_f.write(decompressed)
                current_offset += len(decompressed)
                if progress_callback:
                    progress_callback(current_offset, est_total)

        # Flush any remaining decompressed bytes
        decompressed = decompressor.flush()
        if decompressed:
            if reencrypt and self.sections:
                decompressed = self._apply_reencryption(decompressed, current_offset)
            out_f.write(decompressed)
            current_offset += len(decompressed)
            if progress_callback:
                progress_callback(current_offset, est_total)

    def _decompress_blocks(self, out_f: BinaryIO, reencrypt: bool, progress_callback, cancel_check):
        self._file.seek(self.payload_offset)
        dctx = zstd.ZstdDecompressor()
        current_offset = self.NCA_HEADER_SIZE
        total_decomp = self.block_header.decompressed_size + self.NCA_HEADER_SIZE

        for b_sz in self.block_header.block_sizes:
            if cancel_check and cancel_check():
                raise InterruptedError("Decompression cancelled by user.")

            chunk = self._file.read(b_sz)
            if b_sz == self.block_header.block_size:
                # Raw uncompressed block
                block_data = chunk
            else:
                block_data = dctx.decompress(chunk)

            if reencrypt and self.sections:
                block_data = self._apply_reencryption(block_data, current_offset)

            out_f.write(block_data)
            current_offset += len(block_data)
            if progress_callback:
                progress_callback(current_offset, total_decomp)

    def _apply_reencryption(self, data: bytes, current_offset: int) -> bytes:
        """Applies AES-CTR re-encryption to slice of data according to NCZ sections."""
        data_len = len(data)
        data_end = current_offset + data_len
        out = bytearray(data)

        for sec in self.sections:
            sec_start = sec.offset
            sec_end = sec.offset + sec.size

            # Check overlap between [current_offset, data_end) and [sec_start, sec_end)
            overlap_start = max(current_offset, sec_start)
            overlap_end = min(data_end, sec_end)

            if overlap_start < overlap_end and sec.crypto_type == 2:
                # Calculate relative offset within section
                rel_offset = overlap_start - sec_start
                slice_len = overlap_end - overlap_start
                slice_data = bytes(out[overlap_start - current_offset : overlap_end - current_offset])

                # Build CTR counter for this offset
                # Base counter has upper 8 bytes from crypto_counter, lower 8 bytes from (rel_offset >> 4)
                ctr = bytearray(sec.crypto_counter)
                block_num = rel_offset >> 4
                for j in range(8):
                    ctr[15 - j] = (block_num >> (j * 8)) & 0xFF

                cipher = Cipher(algorithms.AES(sec.crypto_key), modes.CTR(bytes(ctr)))
                encryptor = cipher.encryptor()
                reenc = encryptor.update(slice_data) + encryptor.finalize()

                out[overlap_start - current_offset : overlap_end - current_offset] = reenc

        return bytes(out)

