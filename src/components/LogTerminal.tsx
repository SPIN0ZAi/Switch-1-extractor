import React, { useRef, useEffect } from "react";
import { Terminal, X, Copy, Trash2 } from "lucide-react";
import { useAppStore, LogItem } from "../store/useAppStore";

export const LogTerminal: React.FC = () => {
  const { logs, clearLogs, isLogOpen, setIsLogOpen, addToast } = useAppStore();
  const logContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  if (!isLogOpen) return null;

  const handleCopyLogs = () => {
    const text = logs
      .map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] ${l.message}`)
      .join("\n");
    navigator.clipboard.writeText(text);
    addToast("info", "Logs copied to clipboard.");
  };

  const getLevelStyle = (level: LogItem["level"]) => {
    switch (level) {
      case "success":
        return "text-[#10B981]";
      case "error":
        return "text-[#EF4444] font-bold";
      case "warn":
        return "text-[#F59E0B]";
      default:
        return "text-[#00E5FF]";
    }
  };

  return (
    <div
      className="h-44 border-t flex flex-col z-20 select-text transition-all"
      style={{
        backgroundColor: "var(--bg-input)",
        borderColor: "var(--border-subtle)",
      }}
    >
      {/* Terminal Title Header */}
      <div
        className="h-7 px-3 flex items-center justify-between border-b select-none"
        style={{
          borderColor: "var(--border-subtle)",
          backgroundColor: "var(--bg-sidebar)",
        }}
      >
        <div className="flex items-center gap-2 text-xs font-mono font-semibold text-[#8B9BB4]">
          <Terminal className="w-3.5 h-3.5 text-[#00E5FF]" />
          <span>REALTIME CONSOLE LOGS</span>
          <span className="text-[10px] opacity-60">({logs.length} events)</span>
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={handleCopyLogs}
            title="Copy logs to clipboard"
            className="p-1 rounded hover:bg-white/10 text-[#8B9BB4] hover:text-[#F0F4F8] transition-colors"
          >
            <Copy className="w-3 h-3" />
          </button>
          <button
            onClick={clearLogs}
            title="Clear logs"
            className="p-1 rounded hover:bg-white/10 text-[#8B9BB4] hover:text-[#EF4444] transition-colors"
          >
            <Trash2 className="w-3 h-3" />
          </button>
          <button
            onClick={() => setIsLogOpen(false)}
            title="Close log drawer"
            className="p-1 rounded hover:bg-white/10 text-[#8B9BB4] hover:text-[#F0F4F8] transition-colors"
          >
            <X className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Log Output Stream */}
      <div
        ref={logContainerRef}
        className="flex-1 overflow-y-auto p-2 font-mono text-[11px] space-y-1 select-text"
      >
        {logs.length === 0 ? (
          <div className="text-[#53637D] italic">No active logs. Operations will log here.</div>
        ) : (
          logs.map((log) => (
            <div key={log.id} className="flex items-start gap-2 leading-tight">
              <span className="text-[#53637D] shrink-0">[{log.timestamp}]</span>
              <span className={`shrink-0 uppercase font-semibold text-[10px] ${getLevelStyle(log.level)}`}>
                [{log.level}]
              </span>
              <span className="text-[#F0F4F8] break-all">{log.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};

