import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  KeyRound,
  AlertTriangle,
  FolderSearch,
  RotateCcw,
  ShieldCheck,
  Folder,
} from "lucide-react";
import { useAppStore, KeyStatus } from "../store/useAppStore";

export const KeysView: React.FC = () => {
  const { keyStatus, setKeyStatus, addToast, addLog } = useAppStore();
  const [customProd, setCustomProd] = useState<string>(keyStatus?.prod_keys_path || "");
  const [customTitle, setCustomTitle] = useState<string>(keyStatus?.title_keys_path || "");

  const handleSelectProdKeys = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Switch Keys", extensions: ["keys", "txt"] }],
      });
      if (selected && typeof selected === "string") {
        setCustomProd(selected);
      }
    } catch (err) {
      addToast("error", `File picker error: ${err}`);
    }
  };

  const handleSelectTitleKeys = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Title Keys", extensions: ["keys", "txt"] }],
      });
      if (selected && typeof selected === "string") {
        setCustomTitle(selected);
      }
    } catch (err) {
      addToast("error", `File picker error: ${err}`);
    }
  };

  const handleApplyKeys = async () => {
    try {
      await invoke("save_settings", {
        settings: {
          custom_prod_keys: customProd || null,
          custom_title_keys: customTitle || null,
          theme: "dark",
          auto_decompress_ncz: true,
        },
      });

      // Reload keys
      const status = await invoke<KeyStatus>("detect_keys");
      setKeyStatus(status);
      addToast("success", `Keys refreshed! (${status.key_count} keys active)`);
      addLog("info", `Custom keys loaded: ${status.key_count} keys`);
    } catch (err) {
      addToast("error", `Failed to save keys: ${err}`);
    }
  };

  const handleResetAuto = async () => {
    setCustomProd("");
    setCustomTitle("");
    try {
      await invoke("save_settings", {
        settings: {
          custom_prod_keys: null,
          custom_title_keys: null,
          theme: "dark",
          auto_decompress_ncz: true,
        },
      });
      const status = await invoke<KeyStatus>("detect_keys");
      setKeyStatus(status);
      addToast("info", "Reset to automatic emulator key detection.");
    } catch (err) {
      addToast("error", `Failed to reset keys: ${err}`);
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
          <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-[#FF3B53]/15 text-[#FF3B53]">
            <KeyRound className="w-5 h-5" />
          </div>
          <div>
            <div
              className="text-sm font-bold"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Nintendo Switch Keys Inspector
            </div>
            <div className="text-xs text-[#8B9BB4]">
              Automatic discovery for Yuzu, Ryujinx, and custom prod.keys files.
            </div>
          </div>
        </div>

        <button
          onClick={handleResetAuto}
          className="px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#F0F4F8]"
          style={{ borderColor: "var(--border-subtle)" }}
        >
          <RotateCcw className="w-3.5 h-3.5" />
          Auto-Detect
        </button>
      </div>

      {/* Key Status Overview Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {/* Header Key Status */}
        <div
          className={`rounded-xl p-4 border flex items-center gap-3.5 ${
            keyStatus?.has_header_key
              ? "bg-[#10B981]/10 border-[#10B981]/40"
              : "bg-[#F59E0B]/10 border-[#F59E0B]/40"
          }`}
        >
          <div
            className={`w-10 h-10 rounded-xl flex items-center justify-center ${
              keyStatus?.has_header_key
                ? "bg-[#10B981]/20 text-[#10B981]"
                : "bg-[#F59E0B]/20 text-[#F59E0B]"
            }`}
          >
            {keyStatus?.has_header_key ? (
              <ShieldCheck className="w-5 h-5" />
            ) : (
              <AlertTriangle className="w-5 h-5" />
            )}
          </div>
          <div className="flex flex-col">
            <span className="text-xs font-bold" style={{ fontFamily: "var(--font-display)" }}>
              {keyStatus?.has_header_key ? "Header Key Verified" : "Header Key Missing"}
            </span>
            <span className="text-[11px] text-[#8B9BB4]">
              {keyStatus?.has_header_key
                ? "Full NCA decryption & RomFS asset extraction available."
                : "Required to decrypt NCA partitions and game files."}
            </span>
          </div>
        </div>

        {/* Total Keys Count */}
        <div
          className="rounded-xl p-4 border flex items-center gap-3.5"
          style={{
            backgroundColor: "var(--bg-card)",
            borderColor: "var(--border-subtle)",
          }}
        >
          <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-[#00E5FF]/15 text-[#00E5FF]">
            <KeyRound className="w-5 h-5" />
          </div>
          <div className="flex flex-col">
            <span className="text-xs font-bold" style={{ fontFamily: "var(--font-display)" }}>
              {keyStatus?.key_count || 0} Cryptographic Keys Active
            </span>
            <span className="text-[11px] text-[#8B9BB4]">
              Master keys, key area keys, titlekeks, and ticket keys loaded.
            </span>
          </div>
        </div>
      </div>

      {/* Custom Key Overrides */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-4"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div
          className="text-xs font-bold uppercase tracking-wider text-[#8B9BB4]"
          style={{ fontFamily: "var(--font-display)" }}
        >
          Custom Keys Location
        </div>

        {/* prod.keys */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold text-[#F0F4F8]">
            Production Keys (prod.keys)
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={customProd}
              onChange={(e) => setCustomProd(e.target.value)}
              placeholder={keyStatus?.prod_keys_path || "Select prod.keys location..."}
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />
            <button
              onClick={handleSelectProdKeys}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <FolderSearch className="w-3.5 h-3.5 text-[#00E5FF]" />
              Browse
            </button>
          </div>
        </div>

        {/* title.keys */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold text-[#F0F4F8]">
            Title Keys (title.keys - optional)
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={customTitle}
              onChange={(e) => setCustomTitle(e.target.value)}
              placeholder={keyStatus?.title_keys_path || "Select title.keys location (optional)..."}
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />
            <button
              onClick={handleSelectTitleKeys}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <FolderSearch className="w-3.5 h-3.5 text-[#00E5FF]" />
              Browse
            </button>
          </div>
        </div>

        <button
          onClick={handleApplyKeys}
          className="w-full py-2.5 rounded-lg text-xs font-bold text-white bg-[#FF3B53] shadow-md hover:brightness-110 active:scale-[0.99] transition-all cursor-pointer"
          style={{ fontFamily: "var(--font-display)" }}
        >
          Apply Keys
        </button>
      </div>

      {/* Auto-Search Locations Reference */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-2"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div className="text-xs font-bold text-[#8B9BB4] flex items-center gap-2">
          <Folder className="w-4 h-4 text-[#00E5FF]" />
          Automatic Search Paths Scanned on Startup:
        </div>
        <div className="space-y-1 font-mono text-[11px] text-[#8B9BB4]">
          {keyStatus?.search_locations.map((loc, i) => (
            <div key={i} className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-[#53637D]" />
              <span className="truncate">{loc}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
