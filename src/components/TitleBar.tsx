import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, Copy, X, KeyRound, Sun, Moon, Disc3, Sparkles } from "lucide-react";
import { useAppStore } from "../store/useAppStore";

export const TitleBar: React.FC = () => {
  const { keyStatus, theme, setTheme, appVersion, setAppVersion, updateInfo, setIsUpdateModalOpen } = useAppStore();
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const checkMax = async () => {
      try {
        const appWindow = getCurrentWindow();
        setIsMaximized(await appWindow.isMaximized());
      } catch {
        // Fallback
      }
    };
    checkMax();

    const fetchVer = async () => {
      try {
        const ver = await invoke<string>("get_app_version");
        if (ver) setAppVersion(ver);
      } catch {
        // Fallback
      }
    };
    fetchVer();
  }, [setAppVersion]);

  const handleMinimize = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("window_minimize");
    } catch {
      try {
        await getCurrentWindow().minimize();
      } catch (err) {
        console.error("Minimize error:", err);
      }
    }
  };

  const handleToggleMaximize = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("window_toggle_maximize");
      setIsMaximized(!isMaximized);
    } catch {
      try {
        const appWindow = getCurrentWindow();
        await appWindow.toggleMaximize();
        setIsMaximized(await appWindow.isMaximized());
      } catch (err) {
        console.error("Maximize error:", err);
      }
    }
  };

  const handleClose = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("window_close");
    } catch {
      try {
        await getCurrentWindow().close();
      } catch (err) {
        console.error("Close error:", err);
      }
    }
  };

  const toggleTheme = (e: React.MouseEvent) => {
    e.stopPropagation();
    setTheme(theme === "dark" ? "light" : "dark");
  };

  return (
    <header
      className="h-9 w-full flex items-center justify-between px-3 select-none border-b transition-colors relative z-40"
      style={{
        backgroundColor: "var(--bg-sidebar)",
        borderColor: "var(--border-subtle)",
      }}
    >
      {/* Brand Title (Draggable) */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-2.5 h-full cursor-default titlebar-drag flex-1"
      >
        <div className="flex items-center justify-center w-5 h-5 rounded bg-[#FF3B53]/15 text-[#FF3B53]">
          <Disc3 className="w-3.5 h-3.5 animate-spin-slow" />
        </div>
        <span
          data-tauri-drag-region
          className="text-xs font-semibold tracking-wider uppercase pointer-events-none"
          style={{
            fontFamily: "var(--font-display)",
            color: "var(--text-main)",
          }}
        >
          Switch Extractor
        </span>
        <span
          data-tauri-drag-region
          className="text-[10px] px-1.5 py-0.5 rounded font-mono font-medium pointer-events-none"
          style={{
            backgroundColor: "var(--bg-input)",
            color: "var(--accent-coral)",
            border: "1px solid var(--border-subtle)",
          }}
        >
          v{appVersion}
        </span>
      </div>

      {/* Middle Status Pill (Draggable area) */}
      <div
        data-tauri-drag-region
        className="flex items-center justify-center gap-3 h-full flex-1 titlebar-drag cursor-default"
      >
        {updateInfo?.update_available && (
          <button
            onClick={() => setIsUpdateModalOpen(true)}
            className="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[11px] font-bold bg-[#00E5FF]/20 text-[#00E5FF] border border-[#00E5FF]/40 hover:bg-[#00E5FF]/30 transition-all cursor-pointer animate-pulse shrink-0"
            style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
          >
            <Sparkles className="w-3 h-3" />
            <span>Update {updateInfo.latest_version} Available!</span>
          </button>
        )}

        {keyStatus && (
          <div
            className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-medium transition-all pointer-events-none"
            style={{
              backgroundColor: keyStatus.has_header_key
                ? "rgba(16, 185, 129, 0.12)"
                : "rgba(245, 158, 11, 0.12)",
              color: keyStatus.has_header_key ? "#10B981" : "#F59E0B",
              border: `1px solid ${
                keyStatus.has_header_key
                  ? "rgba(16, 185, 129, 0.3)"
                  : "rgba(245, 158, 11, 0.3)"
              }`,
            }}
          >
            <div
              className={`w-1.5 h-1.5 rounded-full ${
                keyStatus.has_header_key
                  ? "bg-[#10B981] animate-pulse"
                  : "bg-[#F59E0B]"
              }`}
            />
            <KeyRound className="w-3 h-3" />
            <span className="font-mono">
              {keyStatus.has_header_key
                ? `${keyStatus.key_count} Keys Loaded`
                : "Keys Missing"}
            </span>
          </div>
        )}
      </div>

      {/* Window Controls & Theme Toggle (STRICTLY NO DRAG) */}
      <div
        className="flex items-center gap-1 z-50 shrink-0"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        <button
          onClick={toggleTheme}
          title={theme === "dark" ? "Switch to Light Mode" : "Switch to Dark Mode"}
          className="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10 text-muted transition-colors cursor-pointer"
          style={{
            color: "var(--text-muted)",
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties}
        >
          {theme === "dark" ? (
            <Sun className="w-3.5 h-3.5" />
          ) : (
            <Moon className="w-3.5 h-3.5" />
          )}
        </button>

        <button
          onClick={handleMinimize}
          title="Minimize"
          className="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10 transition-colors cursor-pointer"
          style={{
            color: "var(--text-muted)",
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties}
        >
          <Minus className="w-3.5 h-3.5" />
        </button>

        <button
          onClick={handleToggleMaximize}
          title={isMaximized ? "Restore Down" : "Maximize"}
          className="w-7 h-7 flex items-center justify-center rounded hover:bg-white/10 transition-colors cursor-pointer"
          style={{
            color: "var(--text-muted)",
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties}
        >
          {isMaximized ? <Copy className="w-3 h-3" /> : <Square className="w-3 h-3" />}
        </button>

        <button
          onClick={handleClose}
          title="Close"
          className="w-7 h-7 flex items-center justify-center rounded hover:bg-[#EF4444] hover:text-white transition-colors cursor-pointer"
          style={{
            color: "var(--text-muted)",
            WebkitAppRegion: "no-drag",
          } as React.CSSProperties}
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </header>
  );
};
