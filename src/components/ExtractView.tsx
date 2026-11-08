import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  UploadCloud,
  FolderOpen,
  FolderSearch,
  CheckCircle2,
  AlertCircle,
  FileArchive,
  Layers,
  Cpu,
  Image as ImageIcon,
  Play,
  Square,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  X,
  Plus,
  Sparkles,
  Gift,
} from "lucide-react";
import { useAppStore, SwitchGameInfo } from "../store/useAppStore";

export const ExtractView: React.FC = () => {
  const {
    currentGame,
    setCurrentGame,
    isInspecting,
    setIsInspecting,
    extractNcas,
    setExtractNcas,
    extractRomfs,
    setExtractRomfs,
    extractExefs,
    setExtractExefs,
    extractMetadata,
    setExtractMetadata,
    outputDir,
    setOutputDir,
    baseGamePath,
    setBaseGamePath,
    updatePath,
    setUpdatePath,
    dlcPaths,
    addDlcPath,
    removeDlcPath,
    clearDlcPaths,
    isExtracting,
    setIsExtracting,
    progress,
    summary,
    setSummary,
    addLog,
    addToast,
    keyStatus,
  } = useAppStore();

  const [isFilesListOpen, setIsFilesListOpen] = useState(false);

  // --- Handlers for Base Game ---
  const handleSelectBaseGame = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Nintendo Switch Base Game (*.nsp, *.xci, *.nsz, *.xcz)",
            extensions: ["nsp", "xci", "nsz", "xcz"],
          },
        ],
      });
      if (selected && typeof selected === "string") {
        setBaseGamePath(selected);
        addToast("success", "Base Game selected.");
        addLog("info", `Base Game selected: ${selected}`);
      }
    } catch (err) {
      addToast("error", `File select error: ${err}`);
    }
  };

  const handleClearBaseGame = () => {
    setBaseGamePath(null);
    addToast("info", "Base Game selection cleared.");
  };

  // --- Handlers for Game Update ---
  const handleSelectUpdate = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Nintendo Switch Update (*.nsp, *.xci, *.nsz, *.xcz)",
            extensions: ["nsp", "xci", "nsz", "xcz"],
          },
        ],
      });
      if (selected && typeof selected === "string") {
        setUpdatePath(selected);
        addToast("success", "Game Update package selected.");
        addLog("info", `Game Update selected: ${selected}`);
      }
    } catch (err) {
      addToast("error", `Update picker error: ${err}`);
    }
  };

  const handleClearUpdate = () => {
    setUpdatePath(null);
    addToast("info", "Game Update selection cleared.");
  };

  const handleAutoDetectUpdate = async () => {
    const targetFile = currentGame?.file_path || baseGamePath;
    if (!targetFile) {
      addToast("warning", "Select a Base Game first to auto-detect updates.");
      return;
    }
    try {
      const detected = await invoke<string | null>("find_matching_update", { basePath: targetFile });
      if (detected) {
        setUpdatePath(detected);
        addToast("success", "Found matching Game Update!");
        addLog("success", `Auto-detected Update: ${detected}`);
      } else {
        addToast("info", "No matching Update found in the same or parent folder.");
      }
    } catch (err) {
      addToast("error", `Auto-detect error: ${err}`);
    }
  };

  // --- Handlers for DLCs ---
  const handleAddDlcs = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Nintendo Switch DLCs (*.nsp, *.xci, *.nsz, *.xcz, *.nca)",
            extensions: ["nsp", "xci", "nsz", "xcz", "nca"],
          },
        ],
      });
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        let addedCount = 0;
        for (const p of paths) {
          if (typeof p === "string") {
            addDlcPath(p);
            addedCount++;
          }
        }
        if (addedCount > 0) {
          addToast("success", `Added ${addedCount} DLC package(s).`);
          addLog("info", `Added ${addedCount} DLC(s) to extraction list.`);
        }
      }
    } catch (err) {
      addToast("error", `DLC picker error: ${err}`);
    }
  };

  const handleAutoDetectDlcs = async () => {
    const targetFile = currentGame?.file_path || baseGamePath;
    if (!targetFile) {
      addToast("warning", "Select a Base Game first to auto-detect DLCs.");
      return;
    }
    try {
      const detected = await invoke<string[]>("find_matching_dlcs", { basePath: targetFile });
      if (detected && detected.length > 0) {
        for (const p of detected) {
          addDlcPath(p);
        }
        addToast("success", `Found and linked ${detected.length} matching DLC package(s)!`);
        addLog("success", `Auto-detected ${detected.length} DLCs in directory.`);
      } else {
        addToast("info", "No matching DLCs found in the same or parent folder.");
      }
    } catch (err) {
      addToast("error", `DLC auto-detect error: ${err}`);
    }
  };

  // --- Handlers for Primary File ---
  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Nintendo Switch Package",
            extensions: ["nsp", "xci", "nsz", "xcz", "nca", "pfs0", "romfs"],
          },
        ],
      });

      if (selected && typeof selected === "string") {
        await inspectFile(selected);
      }
    } catch (err) {
      addToast("error", `File dialog error: ${err}`);
    }
  };

  const inspectFile = async (filePath: string) => {
    setIsInspecting(true);
    setSummary(null);
    addLog("info", `Inspecting package: ${filePath}`);

    try {
      const info = await invoke<SwitchGameInfo>("inspect_game", { path: filePath });
      setCurrentGame(info);
      addLog("success", `Loaded: ${info.title_name} [${info.format}] (${info.file_size_formatted})`);

      // Set default output directory if empty
      if (!outputDir) {
        const parentDir = filePath.substring(0, Math.max(filePath.lastIndexOf("/"), filePath.lastIndexOf("\\")));
        const safeTitle = info.title_name.replace(/[\\/:*?"<>|]/g, "_");
        const defaultOut = `${parentDir}\\Extracted_${safeTitle}`;
        setOutputDir(defaultOut);
      }

      // If the selected file is an Update package:
      if (info.is_update || info.requires_base_game) {
        try {
          const autoBase = await invoke<string | null>("find_matching_base_game", { updatePath: filePath });
          if (autoBase) {
            setBaseGamePath(autoBase);
            addToast("info", "Auto-detected matching Base Game in same directory!");
            addLog("info", `Auto-detected Base Game: ${autoBase}`);
          }
        } catch {
          // Ignore
        }
      } else {
        // If the selected file is a Base Game, auto-search for matching Update and DLCs!
        try {
          const autoUpd = await invoke<string | null>("find_matching_update", { basePath: filePath });
          if (autoUpd) {
            setUpdatePath(autoUpd);
            addToast("info", "Auto-detected matching Game Update in same directory!");
            addLog("info", `Auto-detected Update: ${autoUpd}`);
          }
        } catch {
          // Ignore
        }

        try {
          const autoDlcs = await invoke<string[]>("find_matching_dlcs", { basePath: filePath });
          if (autoDlcs && autoDlcs.length > 0) {
            for (const p of autoDlcs) {
              addDlcPath(p);
            }
            addToast("info", `Auto-detected ${autoDlcs.length} matching DLC package(s)!`);
            addLog("info", `Auto-detected ${autoDlcs.length} DLCs.`);
          }
        } catch {
          // Ignore
        }
      }
    } catch (err) {
      addToast("error", `Failed to inspect file: ${err}`);
      addLog("error", `Inspection error: ${err}`);
    } finally {
      setIsInspecting(false);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected && typeof selected === "string") {
        setOutputDir(selected);
      }
    } catch (err) {
      addToast("error", `Folder picker error: ${err}`);
    }
  };

  const handleOpenFolder = async () => {
    if (!outputDir) return;
    try {
      await invoke("open_folder", { path: outputDir });
    } catch (err) {
      addToast("error", `Could not open folder: ${err}`);
    }
  };

  const handleStartExtraction = async () => {
    if (!currentGame || !currentGame.file_path) {
      addToast("warning", "Please select a game package first.");
      return;
    }

    if (!outputDir) {
      addToast("warning", "Please specify an output folder.");
      return;
    }

    if (!extractNcas && !extractRomfs && !extractExefs && !extractMetadata) {
      addToast("warning", "Select at least one extraction target.");
      return;
    }

    setIsExtracting(true);
    setSummary(null);

    try {
      const res = await invoke("start_extraction", {
        req: {
          input_path: currentGame.file_path,
          output_dir: outputDir,
          base_game_path: baseGamePath || null,
          update_path: updatePath || null,
          dlc_paths: dlcPaths,
          extract_ncas: extractNcas,
          extract_romfs: extractRomfs,
          extract_exefs: extractExefs,
          extract_metadata: extractMetadata,
        },
      });
      setSummary(res as any);
      addToast("success", "Extraction completed successfully!");
    } catch (err) {
      addToast("error", `Extraction error: ${err}`);
    } finally {
      setIsExtracting(false);
    }
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_operation");
      addToast("warning", "Cancellation requested...");
      addLog("warn", "User requested extraction cancellation.");
    } catch (err) {
      addToast("error", `Failed to cancel: ${err}`);
    }
  };

  const getFileName = (pathStr: string) => {
    return pathStr.split(/[/\\]/).pop() || pathStr;
  };

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-4 gap-4">
      {/* ============================================================ */}
      {/* 1. PRIMARY / BASE GAME CARD */}
      {/* ============================================================ */}
      {!currentGame ? (
        <div
          onClick={handleSelectFile}
          className="border-2 border-dashed rounded-xl p-8 flex flex-col items-center justify-center gap-3 cursor-pointer transition-all hover:border-[#FF3B53]/60 hover:bg-[#FF3B53]/5 group"
          style={{
            borderColor: "var(--border-subtle)",
            backgroundColor: "var(--bg-card)",
          }}
        >
          <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-[#FF3B53]/15 text-[#FF3B53] group-hover:scale-110 transition-transform">
            <UploadCloud className="w-6 h-6" />
          </div>
          <div className="text-center">
            <div
              className="text-sm font-semibold mb-0.5 text-[#F0F4F8]"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Select Base Game Package
            </div>
            <div className="text-xs text-[#8B9BB4]">
              Drop Nintendo Switch Game (.XCI, .XCZ, .NSP, .NSZ, .NCA) or Click to Browse
            </div>
          </div>
          {isInspecting && (
            <div className="flex items-center gap-2 text-xs text-[#00E5FF] animate-pulse">
              <span className="w-2 h-2 rounded-full bg-[#00E5FF]" />
              Decrypting headers & reading metadata...
            </div>
          )}
        </div>
      ) : (
        /* Game Hero Card */
        <div
          className="rounded-xl p-4 border flex flex-col gap-4 shadow-lg transition-all"
          style={{
            backgroundColor: "var(--bg-card)",
            borderColor: "var(--border-subtle)",
          }}
        >
          <div className="flex items-start justify-between gap-4">
            <div className="flex items-center gap-4">
              {/* Game Icon */}
              {currentGame.icon_base64 ? (
                <img
                  src={currentGame.icon_base64}
                  alt={currentGame.title_name}
                  className="w-16 h-16 rounded-xl border object-cover shadow-md shrink-0"
                  style={{ borderColor: "var(--border-subtle)" }}
                />
              ) : (
                <div
                  className="w-16 h-16 rounded-xl border flex items-center justify-center bg-[#FF3B53]/10 text-[#FF3B53] shrink-0"
                  style={{ borderColor: "var(--border-subtle)" }}
                >
                  <FileArchive className="w-8 h-8" />
                </div>
              )}

              {/* Title & Metadata */}
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span
                    className="text-base font-bold text-[#F0F4F8] leading-tight"
                    style={{ fontFamily: "var(--font-display)" }}
                  >
                    {currentGame.title_name}
                  </span>
                  <span className="text-[10px] px-2 py-0.5 rounded-full font-bold bg-[#FF3B53]/20 text-[#FF3B53] border border-[#FF3B53]/30">
                    {currentGame.format}
                  </span>
                  {currentGame.is_update && (
                    <span className="text-[10px] px-2 py-0.5 rounded-full font-bold bg-[#00E5FF]/20 text-[#00E5FF] border border-[#00E5FF]/30">
                      Update Package
                    </span>
                  )}
                  {currentGame.is_compressed && (
                    <span className="text-[10px] px-2 py-0.5 rounded-full font-bold bg-[#10B981]/20 text-[#10B981] border border-[#10B981]/30">
                      NSZ Compressed
                    </span>
                  )}
                </div>

                <div className="flex items-center gap-3 text-xs text-[#8B9BB4] flex-wrap">
                  <span>Publisher: <strong className="text-[#F0F4F8]">{currentGame.publisher}</strong></span>
                  <span>•</span>
                  <span>Version: <strong className="text-[#F0F4F8]">{currentGame.display_version}</strong></span>
                  <span>•</span>
                  <span>Size: <strong className="text-[#F0F4F8]">{currentGame.file_size_formatted}</strong></span>
                </div>

                <div className="flex items-center gap-2 text-[11px] text-[#8B9BB4] mt-0.5 font-mono">
                  <span>Title ID:</span>
                  <span
                    className="px-1.5 py-0.5 rounded text-[10px] border"
                    style={{
                      backgroundColor: "var(--bg-input)",
                      borderColor: "var(--border-subtle)",
                      color: "var(--accent-cyan)",
                    }}
                  >
                    {currentGame.title_id}
                  </span>
                </div>
              </div>
            </div>

            <button
              onClick={handleSelectFile}
              className="text-xs px-3 py-1.5 rounded-lg border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#F0F4F8] shrink-0"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              Change Game
            </button>
          </div>

          {/* Internal Files Toggle */}
          <div className="border-t pt-2" style={{ borderColor: "var(--border-subtle)" }}>
            <button
              onClick={() => setIsFilesListOpen(!isFilesListOpen)}
              className="flex items-center gap-1.5 text-xs text-[#8B9BB4] hover:text-[#F0F4F8] transition-colors"
            >
              {isFilesListOpen ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
              <span>Inspect Internal Files ({currentGame.files_list.length} entries)</span>
            </button>

            {isFilesListOpen && (
              <div
                className="mt-2 max-h-36 overflow-y-auto rounded-lg p-2 font-mono text-[11px] space-y-1 border"
                style={{
                  backgroundColor: "var(--bg-input)",
                  borderColor: "var(--border-subtle)",
                }}
              >
                {currentGame.files_list.map((f, i) => (
                  <div key={i} className="text-[#8B9BB4] hover:text-[#F0F4F8]">
                    {f}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* ============================================================ */}
      {/* 2. BASE GAME MERGE CARD (Shown if primary game is an update) */}
      {/* ============================================================ */}
      {currentGame && (currentGame.is_update || currentGame.requires_base_game) && (
        <div
          className="rounded-xl p-4 border flex flex-col gap-3 bg-[#00E5FF]/5 border-[#00E5FF]/30 shadow-[0_0_15px_rgba(0,229,255,0.05)] transition-all"
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Layers className="w-4 h-4 text-[#00E5FF]" />
              <div
                className="text-xs font-bold uppercase tracking-wider text-[#F0F4F8]"
                style={{ fontFamily: "var(--font-display)" }}
              >
                Base Game for RomFS Merging
              </div>
              <span className="text-[10px] px-2 py-0.5 rounded-full font-bold bg-[#00E5FF]/20 text-[#00E5FF] border border-[#00E5FF]/40">
                Required for Update RomFS
              </span>
            </div>
          </div>

          <p className="text-xs text-[#8B9BB4] leading-relaxed">
            This update stores RomFS assets as BKTR deltas. To rebuild the complete updated RomFS, link the original Base Game (<code className="text-[#F0F4F8]">.nsp</code> or <code className="text-[#F0F4F8]">.xci</code>).
          </p>

          <div className="flex items-center gap-2">
            <div
              className="flex-1 flex items-center gap-2 px-3 py-2 rounded-lg border text-xs font-mono truncate"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: baseGamePath ? "#10B981" : "var(--border-subtle)",
                color: baseGamePath ? "#F0F4F8" : "#53637D",
              }}
            >
              <FileArchive className={`w-4 h-4 shrink-0 ${baseGamePath ? "text-[#10B981]" : "text-[#53637D]"}`} />
              <span className="truncate">
                {baseGamePath || "No Base Game package linked"}
              </span>
            </div>

            <button
              onClick={handleSelectBaseGame}
              className="px-3.5 py-2 rounded-lg text-xs font-bold bg-[#00E5FF] text-black hover:brightness-110 transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
            >
              <FolderSearch className="w-3.5 h-3.5" />
              {baseGamePath ? "Change Base Game" : "Browse Base Game..."}
            </button>

            {baseGamePath && (
              <button
                onClick={handleClearBaseGame}
                title="Clear Base Game"
                className="p-2 rounded-lg border hover:bg-red-500/10 hover:border-red-500/40 text-[#8B9BB4] hover:text-[#EF4444] transition-colors shrink-0"
                style={{ borderColor: "var(--border-subtle)" }}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        </div>
      )}

      {/* ============================================================ */}
      {/* 3. GAME UPDATE SLOT (If primary game is Base Game) */}
      {/* ============================================================ */}
      {currentGame && !currentGame.is_update && (
        <div
          className="rounded-xl p-4 border flex flex-col gap-3 transition-all"
          style={{
            backgroundColor: "var(--bg-card)",
            borderColor: updatePath ? "#00E5FF80" : "var(--border-subtle)",
          }}
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Layers className="w-4 h-4 text-[#00E5FF]" />
              <div
                className="text-xs font-bold uppercase tracking-wider text-[#F0F4F8]"
                style={{ fontFamily: "var(--font-display)" }}
              >
                Game Update (Optional)
              </div>
              {updatePath && (
                <span className="text-[10px] px-2 py-0.5 rounded-full font-bold bg-[#00E5FF]/20 text-[#00E5FF] border border-[#00E5FF]/40 flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  Update Linked
                </span>
              )}
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={handleAutoDetectUpdate}
                className="text-xs px-2.5 py-1 rounded-lg border hover:bg-white/5 transition-colors text-[#00E5FF] border-[#00E5FF]/40 flex items-center gap-1"
                title="Search folder for matching update"
              >
                <Sparkles className="w-3 h-3" />
                Auto-Detect Update
              </button>
            </div>
          </div>

          <p className="text-xs text-[#8B9BB4] leading-relaxed">
            If you have a game update package (<code className="text-[#F0F4F8]">.nsp</code> / <code className="text-[#F0F4F8]">.xci</code>), select it here. The engine will merge its BKTR deltas with the base game to output the complete, updated RomFS and latest binaries.
          </p>

          <div className="flex items-center gap-2">
            <div
              className="flex-1 flex items-center gap-2 px-3 py-2 rounded-lg border text-xs font-mono truncate"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: updatePath ? "#00E5FF" : "var(--border-subtle)",
                color: updatePath ? "#F0F4F8" : "#53637D",
              }}
            >
              <FileArchive className={`w-4 h-4 shrink-0 ${updatePath ? "text-[#00E5FF]" : "text-[#53637D]"}`} />
              <span className="truncate">
                {updatePath || "No Update package linked (Base Game will be extracted without patches)"}
              </span>
            </div>

            <button
              onClick={handleSelectUpdate}
              className="px-3.5 py-2 rounded-lg text-xs font-bold bg-[#00E5FF] text-black hover:brightness-110 transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
            >
              <FolderSearch className="w-3.5 h-3.5" />
              {updatePath ? "Change Update" : "Browse Update..."}
            </button>

            {updatePath && (
              <button
                onClick={handleClearUpdate}
                title="Remove Update"
                className="p-2 rounded-lg border hover:bg-red-500/10 hover:border-red-500/40 text-[#8B9BB4] hover:text-[#EF4444] transition-colors shrink-0"
                style={{ borderColor: "var(--border-subtle)" }}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        </div>
      )}

      {/* ============================================================ */}
      {/* 4. DLC CONTENT PACKAGES SLOT */}
      {/* ============================================================ */}
      {currentGame && (
        <div
          className="rounded-xl p-4 border flex flex-col gap-3 transition-all"
          style={{
            backgroundColor: "var(--bg-card)",
            borderColor: dlcPaths.length > 0 ? "#A855F780" : "var(--border-subtle)",
          }}
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Gift className="w-4 h-4 text-[#A855F7]" />
              <div
                className="text-xs font-bold uppercase tracking-wider text-[#F0F4F8]"
                style={{ fontFamily: "var(--font-display)" }}
              >
                DLC Content Packages (Optional)
              </div>
              <span className={`text-[10px] px-2 py-0.5 rounded-full font-bold border ${
                dlcPaths.length > 0
                  ? "bg-[#A855F7]/20 text-[#A855F7] border-[#A855F7]/40"
                  : "bg-white/5 text-[#8B9BB4] border-[var(--border-subtle)]"
              }`}>
                {dlcPaths.length} Selected
              </span>
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={handleAutoDetectDlcs}
                className="text-xs px-2.5 py-1 rounded-lg border hover:bg-white/5 transition-colors text-[#A855F7] border-[#A855F7]/40 flex items-center gap-1"
                title="Search folder for matching DLC packages"
              >
                <Sparkles className="w-3 h-3" />
                Auto-Detect DLCs
              </button>

              <button
                onClick={handleAddDlcs}
                className="text-xs px-3 py-1 rounded-lg font-bold bg-[#A855F7] text-white hover:brightness-110 transition-all flex items-center gap-1 cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                Add DLC(s)...
              </button>

              {dlcPaths.length > 0 && (
                <button
                  onClick={clearDlcPaths}
                  className="text-xs px-2.5 py-1 rounded-lg border hover:bg-red-500/10 hover:border-red-500/40 text-[#EF4444] border-red-500/30 transition-colors"
                >
                  Clear All
                </button>
              )}
            </div>
          </div>

          <p className="text-xs text-[#8B9BB4] leading-relaxed">
            Add any DLC expansion packages (<code className="text-[#F0F4F8]">.nsp</code>, <code className="text-[#F0F4F8]">.xci</code>, <code className="text-[#F0F4F8]">.nca</code>). Each DLC will be extracted cleanly into its own dedicated subfolder under <code className="text-[#F0F4F8]">/dlc/</code>.
          </p>

          {/* List of Selected DLCs */}
          {dlcPaths.length > 0 ? (
            <div
              className="max-h-36 overflow-y-auto rounded-lg border divide-y"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
              }}
            >
              {dlcPaths.map((p, idx) => (
                <div key={idx} className="flex items-center justify-between p-2 text-xs font-mono">
                  <div className="flex items-center gap-2 truncate mr-2">
                    <span className="w-5 h-5 rounded flex items-center justify-center bg-[#A855F7]/20 text-[#A855F7] text-[10px] font-bold shrink-0">
                      {idx + 1}
                    </span>
                    <span className="text-[#F0F4F8] truncate">{getFileName(p)}</span>
                  </div>
                  <button
                    onClick={() => removeDlcPath(idx)}
                    title="Remove this DLC"
                    className="p-1 rounded text-[#8B9BB4] hover:text-[#EF4444] transition-colors shrink-0"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div
              className="p-3 rounded-lg border border-dashed flex items-center justify-center text-xs text-[#53637D]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              No DLC packages added yet. Click &ldquo;Add DLC(s)...&rdquo; or &ldquo;Auto-Detect DLCs&rdquo; above.
            </div>
          )}
        </div>
      )}

      {/* ============================================================ */}
      {/* 5. EXTRACTION TARGETS & CHECKBOXES */}
      {/* ============================================================ */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-3"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div
          className="text-xs font-bold uppercase tracking-wider text-[#8B9BB4]"
          style={{ fontFamily: "var(--font-display)" }}
        >
          Extraction Targets
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {/* RomFS Assets */}
          <label
            className={`flex flex-col gap-1 p-3 rounded-lg border cursor-pointer transition-all ${
              extractRomfs
                ? "bg-[#FF3B53]/10 border-[#FF3B53]/40 text-[#F0F4F8]"
                : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Layers className={`w-4 h-4 ${extractRomfs ? "text-[#FF3B53]" : ""}`} />
                <span className="text-xs font-bold">RomFS Game Assets</span>
              </div>
              <input
                type="checkbox"
                checked={extractRomfs}
                onChange={(e) => setExtractRomfs(e.target.checked)}
                className="accent-[#FF3B53]"
              />
            </div>
            <span className="text-[10px] text-[#8B9BB4]">
              3D models, textures, BGM audio, game scripts (merges base + update deltas &amp; DLCs)
            </span>
          </label>

          {/* ExeFS Binaries */}
          <label
            className={`flex flex-col gap-1 p-3 rounded-lg border cursor-pointer transition-all ${
              extractExefs
                ? "bg-[#FF3B53]/10 border-[#FF3B53]/40 text-[#F0F4F8]"
                : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Cpu className={`w-4 h-4 ${extractExefs ? "text-[#FF3B53]" : ""}`} />
                <span className="text-xs font-bold">ExeFS Binaries</span>
              </div>
              <input
                type="checkbox"
                checked={extractExefs}
                onChange={(e) => setExtractExefs(e.target.checked)}
                className="accent-[#FF3B53]"
              />
            </div>
            <span className="text-[10px] text-[#8B9BB4]">
              main, main.npdm, rtld, and subsdk* executables (latest from update or base)
            </span>
          </label>

          {/* Loose NCAs */}
          <label
            className={`flex flex-col gap-1 p-3 rounded-lg border cursor-pointer transition-all ${
              extractNcas
                ? "bg-[#FF3B53]/10 border-[#FF3B53]/40 text-[#F0F4F8]"
                : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <FileArchive className={`w-4 h-4 ${extractNcas ? "text-[#FF3B53]" : ""}`} />
                <span className="text-xs font-bold">Container NCAs</span>
              </div>
              <input
                type="checkbox"
                checked={extractNcas}
                onChange={(e) => setExtractNcas(e.target.checked)}
                className="accent-[#FF3B53]"
              />
            </div>
            <span className="text-[10px] text-[#8B9BB4]">
              Unpacked .nca files for Base, Update, and DLCs
            </span>
          </label>

          {/* Metadata & Icon */}
          <label
            className={`flex flex-col gap-1 p-3 rounded-lg border cursor-pointer transition-all ${
              extractMetadata
                ? "bg-[#FF3B53]/10 border-[#FF3B53]/40 text-[#F0F4F8]"
                : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <ImageIcon className={`w-4 h-4 ${extractMetadata ? "text-[#FF3B53]" : ""}`} />
                <span className="text-xs font-bold">Metadata &amp; Icon</span>
              </div>
              <input
                type="checkbox"
                checked={extractMetadata}
                onChange={(e) => setExtractMetadata(e.target.checked)}
                className="accent-[#FF3B53]"
              />
            </div>
            <span className="text-[10px] text-[#8B9BB4]">
              High-res icon.jpg and game_info.json
            </span>
          </label>
        </div>

        {/* Warning if keys missing */}
        {keyStatus && !keyStatus.has_header_key && (extractRomfs || extractExefs) && (
          <div className="flex items-center gap-2 text-xs p-2.5 rounded-lg bg-[#F59E0B]/15 text-[#F59E0B] border border-[#F59E0B]/30">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>
              <strong>prod.keys missing:</strong> Container NCAs can still be extracted, but RomFS and ExeFS decryption requires keys. Visit the <em>Keys Manager</em> tab to configure your keys.
            </span>
          </div>
        )}
      </div>

      {/* ============================================================ */}
      {/* 6. DESTINATION FOLDER & ACTION BUTTON */}
      {/* ============================================================ */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-3"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div
          className="text-xs font-bold uppercase tracking-wider text-[#8B9BB4]"
          style={{ fontFamily: "var(--font-display)" }}
        >
          Destination Folder
        </div>

        <div className="flex items-center gap-2">
          <input
            type="text"
            value={outputDir}
            onChange={(e) => setOutputDir(e.target.value)}
            placeholder="Select output folder..."
            className="flex-1 px-3 py-2 rounded-lg text-xs font-mono outline-none border focus:border-[#FF3B53] transition-colors"
            style={{
              backgroundColor: "var(--bg-input)",
              borderColor: "var(--border-subtle)",
              color: "var(--text-main)",
            }}
          />
          <button
            onClick={handleSelectOutputDir}
            className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
            style={{ borderColor: "var(--border-subtle)" }}
          >
            <FolderSearch className="w-3.5 h-3.5 text-[#00E5FF]" />
            Browse
          </button>
          {outputDir && (
            <button
              onClick={handleOpenFolder}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <ExternalLink className="w-3.5 h-3.5" />
              Open
            </button>
          )}
        </div>

        {/* Action Button & Live Progress */}
        <div className="flex items-center justify-between gap-4 mt-2">
          {!isExtracting ? (
            <button
              onClick={handleStartExtraction}
              disabled={!currentGame}
              className="w-full flex items-center justify-center gap-2 py-3 px-6 rounded-lg font-bold text-sm text-white shadow-lg shadow-[#FF3B53]/20 hover:brightness-110 active:scale-[0.99] transition-all disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
              style={{
                backgroundColor: "var(--accent-coral)",
                fontFamily: "var(--font-display)",
              }}
            >
              <Play className="w-4 h-4 fill-white" />
              {updatePath || dlcPaths.length > 0
                ? `Start Merged Extraction (${[
                    "Base",
                    updatePath ? "Update" : null,
                    dlcPaths.length > 0 ? `${dlcPaths.length} DLCs` : null,
                  ]
                    .filter(Boolean)
                    .join(" + ")})`
                : "Start Full Extraction"}
            </button>
          ) : (
            <div className="w-full flex items-center gap-3">
              <div className="flex-1 flex flex-col gap-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold text-[#00E5FF]">
                    {progress?.stage || "Processing..."}
                  </span>
                  <span className="font-mono text-[#F0F4F8]">
                    {progress?.percent.toFixed(1)}%
                  </span>
                </div>
                <div
                  className="w-full h-2.5 rounded-full overflow-hidden border"
                  style={{
                    backgroundColor: "var(--bg-input)",
                    borderColor: "var(--border-subtle)",
                  }}
                >
                  <div
                    className="h-full bg-gradient-to-r from-[#FF3B53] to-[#00E5FF] transition-all duration-150"
                    style={{ width: `${progress?.percent || 0}%` }}
                  />
                </div>
                <div className="flex items-center justify-between text-[11px] font-mono text-[#8B9BB4]">
                  <span className="truncate max-w-[280px]">
                    {progress?.file_name || ""}
                  </span>
                  <span>
                    {progress ? `${progress.current_file} / ${progress.total_files}` : ""}
                  </span>
                </div>
              </div>

              <button
                onClick={handleCancel}
                className="px-4 py-2.5 rounded-lg text-xs font-bold text-[#EF4444] bg-[#EF4444]/15 border border-[#EF4444]/30 hover:bg-[#EF4444]/25 transition-all flex items-center gap-1.5"
              >
                <Square className="w-3.5 h-3.5 fill-[#EF4444]" />
                Cancel
              </button>
            </div>
          )}
        </div>
      </div>

      {/* ============================================================ */}
      {/* 7. SUMMARY VIEW ON SUCCESS */}
      {/* ============================================================ */}
      {summary && (
        <div
          className="rounded-xl p-4 border flex flex-col gap-3 bg-[#10B981]/10 border-[#10B981]/40"
        >
          <div className="flex items-center gap-2 text-sm font-bold text-[#10B981]">
            <CheckCircle2 className="w-4.5 h-4.5" />
            Extraction Completed Successfully!
          </div>

          <div className="text-xs text-[#F0F4F8] flex items-center gap-4 flex-wrap">
            {summary.ncas.length > 0 && <span><strong>{summary.ncas.length}</strong> NCAs</span>}
            {summary.romfs.length > 0 && <span><strong>{summary.romfs.length}</strong> RomFS Assets</span>}
            {summary.exefs.length > 0 && <span><strong>{summary.exefs.length}</strong> ExeFS Binaries</span>}
            {summary.dlcs && summary.dlcs.length > 0 && (
              <span className="px-2 py-0.5 rounded-full font-bold bg-[#A855F7]/20 text-[#A855F7] border border-[#A855F7]/30">
                <strong>{summary.dlcs.length}</strong> DLC(s) Extracted
              </span>
            )}
          </div>

          {/* List of Extracted DLCs in Summary */}
          {summary.dlcs && summary.dlcs.length > 0 && (
            <div className="flex flex-col gap-1.5 p-2 rounded-lg bg-black/20 border border-white/5">
              <div className="text-[11px] font-bold text-[#A855F7]">Extracted DLC Content:</div>
              <div className="space-y-1">
                {summary.dlcs.map((dlc, i) => (
                  <div key={i} className="flex items-center justify-between text-xs font-mono">
                    <span className="text-[#F0F4F8] truncate mr-2">{dlc.name}</span>
                    <span className="text-[#8B9BB4] text-[10px] shrink-0">
                      {dlc.romfs_count} files ({dlc.nca_count} NCAs)
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {summary.romfs_error && (
            <div className="flex flex-col gap-2 p-3 rounded-lg bg-[#F59E0B]/15 border border-[#F59E0B]/30 text-xs text-[#F59E0B]">
              <div className="flex items-center gap-2 font-bold">
                <AlertCircle className="w-4 h-4 shrink-0" />
                RomFS Note: {summary.romfs_error}
              </div>
              {(summary.romfs_error.includes("Base Game") || summary.romfs_error.includes("BKTR")) && (
                <div className="flex items-center gap-2 pt-1">
                  <button
                    onClick={async () => {
                      await handleSelectBaseGame();
                    }}
                    className="px-3 py-1.5 rounded bg-[#F59E0B] text-black font-bold hover:brightness-110 transition-all flex items-center gap-1.5 cursor-pointer text-xs"
                  >
                    <FolderSearch className="w-3.5 h-3.5" />
                    Select Base Game Package &amp; Re-Extract
                  </button>
                </div>
              )}
            </div>
          )}

          {summary.exefs_error && (
            <div className="text-xs text-[#F59E0B]">
              ExeFS Note: {summary.exefs_error}
            </div>
          )}

          <div className="pt-2">
            <button
              onClick={handleOpenFolder}
              className="text-xs px-3 py-1.5 rounded bg-[#10B981] text-black font-bold hover:brightness-110 transition-all flex items-center gap-1.5 cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5" />
              View Extracted Files
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
