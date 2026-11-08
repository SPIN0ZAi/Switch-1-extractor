import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Settings,
  FolderSearch,
  Moon,
  Sun,
  ShieldCheck,
  Cpu,
  Info,
  Sparkles,
  RefreshCw,
  GitBranch,
} from "lucide-react";
import { useAppStore, UpdateInfo } from "../store/useAppStore";

export const SettingsView: React.FC = () => {
  const {
    outputDir,
    setOutputDir,
    theme,
    setTheme,
    appVersion,
    setAppVersion,
    setUpdateInfo,
    isCheckingUpdate,
    setIsCheckingUpdate,
    setIsUpdateModalOpen,
    addToast,
    addLog,
  } = useAppStore();

  const [githubRepo, setGithubRepo] = useState("switch-extractor/switch-extractor-desktop");
  const [checkOnStartup, setCheckOnStartup] = useState(true);
  const [lastCheckResult, setLastCheckResult] = useState<string | null>(null);

  useEffect(() => {
    const loadConfig = async () => {
      try {
        const s: any = await invoke("get_settings");
        if (s.default_output_dir && !outputDir) {
          setOutputDir(s.default_output_dir);
        }
        if (s.github_repo) {
          setGithubRepo(s.github_repo);
        }
        if (typeof s.check_updates_on_startup === "boolean") {
          setCheckOnStartup(s.check_updates_on_startup);
        }
      } catch {
        // Ignored
      }

      try {
        const ver = await invoke<string>("get_app_version");
        if (ver) setAppVersion(ver);
      } catch {
        // Ignored
      }
    };
    loadConfig();
  }, [outputDir, setOutputDir, setAppVersion]);

  const handleSelectDefaultOutput = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        setOutputDir(selected);
        await saveSettings(selected, githubRepo, checkOnStartup);
        addToast("success", "Default output path saved.");
      }
    } catch (err) {
      addToast("error", `Folder picker error: ${err}`);
    }
  };

  const saveSettings = async (out: string, repo: string, onStartup: boolean) => {
    try {
      await invoke("save_settings", {
        settings: {
          default_output_dir: out || null,
          theme,
          auto_decompress_ncz: true,
          github_repo: repo,
          check_updates_on_startup: onStartup,
        },
      });
    } catch {
      // Ignored
    }
  };

  const handleRepoChange = async (val: string) => {
    setGithubRepo(val);
    await saveSettings(outputDir, val, checkOnStartup);
  };

  const handleStartupToggle = async (val: boolean) => {
    setCheckOnStartup(val);
    await saveSettings(outputDir, githubRepo, val);
  };

  const handleCheckForUpdates = async () => {
    setIsCheckingUpdate(true);
    setLastCheckResult(null);
    addLog("info", `Checking for updates via GitHub repo: ${githubRepo}`);

    try {
      const info = await invoke<UpdateInfo>("check_for_updates", { repo: githubRepo });
      setUpdateInfo(info);

      if (info.update_available) {
        setLastCheckResult(`Update available: ${info.latest_version}`);
        addToast("success", `New version ${info.latest_version} available!`);
        setIsUpdateModalOpen(true);
      } else {
        setLastCheckResult(`You are on the latest version (v${info.current_version})`);
        addToast("info", "You are up to date! No new updates available.");
      }
    } catch (err) {
      setLastCheckResult(`Check failed: ${err}`);
      addToast("error", `Could not check for updates: ${err}`);
      addLog("error", `Update check failed: ${err}`);
    } finally {
      setIsCheckingUpdate(false);
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
            <Settings className="w-5 h-5" />
          </div>
          <div>
            <div
              className="text-sm font-bold text-[#F0F4F8]"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Application Settings
            </div>
            <div className="text-xs text-[#8B9BB4]">
              Manage global paths, software updates, visual appearance, and hardware acceleration options.
            </div>
          </div>
        </div>
      </div>

      {/* ============================================================ */}
      {/* 1. SOFTWARE UPDATES (GITHUB RELEASES) */}
      {/* ============================================================ */}
      <div
        className="rounded-xl p-4 border flex flex-col gap-4 shadow-sm"
        style={{
          backgroundColor: "var(--bg-card)",
          borderColor: "var(--border-subtle)",
        }}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-[#00E5FF]" />
            <div
              className="text-xs font-bold uppercase tracking-wider text-[#F0F4F8]"
              style={{ fontFamily: "var(--font-display)" }}
            >
              Software Updates (GitHub Releases)
            </div>
          </div>

          <span className="text-[11px] font-mono font-bold px-2 py-0.5 rounded-full bg-white/5 border border-white/10 text-[#8B9BB4]">
            Installed: v{appVersion}
          </span>
        </div>

        <p className="text-xs text-[#8B9BB4] leading-relaxed">
          Switch Extractor can automatically check GitHub Releases for new features, bugfixes, and updated Nintendo Switch keys support.
        </p>

        {/* GitHub Repository Target */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold text-[#F0F4F8] flex items-center gap-1.5">
            <GitBranch className="w-3.5 h-3.5 text-[#00E5FF]" />
            GitHub Repository (<code className="text-[#00E5FF]">owner/repo</code>)
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={githubRepo}
              onChange={(e) => handleRepoChange(e.target.value)}
              placeholder="e.g. switch-extractor/switch-extractor-desktop"
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />

            <button
              onClick={handleCheckForUpdates}
              disabled={isCheckingUpdate}
              className="px-4 py-2 rounded-lg text-xs font-bold bg-[#00E5FF] text-black hover:brightness-110 active:scale-95 transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-50 shrink-0 shadow-sm"
              style={{ fontFamily: "var(--font-display)" }}
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isCheckingUpdate ? "animate-spin" : ""}`} />
              {isCheckingUpdate ? "Checking GitHub..." : "Check for Updates"}
            </button>
          </div>
        </div>

        {/* Startup Check Option */}
        <label className="flex items-center gap-2.5 text-xs text-[#F0F4F8] cursor-pointer pt-1">
          <input
            type="checkbox"
            checked={checkOnStartup}
            onChange={(e) => handleStartupToggle(e.target.checked)}
            className="accent-[#00E5FF]"
          />
          <span>Check for updates automatically on application launch</span>
        </label>

        {/* Last check status message */}
        {lastCheckResult && (
          <div className="text-xs font-mono p-2.5 rounded-lg border flex items-center gap-2 bg-black/20 border-white/5 text-[#F0F4F8]">
            <Info className="w-3.5 h-3.5 text-[#00E5FF] shrink-0" />
            <span>{lastCheckResult}</span>
          </div>
        )}
      </div>

      {/* ============================================================ */}
      {/* 2. GENERAL PREFERENCES */}
      {/* ============================================================ */}
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
          General Preferences
        </div>

        {/* Default Output Directory */}
        <div className="flex flex-col gap-1.5">
          <label className="text-xs font-bold text-[#F0F4F8]">
            Default Extraction Directory
          </label>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={outputDir}
              onChange={(e) => setOutputDir(e.target.value)}
              placeholder="Select default folder for extractions..."
              className="flex-1 px-3 py-2 rounded-lg text-xs font-mono border outline-none"
              style={{
                backgroundColor: "var(--bg-input)",
                borderColor: "var(--border-subtle)",
                color: "var(--text-main)",
              }}
            />
            <button
              onClick={handleSelectDefaultOutput}
              className="px-3 py-2 rounded-lg text-xs font-medium flex items-center gap-1.5 border hover:bg-white/5 transition-colors text-[#F0F4F8]"
              style={{ borderColor: "var(--border-subtle)" }}
            >
              <FolderSearch className="w-3.5 h-3.5 text-[#00E5FF]" />
              Browse
            </button>
          </div>
        </div>

        {/* Appearance Theme */}
        <div className="flex flex-col gap-2 pt-2 border-t" style={{ borderColor: "var(--border-subtle)" }}>
          <label className="text-xs font-bold text-[#F0F4F8]">
            Interface Theme
          </label>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setTheme("dark")}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 rounded-lg border text-xs font-bold transition-all ${
                theme === "dark"
                  ? "bg-[#FF3B53]/15 border-[#FF3B53] text-[#FF3B53]"
                  : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
              }`}
            >
              <Moon className="w-4 h-4" />
              Nintendo OLED Dark
            </button>

            <button
              onClick={() => setTheme("light")}
              className={`flex-1 flex items-center justify-center gap-2 py-2.5 rounded-lg border text-xs font-bold transition-all ${
                theme === "light"
                  ? "bg-[#FF3B53]/15 border-[#FF3B53] text-[#FF3B53]"
                  : "border-[var(--border-subtle)] text-[#8B9BB4] hover:bg-white/5"
              }`}
            >
              <Sun className="w-4 h-4" />
              Clean Light
            </button>
          </div>
        </div>
      </div>

      {/* ============================================================ */}
      {/* 3. SYSTEM DIAGNOSTICS CARD */}
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
          Engine Architecture &amp; Runtime
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs">
          <div
            className="p-3 rounded-lg border flex flex-col gap-1"
            style={{
              backgroundColor: "var(--bg-input)",
              borderColor: "var(--border-subtle)",
            }}
          >
            <div className="flex items-center gap-1.5 text-[#00E5FF] font-semibold">
              <Cpu className="w-4 h-4" /> Hardware Crypto
            </div>
            <div className="text-[#8B9BB4] text-[11px]">
              AES-NI instruction acceleration active for line-rate sector decryption.
            </div>
          </div>

          <div
            className="p-3 rounded-lg border flex flex-col gap-1"
            style={{
              backgroundColor: "var(--bg-input)",
              borderColor: "var(--border-subtle)",
            }}
          >
            <div className="flex items-center gap-1.5 text-[#10B981] font-semibold">
              <ShieldCheck className="w-4 h-4" /> Standalone Binary
            </div>
            <div className="text-[#8B9BB4] text-[11px]">
              Pure Rust backend. 0% Python dependencies. Single self-contained .exe.
            </div>
          </div>

          <div
            className="p-3 rounded-lg border flex flex-col gap-1"
            style={{
              backgroundColor: "var(--bg-input)",
              borderColor: "var(--border-subtle)",
            }}
          >
            <div className="flex items-center gap-1.5 text-[#FF3B53] font-semibold">
              <Info className="w-4 h-4" /> Formats Covered
            </div>
            <div className="text-[#8B9BB4] text-[11px]">
              NSP, XCI, NSZ, XCZ, NCA, PFS0, HFS0, RomFS, and NACP.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
