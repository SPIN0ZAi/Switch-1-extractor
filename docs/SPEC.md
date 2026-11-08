# 📖 Switch Package & Extraction Technical Specification

**Version:** 1.0 · **Target Platform:** Nintendo Switch (NX Horizon)

This document provides a technical specification of the container architectures, encryption schemes, delta relocation structures, and memory-safe streaming pipelines implemented in **Switch Extractor**.

---

## 1. Container Formats

### 1.1 PFS0 (Partition Filesystem) / `.NSP`
PFS0 is the primary submission and archive format for Horizon eShop packages.
- **Magic:** `PFS0` (`0x30534650`)
- **Header:**
  ```c
  struct Pfs0Header {
      uint32_t magic;           // "PFS0"
      uint32_t file_count;      // Number of entries in partition
      uint32_t string_table_size;// Size of filename string table
      uint32_t reserved;        // 0
  };
  ```
- **Entry Structure:**
  ```c
  struct Pfs0FileEntry {
      uint64_t data_offset;     // Relative to start of data payload
      uint64_t file_size;       // Size in bytes
      uint32_t name_offset;     // Offset into string table
      uint32_t reserved;
  };
  ```

### 1.2 HFS0 (Hashed Filesystem) / `.XCI`
GameCard dumps encapsulate an XCI header followed by an HFS0 partition table containing `root`, `update`, `normal`, and `secure` sub-partitions.
- **Magic:** `HEAD` (`0x44414548`) at offset `0x100`.
- **HFS0 Magic:** `HFS0` (`0x30534648`) preceding each GameCard partition.
- Secure partitions house encrypted NCAs containing the actual game assets.

### 1.3 NSZ / XCZ (Solid Compressed Packages)
Compressed Switch packages employing solid Zstandard (`zstd`) streaming blocks:
- **Header Magic:** `NCZSECTN` block descriptor tables.
- **Payload:** Solid compressed stream with block-aligned boundaries and optional encrypted section restoration.

---

## 2. Cryptographic Architecture

```
┌────────────────────────────────────────────────────────┐
│ NCA Header (0x400 bytes)                               │
│ Decrypted with header_key (AES-128-XTS)                │
├────────────────────────────────────────────────────────┤
│ Key Area (4 slots x 16 bytes)                          │
│ Decrypted with key_area_key_application[gen] (AES-ECB) │
├────────────────────────────────────────────────────────┤
│ Section 0 (ExeFS) ──► AES-128-CTR Streaming Decryption │
│ Section 1 (RomFS) ──► AES-128-CTR / BKTR Relocation    │
└────────────────────────────────────────────────────────┘
```

### 2.1 Header Decryption (AES-128-XTS)
- Offset `0x000` to `0x400` in every NCA is encrypted using AES-128-XTS.
- Key: 256-bit `header_key` derived from `prod.keys`.
- Sector size: 512 bytes (2 sectors per header).

### 2.2 Key Area Slot Derivation
- Slots `0x0` through `0x3` at offset `0x120` in the NCA header hold encrypted content keys.
- Decrypted using `key_area_key_application_{gen:02x}` (or `ocean` / `system`).
- Generation is indexed by the `key_generation` byte at offset `0x205` or `key_generation_2` at offset `0x220`.

### 2.3 Ticket Decryption (`.tik`)
- Standard retail eShop tickets store title keys encrypted under `titlekek_{gen:02x}` (AES-128-CBC with null IV).

---

## 3. BKTR Patch Relocation & RomFS Merging

Game Updates (NCA `fs_type == 0`, `crypt_type == 4`) utilize the **BKTR** subsystem to store delta patches rather than duplicating identical assets.

### 3.1 Bucket Tree Relocation
A BKTR NCA contains two relational bucket trees:
1. **Subsection Bucket Tree**: Maps virtual offsets to physical encryption counters and encryption modes.
2. **Relocation Bucket Tree**: Contains patch relocation entries:
   ```c
   struct BktrRelocationEntry {
       uint64_t virt_offset;  // Destination virtual offset in merged RomFS
       uint64_t phys_offset;  // Source offset in base or patch NCA
       uint32_t is_patch;     // 0 = read from Base RomFS, 1 = read from Patch RomFS
   };
   ```

### 3.2 Dynamic Stream Resolution
During RomFS extraction, read requests query the relocation table:
- If `is_patch == 0`, the engine reads from the Base Game's RomFS section at `phys_offset`.
- If `is_patch == 1`, the engine reads from the Patch NCA at `phys_offset` with the appropriate BKTR CTR counter adjustments.
- This produces a 100% complete, patched RomFS tree without intermediate disk writes or file copies.

---

## 4. Multi-Slot Extraction Hierarchy

```
Output_Directory/
├── romfs/                  # Merged Base + Update RomFS tree
│   ├── Data/
│   ├── Sound/
│   └── Shader/
├── exefs/                  # Latest executable binaries
│   ├── main
│   ├── main.npdm
│   ├── rtld
│   └── subsdk*
└── dlc/                    # Extracted DLC contents
    ├── DLC_01_0100000000010001/
    │   └── romfs/
    └── DLC_02_0100000000010002/
        └── romfs/
```

---

## 5. In-Place Self-Updating Pipeline

Windows places an exclusive file lock on executing binaries. To achieve atomic, seamless self-updating:

```
[ Running SwitchExtractor.exe ]
           │
           ▼
1. Rename current binary: SwitchExtractor.exe ──► SwitchExtractor.exe.old
           │
           ▼
2. Write new binary stream ──► SwitchExtractor.exe
           │
           ▼
3. Spawn new SwitchExtractor.exe child process
           │
           ▼
4. Terminate old process gracefully
```
- On the subsequent launch, the application cleans up any existing `.old` artifacts.
