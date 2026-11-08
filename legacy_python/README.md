# Nintendo Switch 1 Game Extractor

A high-performance, universal extractor, unpacker, and decryptor for **Nintendo Switch 1** game formats. It supports both a modern graphical user interface (GUI) and a full command-line interface (CLI).

---

## Supported Formats

| Extension | Format Description | Extraction Capabilities |
| :--- | :--- | :--- |
| **`.XCI`** | GameCard Image | Unpack partitions (`secure`, `update`, `normal`, `logo`), extract all NCAs, decrypt RomFS / ExeFS |
| **`.XCZ`** | Block-Compressed GameCard | Decompress NCZ to NCA, unpack partitions & game content |
| **`.NSP`** | Nintendo Submission Package (eShop) | Unpack NCAs, tickets, certs, XML metadata, decrypt RomFS / ExeFS |
| **`.NSZ`** | Compressed NSP | Decompress NCZ to NCA, unpack package contents |
| **`.NCA`** | Nintendo Content Archive | Extract sections, decrypt RomFS / ExeFS |
| **`.NCZ`** | Compressed Content Archive | Decompress zstd stream & restore exact original `.nca` |
| **`.PFS0`** | Partition File System | Extract all contained files |
| **`.HFS0`** | Hierarchical Partition File System | Extract all partition files with SHA-256 verification |
| **`.ROMFS`** | Read-Only Filesystem | Reconstruct directory hierarchy and extract game assets |
| **`.EXEFS`** | Executable Filesystem | Extract executables (`main`, `main.npdm`, `rtld`, `subsdk*`) |

---

## Features

- **Zero-Key Container Unpacking**: Unpack `.xci`, `.nsp`, `.nsz`, `.xcz`, `.pfs0`, and `.hfs0` into raw `.nca` files without requiring any cryptographic keys.
- **Deep Asset Decryption**: With `prod.keys`, decrypt Program NCAs and extract game assets (3D models, textures, audio, scripts) and executables.
- **Auto-Key Discovery**: Automatically detects and loads `prod.keys` and `title.keys` from:
  - Yuzu (`%APPDATA%/yuzu/keys/prod.keys`)
  - Ryujinx (`%APPDATA%/Ryujinx/system/prod.keys`)
  - Switch homebrew directory (`~/.switch/prod.keys`)
  - Current working directory (`./prod.keys`)
- **Metadata & Icon Viewer**: Displays localized game title, publisher, display version string, Title ID, and extracts high-resolution game icons (`icon.jpg`).
- **Decompression Engine**: Full support for both solid-stream and block-compressed (`NCZBLOCK`) zstandard compression.
- **Dual Interface**:
  - **Modern Dark GUI**: Drag-and-drop file selection, real-time progress bars, game card preview, and cancellation support.
  - **Command-Line Interface**: Batch extraction, scriptable automation, and detailed logs.

---

## Quick Start

### 1. Launch the GUI
Double-click `launch_gui.bat` or run:
```bash
python switch_extractor.py --gui
```

### 2. Command-Line Usage

#### Inspect Game Metadata & Internal Contents
```bash
python switch_extractor.py "path/to/game.xci" --info
```

#### Extract Everything (NCAs + RomFS + ExeFS + Icon)
```bash
python switch_extractor.py "path/to/game.nsp" --all --out ./extracted/my_game
```

#### Extract Game Assets Only (RomFS)
```bash
python switch_extractor.py "path/to/game.xci" --romfs --out ./game_assets
```

#### Extract Executables Only (ExeFS for modding/disassembly)
```bash
python switch_extractor.py "path/to/game.nsp" --exefs --out ./game_binaries
```

#### Extract Container NCAs (No keys required)
```bash
python switch_extractor.py "path/to/game.nsz" --ncas --out ./raw_ncas
```

#### Batch Extract a Directory of Games
```bash
python switch_extractor.py --batch "D:/SwitchGames" --out "D:/ExtractedGames"
```

---

## CLI Options Reference

```text
usage: switch_extractor.py [-h] [-g] [-i] [-o OUT] [-k KEYS]
                           [--titlekeys TITLEKEYS] [-a] [-r] [-e] [-n]
                           [--no-decompress] [--batch BATCH]
                           [file]

positional arguments:
  file                  Path to Nintendo Switch game file (.xci, .nsp, .nsz, .xcz, .nca)

options:
  -h, --help            Show this help message and exit
  -g, --gui             Launch Graphical User Interface (GUI)
  -i, --info            Inspect and display game metadata without extracting
  -o, --out OUT         Output directory for extracted files (default: ./extracted/<name>)
  -k, --keys KEYS       Custom path to prod.keys (automatically detects Yuzu/Ryujinx if omitted)
  --titlekeys TITLEKEYS Custom path to title.keys
  -a, --all             Extract everything (NCAs + RomFS Assets + ExeFS Binaries + Icons)
  -r, --romfs           Extract RomFS game assets (models, textures, audio, scripts)
  -e, --exefs           Extract ExeFS game executables (main, main.npdm, etc.)
  -n, --ncas            Extract container NCAs / partitions
  --no-decompress       Do not decompress NCZ files to standard NCA
  --batch BATCH         Batch extract all games in the specified directory
```

---

## Running Tests

Run the test suite using `pytest`:
```bash
python -m pytest tests/ -v
```

