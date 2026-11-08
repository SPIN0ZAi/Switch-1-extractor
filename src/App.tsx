import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { TitleBar } from "./components/TitleBar";
import { Sidebar } from "./components/Sidebar";
import { ExtractView } from "./components/ExtractView";
import { BatchView } from "./components/BatchView";
import { ConvertView } from "./components/ConvertView";
import { KeysView } from "./components/KeysView";
import { SettingsView } from "./components/SettingsView";
import { LogTerminal } from "./components/LogTerminal";
import { ToastContainer } from "./components/ToastContainer";
import { UpdateModal } from "./components/UpdateModal";
import { useAppStore, KeyStatus, ProgressPayload, UpdateDownloadProgress } from "./store/useAppStore";

export function App() {
  const {
    activeTab,
    setKeyStatus,
    setProgress,
    setConvertProgress,
    setUpdateProgress,
    setUpdateInfo,
    addLog,
    addToast,
    setCurrentGame,
    setIsInspecting,
    setOutputDir,
  } = useAppStore();

  useEffect(() => {
    // 1. Initial Keys & Settings Detection
    const initApp = async () => {
      try {
        const keys = await invoke<KeyStatus>("detect_keys");
        setKeyStatus(keys);
        if (keys.has_header_key) {
          addLog("success", `Emulator keys loaded (${keys.key_count} keys active).`);
        } else {
          addLog("warn", "prod.keys not found in standard paths. RomFS extraction requires keys.");
        }

        const settings: any = await invoke("get_settings");
        if (settings.default_output_dir) {
          setOutputDir(settings.default_output_dir);
        }

        // Background update check if enabled
        if (settings.check_updates_on_startup !== false) {
          setTimeout(async () => {
            try {
              const repo = settings.github_repo || "SPIN0ZAi/Switch-1-extractor";
              const info = await invoke<any>("check_for_updates", { repo });
              setUpdateInfo(info);
              if (info.update_available) {
                addLog("info", `Update available: ${info.latest_version}`);
              }
            } catch {
              // Silent in background
            }
          }, 1500);
        }

        const initialFile = await invoke<string | null>("get_initial_file");
        if (initialFile) {
          setIsInspecting(true);
          try {
            const info = await invoke<any>("inspect_game", { path: initialFile });
            setCurrentGame(info);
            addToast("success", `Loaded: ${info.title_name}`);
          } catch (err) {
            addToast("error", `Failed to inspect initial file: ${err}`);
          } finally {
            setIsInspecting(false);
          }
        }
      } catch (err) {
        addLog("error", `Failed to initialize app state: ${err}`);
      }
    };
    initApp();

    // 2. Tauri Event Listeners
    let unlistenProgress: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;
    let unlistenConvert: (() => void) | undefined;
    let unlistenUpdateProgress: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenProgress = await listen<ProgressPayload>("extract-progress", (event) => {
          setProgress(event.payload);
        });

        unlistenLog = await listen<{ level: string; message: string; timestamp: string }>(
          "extract-log",
          (event) => {
            addLog(event.payload.level as any, event.payload.message, event.payload.timestamp);
          },
        );

        unlistenConvert = await listen<ProgressPayload>("convert-progress", (event) => {
          setConvertProgress(event.payload);
        });

        unlistenUpdateProgress = await listen<UpdateDownloadProgress>("update-download-progress", (event) => {
          setUpdateProgress(event.payload);
        });
      } catch {
        // Fallback in web dev mode
      }
    };

    setupListeners();

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenLog) unlistenLog();
      if (unlistenConvert) unlistenConvert();
      if (unlistenUpdateProgress) unlistenUpdateProgress();
    };
  }, [setKeyStatus, setProgress, setConvertProgress, addLog, setOutputDir]);

  // Handle Drag & Drop on the window
  useEffect(() => {
    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    const handleDrop = async (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        const file = files[0];
        // In Tauri, web path can be obtained via path property
        const filePath = (file as any).path || file.name;
        if (filePath) {
          setIsInspecting(true);
          try {
            const info = await invoke<any>("inspect_game", { path: filePath });
            setCurrentGame(info);
            addToast("success", `Loaded: ${info.title_name}`);
          } catch (err) {
            addToast("error", `Failed to inspect file: ${err}`);
          } finally {
            setIsInspecting(false);
          }
        }
      }
    };

    window.addEventListener("dragover", handleDragOver);
    window.addEventListener("drop", handleDrop);

    return () => {
      window.removeEventListener("dragover", handleDragOver);
      window.removeEventListener("drop", handleDrop);
    };
  }, [setCurrentGame, setIsInspecting, addToast]);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden select-none">
      {/* Custom Titlebar */}
      <TitleBar />

      {/* Main Workspace Body */}
      <div className="flex-1 flex overflow-hidden">
        {/* Console Sidebar */}
        <Sidebar />

        {/* Dynamic Content Views */}
        <main className="flex-1 flex flex-col h-full overflow-hidden bg-[var(--bg-canvas)]">
          {activeTab === "extract" && <ExtractView />}
          {activeTab === "batch" && <BatchView />}
          {activeTab === "convert" && <ConvertView />}
          {activeTab === "keys" && <KeysView />}
          {activeTab === "settings" && <SettingsView />}
        </main>
      </div>

      {/* Collapsible Log Terminal */}
      <LogTerminal />

      {/* Global Notifications */}
      <ToastContainer />

      {/* Software Update Modal */}
      <UpdateModal />
    </div>
  );
}

export default App;
