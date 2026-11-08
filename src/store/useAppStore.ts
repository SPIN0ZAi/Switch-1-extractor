import { create } from "zustand";

export interface KeyStatus {
  has_header_key: boolean;
  key_count: number;
  prod_keys_path: string | null;
  title_keys_path: string | null;
  search_locations: string[];
}

export interface SwitchGameInfo {
  file_path: string | null;
  format: string;
  file_size: number;
  file_size_formatted: string;
  title_name: string;
  publisher: string;
  display_version: string;
  title_id: string;
  icon_base64: string | null;
  files_list: string[];
  has_keys: boolean;
  is_compressed: boolean;
  has_ticket: boolean;
  ticket_decrypted: boolean;
  is_update?: boolean;
  requires_base_game?: boolean;
}

export interface ProgressPayload {
  stage: string;
  current_file: number;
  total_files: number;
  file_name: string;
  bytes_done: number;
  bytes_total: number;
  percent: number;
}

export interface DlcSummary {
  name: string;
  title_id: string;
  file_path: string;
  romfs_count: number;
  nca_count: number;
  error?: string | null;
}

export interface UpdateInfo {
  update_available: boolean;
  current_version: string;
  latest_version: string;
  release_name: string;
  release_notes: string;
  release_url: string;
  download_url: string | null;
  asset_name: string | null;
  asset_size: number | null;
  published_at: string;
}

export interface UpdateDownloadProgress {
  bytes_downloaded: number;
  total_bytes: number;
  percent: number;
}

export interface ExtractionSummary {
  ncas: string[];
  romfs: string[];
  exefs: string[];
  metadata: string[];
  romfs_error: string | null;
  exefs_error: string | null;
  dlcs?: DlcSummary[];
}

export interface LogItem {
  id: string;
  level: "info" | "success" | "warn" | "error";
  message: string;
  timestamp: string;
}

export interface ToastItem {
  id: string;
  type: "info" | "success" | "error" | "warning";
  message: string;
}

export interface BatchItem {
  id: string;
  path: string;
  fileName: string;
  status: "queued" | "inspecting" | "extracting" | "completed" | "failed";
  info?: SwitchGameInfo;
  error?: string;
}

export type TabType = "extract" | "batch" | "convert" | "keys" | "settings";

interface AppState {
  activeTab: TabType;
  setActiveTab: (tab: TabType) => void;

  theme: "dark" | "light";
  setTheme: (theme: "dark" | "light") => void;

  keyStatus: KeyStatus | null;
  setKeyStatus: (status: KeyStatus | null) => void;

  currentGame: SwitchGameInfo | null;
  setCurrentGame: (game: SwitchGameInfo | null) => void;

  isInspecting: boolean;
  setIsInspecting: (inspecting: boolean) => void;

  // Extraction Options
  extractNcas: boolean;
  setExtractNcas: (val: boolean) => void;
  extractRomfs: boolean;
  setExtractRomfs: (val: boolean) => void;
  extractExefs: boolean;
  setExtractExefs: (val: boolean) => void;
  extractMetadata: boolean;
  setExtractMetadata: (val: boolean) => void;
  outputDir: string;
  setOutputDir: (dir: string) => void;
  baseGamePath: string | null;
  setBaseGamePath: (path: string | null) => void;
  updatePath: string | null;
  setUpdatePath: (path: string | null) => void;
  dlcPaths: string[];
  addDlcPath: (path: string) => void;
  removeDlcPath: (index: number) => void;
  clearDlcPaths: () => void;

  isExtracting: boolean;
  setIsExtracting: (extracting: boolean) => void;
  progress: ProgressPayload | null;
  setProgress: (p: ProgressPayload | null) => void;
  summary: ExtractionSummary | null;
  setSummary: (s: ExtractionSummary | null) => void;

  // Conversion
  convertInputPath: string;
  setConvertInputPath: (path: string) => void;
  convertOutputPath: string;
  setConvertOutputPath: (path: string) => void;
  isConverting: boolean;
  setIsConverting: (converting: boolean) => void;
  convertProgress: ProgressPayload | null;
  setConvertProgress: (p: ProgressPayload | null) => void;

  // Batch
  batchQueue: BatchItem[];
  addToBatch: (paths: string[]) => void;
  removeFromBatch: (id: string) => void;
  clearBatch: () => void;
  isBatchRunning: boolean;
  setIsBatchRunning: (running: boolean) => void;
  updateBatchItem: (id: string, update: Partial<BatchItem>) => void;

  // Logs
  logs: LogItem[];
  addLog: (level: LogItem["level"], message: string, timestamp?: string) => void;
  clearLogs: () => void;
  isLogOpen: boolean;
  setIsLogOpen: (open: boolean) => void;

  // Toasts
  toasts: ToastItem[];
  addToast: (type: ToastItem["type"], message: string) => void;
  removeToast: (id: string) => void;

  // Software Updates
  appVersion: string;
  setAppVersion: (v: string) => void;
  updateInfo: UpdateInfo | null;
  setUpdateInfo: (info: UpdateInfo | null) => void;
  isCheckingUpdate: boolean;
  setIsCheckingUpdate: (checking: boolean) => void;
  isUpdating: boolean;
  setIsUpdating: (updating: boolean) => void;
  updateProgress: UpdateDownloadProgress | null;
  setUpdateProgress: (p: UpdateDownloadProgress | null) => void;
  isUpdateModalOpen: boolean;
  setIsUpdateModalOpen: (open: boolean) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  activeTab: "extract",
  setActiveTab: (tab) => set({ activeTab: tab }),

  theme: "dark",
  setTheme: (theme) => {
    document.documentElement.setAttribute("data-theme", theme);
    set({ theme });
  },

  keyStatus: null,
  setKeyStatus: (keyStatus) => set({ keyStatus }),

  currentGame: null,
  setCurrentGame: (currentGame) => set({ currentGame }),

  isInspecting: false,
  setIsInspecting: (isInspecting) => set({ isInspecting }),

  extractNcas: true,
  setExtractNcas: (extractNcas) => set({ extractNcas }),
  extractRomfs: true,
  setExtractRomfs: (extractRomfs) => set({ extractRomfs }),
  extractExefs: true,
  setExtractExefs: (extractExefs) => set({ extractExefs }),
  extractMetadata: true,
  setExtractMetadata: (extractMetadata) => set({ extractMetadata }),
  outputDir: "",
  setOutputDir: (outputDir) => set({ outputDir }),
  baseGamePath: null,
  setBaseGamePath: (baseGamePath) => set({ baseGamePath }),
  updatePath: null,
  setUpdatePath: (updatePath) => set({ updatePath }),
  dlcPaths: [],
  addDlcPath: (path) =>
    set((state) => ({
      dlcPaths: state.dlcPaths.includes(path) ? state.dlcPaths : [...state.dlcPaths, path],
    })),
  removeDlcPath: (index) =>
    set((state) => ({
      dlcPaths: state.dlcPaths.filter((_, i) => i !== index),
    })),
  clearDlcPaths: () => set({ dlcPaths: [] }),

  isExtracting: false,
  setIsExtracting: (isExtracting) => set({ isExtracting }),
  progress: null,
  setProgress: (progress) => set({ progress }),
  summary: null,
  setSummary: (summary) => set({ summary }),

  convertInputPath: "",
  setConvertInputPath: (convertInputPath) => set({ convertInputPath }),
  convertOutputPath: "",
  setConvertOutputPath: (convertOutputPath) => set({ convertOutputPath }),
  isConverting: false,
  setIsConverting: (isConverting) => set({ isConverting }),
  convertProgress: null,
  setConvertProgress: (convertProgress) => set({ convertProgress }),

  batchQueue: [],
  addToBatch: (paths) => {
    const newItems: BatchItem[] = paths.map((p) => {
      const fileName = p.split(/[/\\]/).pop() || p;
      return {
        id: Math.random().toString(36).substring(2, 9),
        path: p,
        fileName,
        status: "queued",
      };
    });
    set((state) => ({ batchQueue: [...state.batchQueue, ...newItems] }));
  },
  removeFromBatch: (id) =>
    set((state) => ({
      batchQueue: state.batchQueue.filter((item) => item.id !== id),
    })),
  clearBatch: () => set({ batchQueue: [] }),
  isBatchRunning: false,
  setIsBatchRunning: (isBatchRunning) => set({ isBatchRunning }),
  updateBatchItem: (id, update) =>
    set((state) => ({
      batchQueue: state.batchQueue.map((item) =>
        item.id === id ? { ...item, ...update } : item,
      ),
    })),

  logs: [],
  addLog: (level, message, timestamp) => {
    const time =
      timestamp ||
      new Date().toTimeString().split(" ")[0];
    const logItem: LogItem = {
      id: Math.random().toString(36).substring(2, 9),
      level,
      message,
      timestamp: time,
    };
    set((state) => ({ logs: [logItem, ...state.logs].slice(0, 500) }));
  },
  clearLogs: () => set({ logs: [] }),
  isLogOpen: false,
  setIsLogOpen: (isLogOpen) => set({ isLogOpen }),

  toasts: [],
  addToast: (type, message) => {
    const id = Math.random().toString(36).substring(2, 9);
    set((state) => ({ toasts: [...state.toasts, { id, type, message }] }));
    setTimeout(() => {
      get().removeToast(id);
    }, 4500);
  },
  removeToast: (id) =>
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),

  // Software Updates
  appVersion: "1.0.0",
  setAppVersion: (appVersion) => set({ appVersion }),
  updateInfo: null,
  setUpdateInfo: (updateInfo) => set({ updateInfo }),
  isCheckingUpdate: false,
  setIsCheckingUpdate: (isCheckingUpdate) => set({ isCheckingUpdate }),
  isUpdating: false,
  setIsUpdating: (isUpdating) => set({ isUpdating }),
  updateProgress: null,
  setUpdateProgress: (updateProgress) => set({ updateProgress }),
  isUpdateModalOpen: false,
  setIsUpdateModalOpen: (isUpdateModalOpen) => set({ isUpdateModalOpen }),
}));

