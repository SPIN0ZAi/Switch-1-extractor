import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Sparkles,
  Download,
  ExternalLink,
  X,
  Loader2,
} from "lucide-react";
import { useAppStore } from "../store/useAppStore";

export const UpdateModal: React.FC = () => {
  const {
    updateInfo,
    isUpdateModalOpen,
    setIsUpdateModalOpen,
    isUpdating,
    setIsUpdating,
    updateProgress,
    addToast,
    addLog,
  } = useAppStore();

  if (!isUpdateModalOpen || !updateInfo) {
    return null;
  }

  const handleStartUpdate = async () => {
    if (!updateInfo.download_url) {
      if (updateInfo.release_url) {
        await openUrl(updateInfo.release_url);
      }
      return;
    }

    setIsUpdating(true);
    addLog("info", `Downloading update from: ${updateInfo.download_url}`);

    try {
      await invoke("download_and_install_update", {
        downloadUrl: updateInfo.download_url,
      });
      addToast("success", "Update installed! Restarting application...");
    } catch (err) {
      setIsUpdating(false);
      addToast("error", `Update failed: ${err}`);
      addLog("error", `Update installation error: ${err}`);
    }
  };

  const handleOpenReleaseUrl = async () => {
    if (updateInfo.release_url) {
      try {
        await openUrl(updateInfo.release_url);
      } catch (err) {
        addToast("error", `Could not open browser: ${err}`);
      }
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm animate-in fade-in duration-200">
      <div
        className="w-full max-w-lg rounded-2xl border p-6 flex flex-col gap-4 shadow-2xl relative"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "#00E5FF80",
          boxShadow: "0 0 30px rgba(0, 229, 255, 0.15)",
        }}
      >
        {/* Close Button */}
        {!isUpdating && (
          <button
            onClick={() => setIsUpdateModalOpen(false)}
            className="absolute top-4 right-4 p-1.5 rounded-lg border hover:bg-white/5 text-[#8B9BB4] hover:text-[#F0F4F8] transition-colors"
            style={{ borderColor: "var(--border-subtle)" }}
          >
            <X className="w-4 h-4" />
          </button>
        )}

        {/* Modal Header */}
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl flex items-center justify-center bg-[#00E5FF]/15 text-[#00E5FF]">
            <Sparkles className="w-5 h-5" />
          </div>
          <div>
            <div
              className="text-base font-bold text-[#F0F4F8]"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Software Update Available
            </div>
            <div className="text-xs text-[#8B9BB4]">
              A new version of Switch Extractor is ready to download.
            </div>
          </div>
        </div>

        {/* Version Compare Banner */}
        <div
          className="flex items-center justify-between p-3 rounded-xl border text-xs"
          style={{
            backgroundColor: "var(--bg-input)",
            borderColor: "var(--border-subtle)",
          }}
        >
          <div className="flex items-center gap-2">
            <span className="text-[#8B9BB4]">Current:</span>
            <span className="font-mono font-bold text-[#8B9BB4]">
              v{updateInfo.current_version}
            </span>
          </div>

          <span className="text-[#00E5FF] font-bold">➔</span>

          <div className="flex items-center gap-2">
            <span className="text-[#F0F4F8] font-bold">New Version:</span>
            <span className="font-mono font-bold px-2 py-0.5 rounded-full bg-[#00E5FF]/20 text-[#00E5FF] border border-[#00E5FF]/30">
              {updateInfo.latest_version}
            </span>
          </div>
        </div>

        {/* Release Notes Box */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between text-xs text-[#8B9BB4]">
            <span className="font-bold uppercase tracking-wider text-[10px]">
              Release Notes &amp; Changelog
            </span>
            {updateInfo.published_at && (
              <span className="text-[10px]">
                {new Date(updateInfo.published_at).toLocaleDateString()}
              </span>
            )}
          </div>

          <div
            className="max-h-48 overflow-y-auto p-3 rounded-xl border text-xs font-mono text-[#F0F4F8] whitespace-pre-wrap leading-relaxed select-text"
            style={{
              backgroundColor: "var(--bg-input)",
              borderColor: "var(--border-subtle)",
            }}
          >
            {updateInfo.release_notes || "No release notes provided for this release."}
          </div>
        </div>

        {/* Download & Install Progress Bar (if in progress) */}
        {isUpdating ? (
          <div className="flex flex-col gap-2 p-3 rounded-xl bg-[#00E5FF]/10 border border-[#00E5FF]/30">
            <div className="flex items-center justify-between text-xs font-semibold text-[#00E5FF]">
              <span className="flex items-center gap-2">
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                Downloading &amp; Applying Update...
              </span>
              <span className="font-mono">
                {updateProgress?.percent.toFixed(1) || "0.0"}%
              </span>
            </div>

            <div
              className="w-full h-2 rounded-full overflow-hidden"
              style={{ backgroundColor: "var(--bg-input)" }}
            >
              <div
                className="h-full bg-gradient-to-r from-[#00E5FF] to-[#10B981] transition-all duration-150"
                style={{ width: `${updateProgress?.percent || 0}%` }}
              />
            </div>

            <div className="flex items-center justify-between text-[11px] font-mono text-[#8B9BB4]">
              <span>
                {updateProgress
                  ? `${formatBytes(updateProgress.bytes_downloaded)} / ${formatBytes(updateProgress.total_bytes)}`
                  : "Connecting to GitHub..."}
              </span>
              <span>The app will restart automatically</span>
            </div>
          </div>
        ) : (
          /* Action Buttons */
          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              onClick={() => setIsUpdateModalOpen(false)}
              className="px-4 py-2 rounded-lg text-xs font-medium border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              Later
            </button>

            <button
              onClick={handleOpenReleaseUrl}
              className="px-3.5 py-2 rounded-lg text-xs font-medium border hover:bg-white/5 transition-colors text-[#8B9BB4] hover:text-[#F0F4F8] flex items-center gap-1.5"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <ExternalLink className="w-3.5 h-3.5" />
              GitHub
            </button>

            {updateInfo.download_url ? (
              <button
                onClick={handleStartUpdate}
                className="px-5 py-2 rounded-lg text-xs font-bold bg-[#00E5FF] text-black hover:brightness-110 active:scale-95 transition-all flex items-center gap-1.5 cursor-pointer shadow-lg shadow-[#00E5FF]/20"
                style={{ fontFamily: "var(--font-display)" }}
              >
                <Download className="w-3.5 h-3.5" />
                Download &amp; Restart
              </button>
            ) : (
              <button
                onClick={handleOpenReleaseUrl}
                className="px-5 py-2 rounded-lg text-xs font-bold bg-[#00E5FF] text-black hover:brightness-110 active:scale-95 transition-all flex items-center gap-1.5 cursor-pointer"
                style={{ fontFamily: "var(--font-display)" }}
              >
                <ExternalLink className="w-3.5 h-3.5" />
                View Release on GitHub
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
