"""
Graphical User Interface (GUI) for Nintendo Switch 1 Game Extractor.
Modern dark themed, high-DPI aware, with game icon preview and live progress.
"""

import ctypes
import io
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Optional

import tkinter as tk
from tkinter import ttk, filedialog, messagebox
from PIL import Image, ImageTk

from switch_extractor.keys import KeySet
from switch_extractor.core import SwitchExtractor, SwitchGameInfo


def enable_high_dpi():
    """Enables Windows Per-Monitor DPI awareness for sharp rendering."""
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(1)
    except Exception:
        try:
            ctypes.windll.user32.SetProcessDPIAware()
        except Exception:
            pass


class SwitchExtractorGUI:
    """Modern Dark Themed GUI for Nintendo Switch 1 Game Extractor."""

    # Color Palette (Dark modern theme)
    BG_DARK = "#121316"
    BG_CARD = "#1a1b20"
    BG_INPUT = "#22242b"
    ACCENT = "#6366f1"       # Indigo
    ACCENT_HOVER = "#4f46e5"
    SUCCESS = "#10b981"      # Emerald
    WARNING = "#f59e0b"      # Amber
    DANGER = "#ef4444"       # Rose
    TEXT_MAIN = "#f3f4f6"
    TEXT_MUTED = "#9ca3af"
    BORDER = "#2e323b"

    def __init__(self, root: tk.Tk, initial_file: Optional[str] = None):
        self.root = root
        self.root.title("Nintendo Switch 1 Game Extractor")
        self.root.geometry("980x780")
        self.root.minsize(860, 680)
        self.root.configure(bg=self.BG_DARK)

        # State
        self.keyset = KeySet()
        self.extractor = SwitchExtractor(keyset=self.keyset)
        self.current_info: Optional[SwitchGameInfo] = None
        self.is_extracting = False
        self.cancel_requested = False
        self.output_dir = Path("./extracted")
        self.icon_photo: Optional[ImageTk.PhotoImage] = None

        self._setup_styles()
        self._build_ui()

        # Update key badge
        self._update_key_status()

        # Load initial file if passed
        if initial_file and Path(initial_file).is_file():
            self._select_file_path(Path(initial_file))

    def _setup_styles(self):
        self.style = ttk.Style()
        self.style.theme_use("clam")

        # Configure TTK styles
        self.style.configure(".", background=self.BG_DARK, foreground=self.TEXT_MAIN)
        self.style.configure("TFrame", background=self.BG_DARK)
        self.style.configure("Card.TFrame", background=self.BG_CARD)

        self.style.configure(
            "TLabel",
            background=self.BG_DARK,
            foreground=self.TEXT_MAIN,
            font=("Segoe UI", 10),
        )
        self.style.configure(
            "Card.TLabel",
            background=self.BG_CARD,
            foreground=self.TEXT_MAIN,
            font=("Segoe UI", 10),
        )
        self.style.configure(
            "Muted.TLabel",
            background=self.BG_CARD,
            foreground=self.TEXT_MUTED,
            font=("Segoe UI", 9),
        )
        self.style.configure(
            "Title.TLabel",
            background=self.BG_CARD,
            foreground=self.TEXT_MAIN,
            font=("Segoe UI", 14, "bold"),
        )
        self.style.configure(
            "Badge.TLabel",
            background=self.BG_INPUT,
            foreground=self.ACCENT,
            font=("Segoe UI", 9, "bold"),
            padding=(6, 2),
        )

        # Progress bar
        self.style.configure(
            "Horizontal.TProgressbar",
            troughcolor=self.BG_INPUT,
            background=self.ACCENT,
            bordercolor=self.BORDER,
            lightcolor=self.ACCENT,
            darkcolor=self.ACCENT,
        )

        # Checkbutton
        self.style.configure(
            "TCheckbutton",
            background=self.BG_CARD,
            foreground=self.TEXT_MAIN,
            font=("Segoe UI", 10),
        )
        self.style.map(
            "TCheckbutton",
            background=[("active", self.BG_CARD)],
            foreground=[("active", self.TEXT_MAIN)],
        )

    def _build_ui(self):
        # Main Container with padding
        main_frame = ttk.Frame(self.root, padding=16)
        main_frame.pack(fill=tk.BOTH, expand=True)

        # 1. Header & Keys Bar
        header_frame = ttk.Frame(main_frame)
        header_frame.pack(fill=tk.X, pady=(0, 12))

        app_title = tk.Label(
            header_frame,
            text="Switch 1 Extractor",
            font=("Segoe UI", 16, "bold"),
            bg=self.BG_DARK,
            fg=self.TEXT_MAIN,
        )
        app_title.pack(side=tk.LEFT)

        app_sub = tk.Label(
            header_frame,
            text="Universal .XCI / .NSP / .NSZ / .XCZ / .NCA Unpacker",
            font=("Segoe UI", 9),
            bg=self.BG_DARK,
            fg=self.TEXT_MUTED,
        )
        app_sub.pack(side=tk.LEFT, padx=(10, 0), pady=(4, 0))

        # Keys status pill on the right
        self.key_status_btn = tk.Button(
            header_frame,
            text="Checking keys...",
            command=self._browse_keys,
            font=("Segoe UI", 9),
            relief=tk.FLAT,
            cursor="hand2",
            padx=10,
            pady=3,
        )
        self.key_status_btn.pack(side=tk.RIGHT)

        # 2. File Selection Card
        file_card = tk.Frame(main_frame, bg=self.BG_CARD, highlightbackground=self.BORDER, highlightthickness=1)
        file_card.pack(fill=tk.X, pady=(0, 10), ipadx=12, ipady=10)

        f_label = tk.Label(file_card, text="Game File:", font=("Segoe UI", 10, "bold"), bg=self.BG_CARD, fg=self.TEXT_MAIN)
        f_label.grid(row=0, column=0, sticky="w", padx=(10, 8), pady=4)

        self.file_entry = tk.Entry(
            file_card,
            font=("Segoe UI", 10),
            bg=self.BG_INPUT,
            fg=self.TEXT_MAIN,
            insertbackground=self.TEXT_MAIN,
            relief=tk.FLAT,
            highlightbackground=self.BORDER,
            highlightthickness=1,
        )
        self.file_entry.grid(row=0, column=1, sticky="ew", padx=(0, 8), pady=4)
        file_card.grid_columnconfigure(1, weight=1)

        btn_browse_file = tk.Button(
            file_card,
            text="Browse Game...",
            command=self._browse_game_file,
            bg=self.ACCENT,
            fg="#ffffff",
            activebackground=self.ACCENT_HOVER,
            activeforeground="#ffffff",
            font=("Segoe UI", 9, "bold"),
            relief=tk.FLAT,
            cursor="hand2",
            padx=12,
            pady=4,
        )
        btn_browse_file.grid(row=0, column=2, padx=(0, 10), pady=4)

        # Output directory row
        o_label = tk.Label(file_card, text="Output Folder:", font=("Segoe UI", 10, "bold"), bg=self.BG_CARD, fg=self.TEXT_MAIN)
        o_label.grid(row=1, column=0, sticky="w", padx=(10, 8), pady=4)

        self.out_entry = tk.Entry(
            file_card,
            font=("Segoe UI", 10),
            bg=self.BG_INPUT,
            fg=self.TEXT_MAIN,
            insertbackground=self.TEXT_MAIN,
            relief=tk.FLAT,
            highlightbackground=self.BORDER,
            highlightthickness=1,
        )
        self.out_entry.grid(row=1, column=1, sticky="ew", padx=(0, 8), pady=4)
        self.out_entry.insert(0, str(self.output_dir.resolve()))

        btn_browse_out = tk.Button(
            file_card,
            text="Change Folder",
            command=self._browse_output_dir,
            bg=self.BG_INPUT,
            fg=self.TEXT_MAIN,
            font=("Segoe UI", 9),
            relief=tk.FLAT,
            cursor="hand2",
            padx=10,
            pady=4,
        )
        btn_browse_out.grid(row=1, column=2, padx=(0, 10), pady=4)

        # 3. Game Info Preview Card (Icon + Metadata)
        self.info_card = tk.Frame(main_frame, bg=self.BG_CARD, highlightbackground=self.BORDER, highlightthickness=1)
        self.info_card.pack(fill=tk.X, pady=(0, 10), ipadx=12, ipady=10)

        # Left: Icon container
        self.icon_frame = tk.Frame(self.info_card, bg=self.BG_INPUT, width=128, height=128)
        self.icon_frame.pack_propagate(False)
        self.icon_frame.pack(side=tk.LEFT, padx=12, pady=6)

        self.icon_label = tk.Label(self.icon_frame, text="No Game\nLoaded", bg=self.BG_INPUT, fg=self.TEXT_MUTED, font=("Segoe UI", 9))
        self.icon_label.pack(expand=True)

        # Right: Details
        details_frame = tk.Frame(self.info_card, bg=self.BG_CARD)
        details_frame.pack(side=tk.LEFT, fill=tk.BOTH, expand=True, padx=8)

        self.title_label = tk.Label(details_frame, text="Select a Switch game to inspect", font=("Segoe UI", 13, "bold"), bg=self.BG_CARD, fg=self.TEXT_MAIN)
        self.title_label.pack(anchor="w")

        meta_sub_frame = tk.Frame(details_frame, bg=self.BG_CARD)
        meta_sub_frame.pack(anchor="w", pady=(4, 0))

        self.format_badge = tk.Label(meta_sub_frame, text="FORMAT", bg=self.BG_INPUT, fg=self.ACCENT, font=("Segoe UI", 8, "bold"), padx=6, pady=1)
        self.format_badge.pack(side=tk.LEFT, padx=(0, 8))

        self.publisher_label = tk.Label(meta_sub_frame, text="", bg=self.BG_CARD, fg=self.TEXT_MUTED, font=("Segoe UI", 9))
        self.publisher_label.pack(side=tk.LEFT)

        self.id_version_label = tk.Label(details_frame, text="", bg=self.BG_CARD, fg=self.TEXT_MUTED, font=("Segoe UI", 9))
        self.id_version_label.pack(anchor="w", pady=(4, 0))

        self.contents_summary_label = tk.Label(details_frame, text="", bg=self.BG_CARD, fg=self.TEXT_MUTED, font=("Segoe UI", 9))
        self.contents_summary_label.pack(anchor="w", pady=(2, 0))

        # 4. Extraction Options Card
        options_card = tk.Frame(main_frame, bg=self.BG_CARD, highlightbackground=self.BORDER, highlightthickness=1)
        options_card.pack(fill=tk.X, pady=(0, 10), ipadx=12, ipady=8)

        opt_title = tk.Label(options_card, text="Extraction Targets:", font=("Segoe UI", 10, "bold"), bg=self.BG_CARD, fg=self.TEXT_MAIN)
        opt_title.pack(anchor="w", padx=10, pady=(2, 6))

        opt_cols = tk.Frame(options_card, bg=self.BG_CARD)
        opt_cols.pack(fill=tk.X, padx=10)

        # Variables
        self.var_ncas = tk.BooleanVar(value=True)
        self.var_decompress = tk.BooleanVar(value=True)
        self.var_romfs = tk.BooleanVar(value=True)
        self.var_exefs = tk.BooleanVar(value=True)
        self.var_meta = tk.BooleanVar(value=True)

        cb1 = ttk.Checkbutton(opt_cols, text="Extract NCAs & Partitions", variable=self.var_ncas)
        cb1.pack(side=tk.LEFT, padx=(0, 18))

        cb2 = ttk.Checkbutton(opt_cols, text="Decompress NSZ / XCZ to NCA", variable=self.var_decompress)
        cb2.pack(side=tk.LEFT, padx=(0, 18))

        cb3 = ttk.Checkbutton(opt_cols, text="RomFS Assets (Models, Audio, Textures)", variable=self.var_romfs)
        cb3.pack(side=tk.LEFT, padx=(0, 18))

        cb4 = ttk.Checkbutton(opt_cols, text="ExeFS Binaries (main, NPDM)", variable=self.var_exefs)
        cb4.pack(side=tk.LEFT, padx=(0, 18))

        cb5 = ttk.Checkbutton(opt_cols, text="Game Icon & Info JSON", variable=self.var_meta)
        cb5.pack(side=tk.LEFT)

        # 5. Progress Section
        progress_card = tk.Frame(main_frame, bg=self.BG_CARD, highlightbackground=self.BORDER, highlightthickness=1)
        progress_card.pack(fill=tk.X, pady=(0, 10), ipadx=12, ipady=10)

        prog_header = tk.Frame(progress_card, bg=self.BG_CARD)
        prog_header.pack(fill=tk.X, padx=10, pady=(0, 4))

        self.status_label = tk.Label(prog_header, text="Ready", font=("Segoe UI", 9, "bold"), bg=self.BG_CARD, fg=self.TEXT_MAIN)
        self.status_label.pack(side=tk.LEFT)

        self.percent_label = tk.Label(prog_header, text="0%", font=("Segoe UI", 9, "bold"), bg=self.BG_CARD, fg=self.ACCENT)
        self.percent_label.pack(side=tk.RIGHT)

        self.progress_bar = ttk.Progressbar(progress_card, orient=tk.HORIZONTAL, mode="determinate", style="Horizontal.TProgressbar")
        self.progress_bar.pack(fill=tk.X, padx=10, pady=(0, 8))

        # Action Buttons
        btn_box = tk.Frame(progress_card, bg=self.BG_CARD)
        btn_box.pack(fill=tk.X, padx=10)

        self.btn_extract = tk.Button(
            btn_box,
            text="🚀  Start Extraction",
            command=self._start_extraction,
            bg=self.SUCCESS,
            fg="#ffffff",
            activebackground="#059669",
            activeforeground="#ffffff",
            font=("Segoe UI", 10, "bold"),
            relief=tk.FLAT,
            cursor="hand2",
            padx=16,
            pady=6,
        )
        self.btn_extract.pack(side=tk.LEFT, padx=(0, 10))

        self.btn_cancel = tk.Button(
            btn_box,
            text="Cancel",
            command=self._cancel_extraction,
            bg=self.DANGER,
            fg="#ffffff",
            activebackground="#dc2626",
            activeforeground="#ffffff",
            font=("Segoe UI", 10),
            relief=tk.FLAT,
            cursor="hand2",
            padx=14,
            pady=6,
            state=tk.DISABLED,
        )
        self.btn_cancel.pack(side=tk.LEFT, padx=(0, 10))

        self.btn_open_folder = tk.Button(
            btn_box,
            text="📂  Open Output Folder",
            command=self._open_output_dir,
            bg=self.BG_INPUT,
            fg=self.TEXT_MAIN,
            font=("Segoe UI", 9),
            relief=tk.FLAT,
            cursor="hand2",
            padx=12,
            pady=6,
        )
        self.btn_open_folder.pack(side=tk.RIGHT)

        # 6. Live Log Output
        log_frame = tk.Frame(main_frame, bg=self.BG_CARD, highlightbackground=self.BORDER, highlightthickness=1)
        log_frame.pack(fill=tk.BOTH, expand=True)

        log_title_bar = tk.Frame(log_frame, bg=self.BG_CARD)
        log_title_bar.pack(fill=tk.X, padx=10, pady=(6, 2))

        l_lbl = tk.Label(log_title_bar, text="Console Log", font=("Segoe UI", 9, "bold"), bg=self.BG_CARD, fg=self.TEXT_MUTED)
        l_lbl.pack(side=tk.LEFT)

        btn_clear = tk.Button(
            log_title_bar,
            text="Clear Log",
            command=self._clear_log,
            bg=self.BG_CARD,
            fg=self.TEXT_MUTED,
            font=("Segoe UI", 8),
            relief=tk.FLAT,
            cursor="hand2",
        )
        btn_clear.pack(side=tk.RIGHT)

        self.log_text = tk.Text(
            log_frame,
            bg=self.BG_DARK,
            fg=self.TEXT_MAIN,
            font=("Consolas", 9),
            relief=tk.FLAT,
            wrap=tk.WORD,
            highlightbackground=self.BORDER,
            highlightthickness=1,
        )
        self.log_text.pack(fill=tk.BOTH, expand=True, padx=10, pady=(0, 10))

        # Tag colors in log
        self.log_text.tag_configure("info", foreground=self.TEXT_MAIN)
        self.log_text.tag_configure("success", foreground=self.SUCCESS)
        self.log_text.tag_configure("warn", foreground=self.WARNING)
        self.log_text.tag_configure("error", foreground=self.DANGER)
        self.log_text.tag_configure("accent", foreground=self.ACCENT)

    def log(self, text: str, tag: str = "info"):
        timestamp = time.strftime("[%H:%M:%S] ")
        self.log_text.insert(tk.END, timestamp, "accent")
        self.log_text.insert(tk.END, f"{text}\n", tag)
        self.log_text.see(tk.END)

    def _clear_log(self):
        self.log_text.delete("1.0", tk.END)

    def _update_key_status(self):
        if self.keyset.has_header_key:
            source = "Yuzu" if "yuzu" in str(self.keyset.prod_keys_file).lower() else "Ryujinx" if "ryujinx" in str(self.keyset.prod_keys_file).lower() else "Custom"
            key_count = len(self.keyset.keys)
            self.key_status_btn.config(
                text=f"✓ Keys Active: {source} ({key_count} keys)",
                bg=self.SUCCESS,
                fg="#ffffff",
            )
            self.log(f"Keys loaded: {self.keyset.prod_keys_file} ({key_count} keys)", "success")
        else:
            self.key_status_btn.config(
                text="⚠ No prod.keys Found (Click to Load)",
                bg=self.WARNING,
                fg="#000000",
            )
            self.log("prod.keys not found. You can unpack NCAs/containers, but RomFS/ExeFS requires keys.", "warn")

    def _browse_keys(self):
        file_selected = filedialog.askopenfilename(
            title="Select prod.keys or keys.txt",
            filetypes=[("Switch Keys (*.keys;*.txt;*.dat)", "*.keys;*.txt;*.dat"), ("All Files", "*.*")],
        )
        if file_selected:
            self.keyset = KeySet(prod_keys_path=file_selected)
            self.extractor = SwitchExtractor(keyset=self.keyset)
            self._update_key_status()
            # Re-inspect current file if loaded
            if self.file_entry.get():
                self._select_file_path(Path(self.file_entry.get()))

    def _browse_game_file(self):
        file_selected = filedialog.askopenfilename(
            title="Select Nintendo Switch Game File",
            filetypes=[
                ("Switch Games (*.xci;*.nsp;*.nsz;*.xcz;*.nca;*.pfs0;*.romfs)", "*.xci;*.nsp;*.nsz;*.xcz;*.nca;*.pfs0;*.romfs"),
                ("GameCard Images (*.xci;*.xcz)", "*.xci;*.xcz"),
                ("eShop Packages (*.nsp;*.nsz)", "*.nsp;*.nsz"),
                ("Content Archives (*.nca;*.ncz)", "*.nca;*.ncz"),
                ("All Files (*.*)", "*.*"),
            ],
        )
        if file_selected:
            self._select_file_path(Path(file_selected))

    def _select_file_path(self, path: Path):
        self.file_entry.delete(0, tk.END)
        self.file_entry.insert(0, str(path.resolve()))

        # Run inspect in background thread so UI doesn't hang
        self.status_label.config(text="Inspecting game file...")
        self.btn_extract.config(state=tk.DISABLED)

        threading.Thread(target=self._inspect_async, args=(path,), daemon=True).start()

    def _inspect_async(self, path: Path):
        try:
            info = self.extractor.inspect(path)
            self.current_info = info
            self.root.after(0, self._update_game_info_ui, info)
        except Exception as e:
            self.root.after(0, self.log, f"Error inspecting {path.name}: {e}", "error")
            self.root.after(0, lambda: self.status_label.config(text="Inspection failed."))

    def _update_game_info_ui(self, info: SwitchGameInfo):
        self.btn_extract.config(state=tk.NORMAL)
        self.status_label.config(text="Ready to extract.")

        self.title_label.config(text=info.title_name)
        self.format_badge.config(text=info.format.upper())
        self.publisher_label.config(text=f"by {info.publisher}")

        ver_str = f"Version: {info.display_version}" if info.display_version else ""
        id_str = f"Title ID: {info.title_id}" if info.title_id != "Unknown" else ""
        size_str = f"Size: {info.file_size / (1024**3):.2f} GB" if info.file_size >= 1024**3 else f"Size: {info.file_size / (1024**2):.2f} MB"
        self.id_version_label.config(text=f"{ver_str}   |   {id_str}   |   {size_str}")

        count_str = f"{len(info.files_list)} internal files/partitions"
        self.contents_summary_label.config(text=count_str)

        # Set suggested output path
        safe_name = "".join(c for c in info.title_name if c.isalnum() or c in (" ", "-", "_")).strip()
        safe_name = safe_name or info.file_path.stem
        suggested_dir = Path("./extracted") / f"{safe_name}_{info.title_id}"
        self.out_entry.delete(0, tk.END)
        self.out_entry.insert(0, str(suggested_dir.resolve()))

        # Display Icon if available
        if info.icon_bytes:
            try:
                img = Image.open(io.BytesIO(info.icon_bytes))
                img = img.resize((128, 128), Image.Resampling.LANCZOS)
                self.icon_photo = ImageTk.PhotoImage(img)
                self.icon_label.config(image=self.icon_photo, text="")
            except Exception:
                self.icon_label.config(image="", text="Icon\nError")
        else:
            self.icon_label.config(image="", text="No Icon\nFound")

        self.log(f"Loaded: {info.title_name} [{info.format}] ({size_str})", "success")

    def _browse_output_dir(self):
        dir_selected = filedialog.askdirectory(title="Select Output Directory")
        if dir_selected:
            self.out_entry.delete(0, tk.END)
            self.out_entry.insert(0, str(Path(dir_selected).resolve()))

    def _open_output_dir(self):
        target = Path(self.out_entry.get())
        if not target.exists():
            target.mkdir(parents=True, exist_ok=True)
        if sys.platform == "win32":
            os.startfile(target)
        else:
            subprocess.Popen(["xdg-open", str(target)])

    def _start_extraction(self):
        f_path_str = self.file_entry.get()
        if not f_path_str or not Path(f_path_str).is_file():
            messagebox.showerror("Error", "Please select a valid Nintendo Switch game file first.")
            return

        out_path = Path(self.out_entry.get())
        out_path.mkdir(parents=True, exist_ok=True)

        self.is_extracting = True
        self.cancel_requested = False
        self.btn_extract.config(state=tk.DISABLED)
        self.btn_cancel.config(state=tk.NORMAL)
        self.progress_bar["value"] = 0
        self.percent_label.config(text="0%")

        threading.Thread(
            target=self._extraction_thread,
            args=(Path(f_path_str), out_path),
            daemon=True,
        ).start()

    def _cancel_extraction(self):
        self.cancel_requested = True
        self.status_label.config(text="Cancelling extraction...")
        self.log("Cancel requested by user...", "warn")

    def _extraction_thread(self, game_file: Path, out_dir: Path):
        self.log(f"Starting extraction of: {game_file.name}", "accent")
        start_time = time.time()

        def prog_cb(step_name, idx, total, fname, done, tot):
            pct = int((done / tot) * 100) if tot > 0 else 0
            def ui_update():
                self.status_label.config(text=f"{step_name}: {fname} ({idx}/{total})")
                self.percent_label.config(text=f"{pct}%")
                self.progress_bar["value"] = pct
            self.root.after(0, ui_update)

        def cancel_check():
            return self.cancel_requested

        try:
            results = self.extractor.extract_all(
                file_path=game_file,
                out_dir=out_dir,
                extract_container_ncas=self.var_ncas.get(),
                extract_romfs_assets=self.var_romfs.get(),
                extract_exefs_binaries=self.var_exefs.get(),
                extract_metadata_icon=self.var_meta.get(),
                progress_callback=prog_cb,
                cancel_check=cancel_check,
            )

            if "romfs_error" in results:
                self.root.after(0, self.log, f"RomFS: {results['romfs_error']}", "warn")
            elif "romfs" in results:
                self.root.after(0, self.log, f"RomFS extracted: {len(results['romfs'])} files", "success")

            if "exefs_error" in results:
                self.root.after(0, self.log, f"ExeFS: {results['exefs_error']}", "warn")
            elif "exefs" in results:
                self.root.after(0, self.log, f"ExeFS extracted: {len(results['exefs'])} files", "success")

            elapsed = time.time() - start_time
            self.root.after(0, self._on_extraction_success, out_dir, elapsed)

        except InterruptedError:
            self.root.after(0, self._on_extraction_cancelled)
        except Exception as e:
            self.root.after(0, self._on_extraction_error, str(e))

    def _on_extraction_success(self, out_dir: Path, elapsed: float):
        self.is_extracting = False
        self.btn_extract.config(state=tk.NORMAL)
        self.btn_cancel.config(state=tk.DISABLED)
        self.progress_bar["value"] = 100
        self.percent_label.config(text="100%")
        self.status_label.config(text="Extraction completed!")
        self.log(f"Extraction completed successfully in {elapsed:.1f} seconds!", "success")
        self.log(f"Saved to: {out_dir.resolve()}", "success")
        messagebox.showinfo("Success", f"Extraction completed successfully!\n\nFolder: {out_dir.resolve()}")

    def _on_extraction_cancelled(self):
        self.is_extracting = False
        self.btn_extract.config(state=tk.NORMAL)
        self.btn_cancel.config(state=tk.DISABLED)
        self.status_label.config(text="Extraction cancelled.")
        self.log("Extraction was cancelled by user.", "warn")

    def _on_extraction_error(self, err_msg: str):
        self.is_extracting = False
        self.btn_extract.config(state=tk.NORMAL)
        self.btn_cancel.config(state=tk.DISABLED)
        self.status_label.config(text="Extraction error!")
        self.log(f"Error: {err_msg}", "error")
        messagebox.showerror("Extraction Error", f"An error occurred during extraction:\n\n{err_msg}")


def launch_gui(initial_file: Optional[str] = None):
    enable_high_dpi()
    root = tk.Tk()
    app = SwitchExtractorGUI(root, initial_file=initial_file)
    root.mainloop()


if __name__ == "__main__":
    launch_gui()

