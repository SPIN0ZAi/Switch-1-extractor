"""
Command Line Interface (CLI) for Nintendo Switch 1 Game Extractor.
"""

import argparse
import os
import sys
import time
from pathlib import Path

from switch_extractor import KeySet, SwitchExtractor


def format_bytes(num_bytes: int) -> str:
    """Formats bytes to human-readable string."""
    if num_bytes >= 1024**3:
        return f"{num_bytes / (1024**3):.2f} GB"
    elif num_bytes >= 1024**2:
        return f"{num_bytes / (1024**2):.2f} MB"
    elif num_bytes >= 1024:
        return f"{num_bytes / 1024:.2f} KB"
    return f"{num_bytes} B"


def print_banner():
    banner = r"""
  _   _ _       _                 _         ____            _ _       _     
 | \ | (_)_ __ | |_ ___ _ __   __| | ___   / ___|_      __(_) |_ ___| |__  
 |  \| | | '_ \| __/ _ \ '_ \ / _` |/ _ \  \___ \ \ /\ / /| | __/ __| '_ \ 
 | |\  | | | | | ||  __/ | | | (_| | (_) |  ___) \ V  V / | | || (__| | | |
 |_| \_|_|_| |_|\__\___|_| |_|\__,_|\___/  |____/ \_/\_/  |_|\__\___|_| |_|
           Universal .XCI / .NSP / .NSZ / .XCZ / .NCA Extractor
    """
    print(banner)


def run_cli():
    parser = argparse.ArgumentParser(
        description="Nintendo Switch 1 Game Extractor (.XCI, .XCZ, .NSP, .NSZ, .NCA, .PFS0, .HFS0, .ROMFS)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    parser.add_argument("file", nargs="?", help="Path to Nintendo Switch game file (.xci, .nsp, .nsz, .xcz, .nca)")
    parser.add_argument("-g", "--gui", action="store_true", help="Launch Graphical User Interface (GUI)")
    parser.add_argument("-i", "--info", action="store_true", help="Inspect and display game metadata without extracting")
    parser.add_argument("-o", "--out", help="Output directory for extracted files (default: ./extracted/<name>)")
    parser.add_argument("-k", "--keys", help="Custom path to prod.keys (automatically detects Yuzu/Ryujinx if omitted)")
    parser.add_argument("--titlekeys", help="Custom path to title.keys")
    parser.add_argument("-a", "--all", action="store_true", help="Extract everything (NCAs + RomFS Assets + ExeFS Binaries + Icons)")
    parser.add_argument("-r", "--romfs", action="store_true", help="Extract RomFS game assets (models, textures, audio, scripts)")
    parser.add_argument("-e", "--exefs", action="store_true", help="Extract ExeFS game executables (main, main.npdm, etc.)")
    parser.add_argument("-n", "--ncas", action="store_true", help="Extract container NCAs / partitions")
    parser.add_argument("--no-decompress", action="store_true", help="Do not decompress NCZ files to standard NCA")
    parser.add_argument("--batch", help="Batch extract all games in the specified directory")

    args = parser.parse_args()

    # If no file provided and no batch or gui flag requested, default to GUI if interactive
    if not args.file and not args.batch:
        if args.gui or sys.stdin.isatty():
            from switch_extractor.gui import launch_gui
            launch_gui()
            return
        else:
            parser.print_help()
            return

    if args.gui:
        from switch_extractor.gui import launch_gui
        launch_gui(initial_file=args.file)
        return

    print_banner()

    # Initialize KeySet
    keyset = KeySet(prod_keys_path=args.keys, title_keys_path=args.titlekeys)
    extractor = SwitchExtractor(keyset=keyset)

    if keyset.has_header_key:
        print(f"[+] prod.keys loaded from: {keyset.prod_keys_file}")
        print(f"[+] Loaded {len(keyset.keys)} keys, {len(keyset.title_keys)} title keys.")
    else:
        print("[!] Warning: prod.keys not detected. Container extraction (NCA/tickets) available; RomFS/ExeFS requires keys.")

    # Batch mode
    if args.batch:
        batch_dir = Path(args.batch)
        if not batch_dir.is_dir():
            print(f"[!] Error: Batch directory not found: {batch_dir}")
            sys.exit(1)

        valid_exts = {".xci", ".xcz", ".nsp", ".nsz", ".nca", ".pfs0", ".hfs0"}
        files = [p for p in batch_dir.iterdir() if p.suffix.lower() in valid_exts]
        print(f"[+] Found {len(files)} games in {batch_dir}")

        for idx, f in enumerate(files, 1):
            print(f"\n[{idx}/{len(files)}] Processing: {f.name}")
            process_single_file(f, args, extractor)
        return

    # Single file mode
    target_file = Path(args.file)
    if not target_file.is_file():
        print(f"[!] Error: File not found: {target_file}")
        sys.exit(1)

    process_single_file(target_file, args, extractor)


def process_single_file(file_path: Path, args, extractor: SwitchExtractor):
    print(f"\n--- Analyzing: {file_path.name} ---")
    start_time = time.time()

    info = extractor.inspect(file_path)

    print(f"  Format:       {info.format}")
    print(f"  Size:         {format_bytes(info.file_size)}")
    print(f"  Title:        {info.title_name}")
    print(f"  Title ID:     {info.title_id}")
    print(f"  Version:      {info.display_version}")
    print(f"  Publisher:    {info.publisher}")
    print(f"  Ticket:       {'Found & Loaded' if info.ticket_decrypted else 'None / Standard Crypto'}")
    print(f"  Contained:    {len(info.files_list)} internal files/partitions")

    if args.info:
        print("\nInternal Entries:")
        for ent in info.files_list[:30]:
            print(f"  • {ent}")
        if len(info.files_list) > 30:
            print(f"  ... and {len(info.files_list) - 30} more entries")
        return

    # Determine output folder
    if args.out:
        out_dir = Path(args.out)
    else:
        safe_name = "".join(c for c in info.title_name if c.isalnum() or c in (" ", "-", "_")).strip()
        safe_name = safe_name or file_path.stem
        out_dir = Path("./extracted") / f"{safe_name}_{info.title_id}"

    out_dir.mkdir(parents=True, exist_ok=True)
    print(f"\n[+] Output directory: {out_dir.resolve()}")

    last_print = [0]

    def cli_progress(step, idx, total, fname, done, tot):
        now = time.time()
        pct = (done / tot * 100) if tot > 0 else 0
        if now - last_print[0] > 0.25 or done == tot:
            last_print[0] = now
            bar_len = 25
            filled = int(bar_len * (done / tot)) if tot > 0 else 0
            bar = "#" * filled + "-" * (bar_len - filled)
            msg = f"\r[{step}] ({idx}/{total}) [{bar}] {pct:5.1f}% | {fname[:24]:<24} {format_bytes(done)}/{format_bytes(tot)}"
            sys.stdout.write(msg)
            sys.stdout.flush()

    extract_all = args.all or (not args.romfs and not args.exefs and not args.ncas)

    if extract_all:
        print("\n[*] Starting full extraction (Metadata + NCAs + RomFS + ExeFS)...")
        results = extractor.extract_all(
            file_path,
            out_dir,
            extract_container_ncas=True,
            extract_romfs_assets=True,
            extract_exefs_binaries=True,
            extract_metadata_icon=True,
            progress_callback=cli_progress,
        )
        print()
        if "romfs_error" in results:
            print(f"[!] RomFS Notice: {results['romfs_error']}")
        elif "romfs" in results:
            print(f"[v] RomFS extracted successfully: {len(results['romfs'])} asset files")

        if "exefs_error" in results:
            print(f"[!] ExeFS Notice: {results['exefs_error']}")
        elif "exefs" in results:
            print(f"[v] ExeFS extracted successfully: {len(results['exefs'])} binary files")

    else:
        results = {}
        if args.ncas:
            print("\n[*] Extracting container NCAs...")
            nca_dir = out_dir / "ncas"
            results["ncas"] = extractor.extract_container(
                file_path,
                nca_dir,
                decompress_ncz=not args.no_decompress,
                progress_callback=lambda idx, tot, fn, d, t: cli_progress("NCAs", idx, tot, fn, d, t),
            )
        if args.romfs:
            print("\n[*] Extracting RomFS game assets...")
            try:
                results["romfs"] = extractor.extract_romfs(
                    file_path,
                    out_dir,
                    progress_callback=lambda idx, tot, fn, d, t: cli_progress("RomFS", idx, tot, fn, d, t),
                )
                print(f"\n[v] RomFS extracted: {len(results['romfs'])} files")
            except Exception as e:
                print(f"\n[!] RomFS Error: {e}")

        if args.exefs:
            print("\n[*] Extracting ExeFS game binaries...")
            try:
                results["exefs"] = extractor.extract_exefs(
                    file_path,
                    out_dir,
                    progress_callback=lambda idx, tot, fn, d, t: cli_progress("ExeFS", idx, tot, fn, d, t),
                )
                print(f"\n[v] ExeFS extracted: {len(results['exefs'])} files")
            except Exception as e:
                print(f"\n[!] ExeFS Error: {e}")

    elapsed = time.time() - start_time
    print(f"\n[v] Completed in {elapsed:.2f} seconds!")
    print(f"[v] Extracted files saved to: {out_dir.resolve()}")


if __name__ == "__main__":
    run_cli()
