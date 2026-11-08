import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Repeat,
  FolderSearch,
  FolderOpen,
  CheckCircle2,
  Play,
  Square,
  Zap,
  HardDrive,
} from "lucide-react";
import { useAppStore } from "../store/useAppStore";

export const ConvertView: React.FC = () => {
  const {
    convertInputPath,
    setConvertInputPath,
    convertOutputPath,
    setConvertOutputPath,
    isConverting,
    setIsConverting,
    convertProgress,
    addLog,
    addToast,
  } = useAppStore();

  const [convertedBytes, setConvertedBytes] = useState<number | null>(null);

  const handleSelectXci = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "GameCard Dump",
            extensions: ["xci", "xcz"],
          },
        ],
      });

      if (selected && typeof selected === "string") {
        setConvertInputPath(selected);
        setConvertedBytes(null);

        // Derive default output NSP path
        const nspPath = selected.replace(/\.(xci|xcz)$/i, ".nsp");
        setConvertOutputPath(nspPath);
        addLog("info", `Selected XCI for conversion: ${selected}`);
      }
    } catch (err) {
      addToast("error", `File picker error: ${err}`);
    }
  };

  const handleSelectOutputNsp = async () => {
    try {
      const defaultName = convertInputPath
        ? convertInputPath.split(/[/\\]/).pop()?.replace(/\.(xci|xcz)$/i, ".nsp")
        : "game.nsp";

      const selected = await save({
        defaultPath: defaultName,
        filters: [
          {
            name: "eShop NSP Package",
            extensions: ["nsp"],
          },
        ],
      });

      if (selected && typeof selected === "string") {
        setConvertOutputPath(selected);
      }
    } catch (err) {
      addToast("error", `Save dialog error: ${err}`);
    }
  };

  const handleConvert = async () => {
    if (!convertInputPath) {
      addToast("warning", "Please select an XCI file to convert.");
      return;
    }
    if (!convertOutputPath) {
      addToast("warning", "Please specify output NSP path.");
      return;
    }

    setIsConverting(true);
    setConvertedBytes(null);

    try {
      const written = await invoke<number>("convert_xci_to_nsp", {
        req: {
          input_path: convertInputPath,
          output_nsp_path: convertOutputPath,
        },
      });
      setConvertedBytes(written);
      addToast("success", "XCI successfully converted to installable NSP!");
    } catch (err) {
      addToast("error", `Conversion error: ${err}`);
    } finally {
      setIsConverting(false);
    }
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_operation");
      addToast("warning", "Cancellation requested...");
    } catch (err) {
      addToast("error", `Failed to cancel: ${err}`);
    }
  };

  const handleOpenFolder = async () => {
    if (!convertOutputPath) return;
    try {
      await invoke("open_folder", { path: convertOutputPath });
    } catch (err) {
      addToast("error", `Could not open folder: ${err}`);
    }
  };

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-4 gap-4">
      {/* Header Banner */}
      <div
        className="rounded-xl p-4 border flex items-center justify-between"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-[#00E5FF]/15 text-[#00E5FF]">
            <Repeat className="w-5 h-5" />
          </div>
          <div>
            <div
              className="text-sm font-bold"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Direct XCI ➔ NSP Repacker
            </div>
            <div className="text-xs text-[#8B9BB4]">
              Repacks GameCard dumps into standard eShop-installable .nsp packages on-the-fly.
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-mono bg-[#00E5FF]/10 text-[#00E5FF] border border-[#00E5FF]/30">
          <Zap className="w-3.5 h-3.5" />
          Zero Disk Buffering
        </div>
      </div>

      {/* Input / Output Form */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-4"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        {/* Source XCI */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold uppercase text-[#8B9BB4]">
            Source GameCard Dump (.xci / .xcz)
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={convertInputPath}
              readOnly
              placeholder="Select an .xci file to convert..."
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />
            <button
              onClick={handleSelectXci}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <FolderSearch className="w-3.5 h-3.5 text-[#00E5FF]" />
              Select XCI
            </button>
          </div>
        </div>

        {/* Output NSP */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold uppercase text-[#8B9BB4]">
            Output NSP Package Destination
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={convertOutputPath}
              onChange={(e) => setConvertOutputPath(e.target.value)}
              placeholder="Target .nsp file path..."
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />
            <button
              onClick={handleSelectOutputNsp}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <HardDrive className="w-3.5 h-3.5 text-[#FF3B53]" />
              Set Path
            </button>
          </div>
        </div>

        {/* Action Button & Live Progress */}
        <div className="pt-2">
          {!isConverting ? (
            <button
              onClick={handleConvert}
              disabled={!convertInputPath || !convertOutputPath}
              className="w-full flex items-center justify-center gap-2 py-3 px-6 rounded-lg font-bold text-sm text-white shadow-lg shadow-[#FF3B53]/20 hover:brightness-110 active:scale-[0.99] transition-all disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
              style={{
                backgroundColor: "var(--accent-coral)",
                fontFamily: "var(--font-display)",
              }}
            >
              <Play className="w-4 h-4 fill-white" />
              Convert to NSP Now
            </button>
          ) : (
            <div className="flex items-center gap-3">
              <div className="flex-1 flex flex-col gap-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold text-[#00E5FF]">
                    {convertProgress?.stage || "Repacking partitions..."}
                  </span>
                  <span className="font-mono text-[#F0F4F8]">
                    {convertProgress?.percent.toFixed(1)}%
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
                    className="h-full bg-gradient-to-r from-[#00E5FF] to-[#FF3B53] transition-all duration-150"
                    style={{ width: `${convertProgress?.percent || 0}%` }}
                  />
                </div>
                <div className="flex items-center justify-between text-[11px] font-mono text-[#8B9BB4]">
                  <span className="truncate max-w-[280px]">
                    {convertProgress?.file_name || ""}
                  </span>
                  <span>
                    {convertProgress
                      ? `${convertProgress.current_file} / ${convertProgress.total_files}`
                      : ""}
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

      {/* Success Banner */}
      {convertedBytes !== null && (
        <div className="rounded-xl p-4 border flex items-center justify-between bg-[#10B981]/10 border-[#10B981]/40">
          <div className="flex items-center gap-2 text-sm font-bold text-[#10B981]">
            <CheckCircle2 className="w-4.5 h-4.5" />
            Conversion Complete! ({(convertedBytes / (1024 * 1024 * 1024)).toFixed(2)} GB written)
          </div>
          <button
            onClick={handleOpenFolder}
            className="text-xs px-3 py-1.5 rounded bg-[#10B981] text-black font-bold hover:brightness-110 transition-all flex items-center gap-1.5 cursor-pointer"
          >
            <FolderOpen className="w-3.5 h-3.5" />
            Show in Explorer
          </button>
        </div>
      )}
    </div>
  );
};

