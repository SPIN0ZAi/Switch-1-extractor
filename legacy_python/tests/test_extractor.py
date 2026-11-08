"""
Unit and Integration Tests for Nintendo Switch 1 Game Extractor.
Tests PFS0, HFS0, XCI, RomFS, and KeySet parsing & extraction.
"""

import hashlib
import io
import os
import struct
import tempfile
from pathlib import Path
import pytest

from switch_extractor.keys import KeySet
from switch_extractor.pfs0 import Pfs0
from switch_extractor.xci import Xci
from switch_extractor.romfs import RomFs
from switch_extractor.nacp import Nacp
from switch_extractor.core import SwitchExtractor


def create_mock_pfs0(files_dict: dict, is_hfs0: bool = False) -> bytes:
    """Helper to generate a syntactically valid PFS0 or HFS0 container."""
    file_count = len(files_dict)
    entry_size = 0x40 if is_hfs0 else 0x18
    magic = b"HFS0" if is_hfs0 else b"PFS0"

    string_table = b""
    entries = []
    current_data_offset = 0

    for name, data in files_dict.items():
        name_bytes = name.encode("utf-8") + b"\x00"
        name_offset = len(string_table)
        string_table += name_bytes

        file_size = len(data)
        if not is_hfs0:
            entry = struct.pack("<QQII", current_data_offset, file_size, name_offset, 0)
        else:
            sha = hashlib.sha256(data).digest()
            entry = struct.pack("<QQIIQ32s", current_data_offset, file_size, name_offset, file_size, 0, sha)

        entries.append(entry)
        current_data_offset += file_size

    header_size = 0x10 + (file_count * entry_size) + len(string_table)
    header = struct.pack("<4sIII", magic, file_count, len(string_table), 0)
    for ent in entries:
        header += ent
    header += string_table

    data_payload = b"".join(files_dict.values())
    return header + data_payload


def test_keyset_loading():
    """Verify KeySet discovers installed emulator keys."""
    ks = KeySet()
    assert ks.has_header_key is True
    assert len(ks.keys) > 100
    assert ks.get_header_key() is not None
    assert len(ks.get_header_key()) == 32


def test_pfs0_packing_and_extraction(tmp_path):
    """Verify PFS0 parsing and streaming extraction."""
    mock_files = {
        "program.nca": b"Dummy NCA Program Data 12345",
        "control.nca": b"Dummy NCA Control Data 67890",
        "game.cnmt.xml": b"<CNMT>Metadata XML</CNMT>",
    }

    pfs0_bytes = create_mock_pfs0(mock_files, is_hfs0=False)
    pfs0_file = tmp_path / "test.nsp"
    pfs0_file.write_bytes(pfs0_bytes)

    with Pfs0(pfs0_file) as p:
        assert len(p.entries) == 3
        assert p.is_hfs0 is False
        assert p.get_entry("program.nca") is not None
        assert p.get_entry("program.nca").size == len(mock_files["program.nca"])

        # Test extraction
        out_dir = tmp_path / "extracted_pfs0"
        extracted = p.extract_all(out_dir)
        assert len(extracted) == 3
        assert (out_dir / "program.nca").read_bytes() == mock_files["program.nca"]
        assert (out_dir / "game.cnmt.xml").read_bytes() == mock_files["game.cnmt.xml"]


def test_hfs0_parsing(tmp_path):
    """Verify HFS0 parsing and extraction."""
    mock_files = {
        "secure_nca_0.nca": b"Encrypted NCA Section 0",
        "secure_nca_1.nca": b"Encrypted NCA Section 1",
    }

    hfs0_bytes = create_mock_pfs0(mock_files, is_hfs0=True)
    hfs0_file = tmp_path / "partition.hfs0"
    hfs0_file.write_bytes(hfs0_bytes)

    with Pfs0(hfs0_file) as h:
        assert h.is_hfs0 is True
        assert len(h.entries) == 2
        entry = h.get_entry("secure_nca_0.nca")
        assert entry is not None
        assert entry.sha256 == hashlib.sha256(mock_files["secure_nca_0.nca"]).digest()


def test_xci_parsing_and_extraction(tmp_path):
    """Verify XCI image layout, partition parsing, and secure NCA extraction."""
    # 1. Create sub-partitions
    secure_sub = create_mock_pfs0({"game_data.nca": b"NCA in secure partition"}, is_hfs0=True)
    update_sub = create_mock_pfs0({"update.nca": b"System update"}, is_hfs0=True)

    # 2. Create Root HFS0 containing 'secure' and 'update'
    root_hfs0 = create_mock_pfs0({"secure": secure_sub, "update": update_sub}, is_hfs0=True)

    # 3. Create XCI container
    # Untrimmed XCI has HEAD magic at 0x1100, Root HFS0 at 0xF000
    xci_data = bytearray(0x10000 + len(root_hfs0))

    # Write CardHeader at 0x1000
    # Signature: 0x100 bytes (0x1000..0x1100)
    # Magic "HEAD" at 0x1100
    xci_data[0x1100:0x1104] = b"HEAD"
    # PartitionFsHeaderAddress at 0x1130 (u64): 0xF000
    struct.pack_into("<QQ", xci_data, 0x1130, 0xF000, len(root_hfs0))

    # Write Root HFS0 at 0xF000
    xci_data[0xF000 : 0xF000 + len(root_hfs0)] = root_hfs0

    xci_file = tmp_path / "game.xci"
    xci_file.write_bytes(bytes(xci_data))

    # Test XCI parser
    with Xci(xci_file) as x:
        assert "secure" in x.partitions
        assert "update" in x.partitions
        sec = x.secure_partition
        assert sec is not None
        assert len(sec.hfs0.entries) == 1
        assert sec.hfs0.entries[0].name == "game_data.nca"

        # Test extraction of secure partition
        out_dir = tmp_path / "extracted_xci"
        extracted = x.extract_secure_partition(out_dir)
        assert len(extracted) == 1
        assert (out_dir / "game_data.nca").read_bytes() == b"NCA in secure partition"


def test_romfs_parsing_and_hierarchy(tmp_path):
    """Verify RomFS Level 3 header, Directory Table, File Table, and extraction."""
    # Build a small RomFS layout:
    # Directories:
    # 0: root "" (parent 0xFFFFFFFF, child_dir 0x18, child_file 0xFFFFFFFF)
    # 0x18: "models" (parent 0, child_dir 0xFFFFFFFF, child_file 0)
    # Files:
    # 0: "mario.obj" (parent 0x18, size 16, data_off 0)
    # 0x30: "peach.obj" (parent 0x18, size 16, data_off 16)

    # Dir Table
    d0 = struct.pack("<IIIIII", 0xFFFFFFFF, 0xFFFFFFFF, 0x18, 0xFFFFFFFF, 0, 0)
    d1_name = b"models"
    d1_pad = (0x18 + len(d1_name) + 3) & ~3
    d1 = struct.pack("<IIIIII", 0, 0xFFFFFFFF, 0xFFFFFFFF, 0, 0, len(d1_name)) + d1_name
    d1 += b"\x00" * (d1_pad - len(d1))
    dir_table = d0 + d1

    # File Table
    f1_name = b"mario.obj"
    f1_pad = (0x20 + len(f1_name) + 3) & ~3
    f1 = struct.pack("<IIQQII", 0x18, 0x30, 0, 16, 0, len(f1_name)) + f1_name
    f1 += b"\x00" * (f1_pad - len(f1))

    f2_name = b"peach.obj"
    f2_pad = (0x20 + len(f2_name) + 3) & ~3
    f2 = struct.pack("<IIQQII", 0x18, 0xFFFFFFFF, 16, 16, 0, len(f2_name)) + f2_name
    f2 += b"\x00" * (f2_pad - len(f2))
    file_table = f1 + f2

    # File Data
    file_data = b"Mario 3D Mesh Data!"[:16] + b"Peach 3D Mesh Data!"[:16]

    # RomFS Header (0x50 bytes)
    hdr_sz = 0x50
    dir_hash_off = hdr_sz
    dir_hash_sz = 0
    dir_table_off = dir_hash_off + dir_hash_sz
    dir_table_sz = len(dir_table)
    file_hash_off = dir_table_off + dir_table_sz
    file_hash_sz = 0
    file_table_off = file_hash_off + file_hash_sz
    file_table_sz = len(file_table)
    file_data_off = file_table_off + file_table_sz

    romfs_hdr = struct.pack(
        "<QQQQQQQQQQ",
        hdr_sz,
        dir_hash_off,
        dir_hash_sz,
        dir_table_off,
        dir_table_sz,
        file_hash_off,
        file_hash_sz,
        file_table_off,
        file_table_sz,
        file_data_off,
    )

    romfs_bytes = romfs_hdr + dir_table + file_table + file_data
    romfs_file = tmp_path / "test.romfs"
    romfs_file.write_bytes(romfs_bytes)

    with RomFs(romfs_file) as r:
        assert len(r.files) == 2
        f = r.get_file("models/mario.obj")
        assert f is not None
        assert f.size == 16
        assert r.read_file(f) == b"Mario 3D Mesh Da"

        out_dir = tmp_path / "extracted_romfs"
        extracted = r.extract_all(out_dir)
        assert len(extracted) == 2
        assert (out_dir / "models" / "mario.obj").is_file()
        assert (out_dir / "models" / "mario.obj").read_bytes() == b"Mario 3D Mesh Da"


def test_nacp_parsing():
    """Verify NACP title and publisher parsing."""
    raw = bytearray(0x4000)
    # AmericanEnglish title at 0x0
    raw[0:18] = "Super Mario Odyssey".encode("utf-8")
    # AmericanEnglish publisher at 0x200
    raw[0x200:0x208] = "Nintendo".encode("utf-8")
    # Display version at 0x3060
    raw[0x3060:0x3065] = "1.3.0".encode("ascii")
    # Title ID at 0x3070
    struct.pack_into("<Q", raw, 0x3070, 0x0100000000010000)

    nacp = Nacp(bytes(raw))
    assert nacp.get_preferred_title() == "Super Mario Odyssey"
    assert nacp.get_preferred_publisher() == "Nintendo"
    assert nacp.display_version == "1.3.0"
    assert nacp.title_id_hex == "0100000000010000"


def test_switch_extractor_core_integration(tmp_path):
    """Test high-level SwitchExtractor engine on mock NSP and XCI."""
    extractor = SwitchExtractor()

    # 1. Test NSP detection and container extraction
    nsp_files = {
        "0100000000010000.nca": b"Mock NCA Data",
        "0100000000010001.nca": b"Mock Control NCA Data",
    }
    nsp_file = tmp_path / "game.nsp"
    nsp_file.write_bytes(create_mock_pfs0(nsp_files))

    assert extractor.detect_format(nsp_file) == "NSP"

    info = extractor.inspect(nsp_file)
    assert info.format == "NSP"
    assert len(info.files_list) == 2

    out_nsp = tmp_path / "out_nsp"
    extracted = extractor.extract_container(nsp_file, out_nsp)
    assert len(extracted) == 2
    assert (out_nsp / "0100000000010000.nca").read_bytes() == b"Mock NCA Data"


def test_common_ticket_parsing(tmp_path):
    """Verify common ticket cleartext titlekey extraction."""
    ks = KeySet()
    # 0x400 bytes ticket
    raw_tik = bytearray(0x400)
    mock_title_key = bytes.fromhex("0123456789abcdef0123456789abcdef")
    mock_rights_id = bytes.fromhex("01000000000100000000000000000000")

    # Set titlekey at 0x180 (first 16 bytes), rest zeroes
    raw_tik[0x180:0x190] = mock_title_key
    # Set rights_id at 0x2A0
    raw_tik[0x2A0:0x2B0] = mock_rights_id

    res = ks.decrypt_ticket_title_key(bytes(raw_tik))
    assert res is not None
    rid, tkey = res
    assert rid == mock_rights_id
    assert tkey == mock_title_key
    assert ks.get_title_key(mock_rights_id) == mock_title_key
