import React from "react";
import {
  PackageOpen,
  Layers,
  Repeat,
  KeyRound,
  Settings,
  Terminal,
} from "lucide-react";
import { useAppStore, TabType } from "../store/useAppStore";

export const Sidebar: React.FC = () => {
  const { activeTab, setActiveTab, logs, isLogOpen, setIsLogOpen } = useAppStore();

  const navItems: { id: TabType; label: string; icon: React.FC<{ className?: string }> }[] = [
    { id: "extract", label: "Extract Game", icon: PackageOpen },
    { id: "batch", label: "Batch Queue", icon: Layers },
    { id: "convert", label: "XCI ➔ NSP", icon: Repeat },
    { id: "keys", label: "Keys Manager", icon: KeyRound },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  const errorCount = logs.filter((l) => l.level === "error").length;

  return (
    <aside
      className="w-48 h-full flex flex-col justify-between py-3 px-2 border-r select-none transition-colors"
      style={{
        backgroundColor: "var(--bg-sidebar)",
        borderColor: "var(--border-subtle)",
      }}
    >
      {/* Navigation tabs */}
      <nav className="flex flex-col gap-1">
        <div className="px-2 pb-2 text-[10px] uppercase font-bold tracking-widest text-[#53637D]">
          Console Menu
        </div>
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all ${
                isActive
                  ? "bg-[#FF3B53]/15 text-[#FF3B53] shadow-sm shadow-[#FF3B53]/10"
                  : "text-[#8B9BB4] hover:text-[#F0F4F8] hover:bg-white/5"
              }`}
              style={{
                fontFamily: "var(--font-display)",
                border: isActive
                  ? "1px solid rgba(255, 59, 83, 0.35)"
                  : "1px solid transparent",
              }}
            >
              <Icon
                className={`w-4 h-4 transition-transform ${
                  isActive ? "scale-110 text-[#FF3B53]" : ""
                }`}
              />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* Bottom Drawer Actions */}
      <div className="pt-2 border-t" style={{ borderColor: "var(--border-subtle)" }}>
        <button
          onClick={() => setIsLogOpen(!isLogOpen)}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-mono transition-all ${
            isLogOpen
              ? "bg-[#00E5FF]/15 text-[#00E5FF] border border-[#00E5FF]/30"
              : "text-[#8B9BB4] hover:text-[#F0F4F8] hover:bg-white/5 border border-transparent"
          }`}
        >
          <div className="flex items-center gap-2">
            <Terminal className="w-3.5 h-3.5" />
            <span className="text-[11px]">Console Log</span>
          </div>
          {errorCount > 0 ? (
            <span className="px-1.5 py-0.2 rounded-full text-[10px] font-bold bg-[#EF4444]/20 text-[#EF4444] border border-[#EF4444]/40">
              {errorCount}
            </span>
          ) : (
            <span className="text-[10px] opacity-60">{logs.length}</span>
          )}
        </button>
      </div>
    </aside>
  );
};

