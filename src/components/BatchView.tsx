import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Layers,
  Plus,
  Trash2,
  Play,
  Square,
  CheckCircle2,
  AlertCircle,
  Clock,
  Loader2,
  FolderSearch,
} from "lucide-react";
import { useAppStore } from "../store/useAppStore";

export const BatchView: React.FC = () => {
  const {
    batchQueue,
    addToBatch,
    removeFromBatch,
    clearBatch,
    isBatchRunning,
    setIsBatchRunning,
    updateBatchItem,
    outputDir,
    setOutputDir,
    extractNcas,
    extractRomfs,
    extractExefs,
    extractMetadata,
    addToast,
    addLog,
  } = useAppStore();

  const handleAddFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Switch Games",
            extensions: ["nsp", "xci", "nsz", "xcz", "nca", "pfs0"],
          },
        ],
      });

      if (selected && Array.isArray(selected)) {
        addToBatch(selected);
        addToast("info", `Added ${selected.length} files to queue.`);
      }
    } catch (err) {
      addToast("error", `File picker error: ${err}`);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        setOutputDir(selected);
      }
    } catch (err) {
      addToast("error", `Folder picker error: ${err}`);
    }
  };

  const handleStartBatch = async () => {
    if (batchQueue.length === 0) {
      addToast("warning", "Queue is empty. Add games first.");
      return;
    }
    if (!outputDir) {
      addToast("warning", "Specify a root output directory.");
      return;
    }

    setIsBatchRunning(true);
    addLog("info", `Starting batch extraction of ${batchQueue.length} files.`);

    for (const item of batchQueue) {
      if (item.status === "completed") continue;

      updateBatchItem(item.id, { status: "extracting" });

      const safeName = item.fileName.replace(/\.[^.]+$/, "").replace(/[\\/:*?"<>|]/g, "_");
      const targetDir = `${outputDir}\\${safeName}`;

      let autoBase: string | null = null;
      try {
        autoBase = await invoke<string | null>("find_matching_base_game", { updatePath: item.path });
      } catch {
        // ignore
      }

      try {
        await invoke("start_extraction", {
          req: {
            input_path: item.path,
            output_dir: targetDir,
            base_game_path: autoBase,
            update_path: null,
            dlc_paths: [],
            extract_ncas: extractNcas,
            extract_romfs: extractRomfs,
            extract_exefs: extractExefs,
            extract_metadata: extractMetadata,
          },
        });
        updateBatchItem(item.id, { status: "completed" });
      } catch (err) {
        updateBatchItem(item.id, { status: "failed", error: String(err) });
      }
    }

    setIsBatchRunning(false);
    addToast("success", "Batch queue finished!");
  };

  const handleCancelBatch = async () => {
    try {
      await invoke("cancel_operation");
      setIsBatchRunning(false);
      addToast("warning", "Batch extraction stopped.");
    } catch (err) {
      addToast("error", `Cancel error: ${err}`);
    }
  };

  return (
    <div className="flex-1 flex flex-col h-full overflow-y-auto p-4 gap-4">
      {/* Header Bar */}
      <div
        className="rounded-xl p-4 border flex items-center justify-between"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-[#FF3B53]/15 text-[#FF3B53]">
            <Layers className="w-5 h-5" />
          </div>
          <div>
            <div
              className="text-sm font-bold"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Batch Game Unpacker
            </div>
            <div className="text-xs text-[#8B9BB4]">
              Queue multiple Nintendo Switch titles for hands-off sequential extraction.
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleAddFiles}
            disabled={isBatchRunning}
            className="px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 bg-[#FF3B53] text-white hover:brightness-110 transition-all disabled:opacity-40 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Add Games
          </button>
          <button
            onClick={clearBatch}
            disabled={isBatchRunning || batchQueue.length === 0}
            className="px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#EF4444] disabled:opacity-40"
            style={{ borderColor: "var(--border-subtle)" }}
          >
            <Trash2 className="w-3.5 h-3.5" />
            Clear
          </button>
        </div>
      </div>

      {/* Destination Folder */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-2"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <label className="text-xs font-bold uppercase text-[#8B9BB4]">
          Batch Output Folder
        </label>
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={outputDir}
            onChange={(e) => setOutputDir(e.target.value)}
            placeholder="Select directory where each game will receive its own folder..."
            className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
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
        </div>
      </div>

      {/* Queue Table */}
      <div
        className="flex-1 rounded-xl border flex flex-col overflow-hidden"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div
          className="flex items-center justify-between px-4 py-2.5 border-b text-xs font-semibold text-[#8B9BB4]"
          style={{ borderColor: "var(--border-subtle)" }}
        >
          <span>Package Name & Path</span>
          <span>Status</span>
        </div>

        <div className="flex-1 overflow-y-auto divide-y" style={{ borderColor: "var(--border-subtle)" }}>
          {batchQueue.length === 0 ? (
            <div className="p-8 text-center text-xs text-[#53637D] font-mono">
              Queue is empty. Click "+ Add Games" to load multiple files.
            </div>
          ) : (
            batchQueue.map((item) => (
              <div
                key={item.id}
                className="flex items-center justify-between px-4 py-3 hover:bg-white/[0.02] transition-colors"
              >
                <div className="flex flex-col gap-0.5 truncate max-w-[70%]">
                  <span className="text-xs font-medium text-[#F0F4F8] truncate">
                    {item.fileName}
                  </span>
                  <span className="text-[10px] font-mono text-[#8B9BB4] truncate">
                    {item.path}
                  </span>
                </div>

                <div className="flex items-center gap-3">
                  {item.status === "queued" && (
                    <span className="flex items-center gap-1 text-[11px] font-mono text-[#8B9BB4]">
                      <Clock className="w-3 h-3" /> Queued
                    </span>
                  )}
                  {item.status === "extracting" && (
                    <span className="flex items-center gap-1 text-[11px] font-mono text-[#00E5FF] animate-pulse">
                      <Loader2 className="w-3 h-3 animate-spin" /> Extracting...
                    </span>
                  )}
                  {item.status === "completed" && (
                    <span className="flex items-center gap-1 text-[11px] font-mono text-[#10B981]">
                      <CheckCircle2 className="w-3 h-3" /> Done
                    </span>
                  )}
                  {item.status === "failed" && (
                    <span
                      title={item.error}
                      className="flex items-center gap-1 text-[11px] font-mono text-[#EF4444]"
                    >
                      <AlertCircle className="w-3 h-3" /> Failed
                    </span>
                  )}

                  {!isBatchRunning && (
                    <button
                      onClick={() => removeFromBatch(item.id)}
                      className="text-[#8B9BB4] hover:text-[#EF4444] p-1 transition-colors"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  )}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer Execution Bar */}
        <div
          className="p-3 border-t flex items-center justify-between"
          style={{
            borderColor: "var(--border-subtle)",
            backgroundColor: "var(--bg-input)",
          }}
        >
          <div className="text-xs text-[#8B9BB4] font-mono">
            {batchQueue.filter((i) => i.status === "completed").length} / {batchQueue.length} completed
          </div>

          {!isBatchRunning ? (
            <button
              onClick={handleStartBatch}
              disabled={batchQueue.length === 0}
              className="px-5 py-2 rounded-lg text-xs font-bold text-white bg-[#FF3B53] shadow-md hover:brightness-110 active:scale-[0.99] transition-all disabled:opacity-40 cursor-pointer flex items-center gap-2"
              style={{ fontFamily: "var(--font-display)" }}
            >
              <Play className="w-3.5 h-3.5 fill-white" />
              Start Batch Processing
            </button>
          ) : (
            <button
              onClick={handleCancelBatch}
              className="px-4 py-2 rounded-lg text-xs font-bold text-[#EF4444] bg-[#EF4444]/15 border border-[#EF4444]/30 hover:bg-[#EF4444]/25 transition-all flex items-center gap-1.5"
            >
              <Square className="w-3.5 h-3.5 fill-[#EF4444]" />
              Stop Batch
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

