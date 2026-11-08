import React from "react";
import { CheckCircle2, AlertCircle, AlertTriangle, Info, X } from "lucide-react";
import { useAppStore, ToastItem } from "../store/useAppStore";

export const ToastContainer: React.FC = () => {
  const { toasts, removeToast } = useAppStore();

  if (toasts.length === 0) return null;

  const getToastIcon = (type: ToastItem["type"]) => {
    switch (type) {
      case "success":
        return <CheckCircle2 className="w-4 h-4 text-[#10B981]" />;
      case "error":
        return <AlertCircle className="w-4 h-4 text-[#EF4444]" />;
      case "warning":
        return <AlertTriangle className="w-4 h-4 text-[#F59E0B]" />;
      default:
        return <Info className="w-4 h-4 text-[#00E5FF]" />;
    }
  };

  const getBorderColor = (type: ToastItem["type"]) => {
    switch (type) {
      case "success":
        return "border-[#10B981]/50";
      case "error":
        return "border-[#EF4444]/50";
      case "warning":
        return "border-[#F59E0B]/50";
      default:
        return "border-[#00E5FF]/50";
    }
  };

  return (
    <div className="fixed top-12 right-4 z-50 flex flex-col gap-2 max-w-sm pointer-events-none">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`pointer-events-auto flex items-center justify-between gap-3 px-3 py-2.5 rounded-lg border shadow-xl backdrop-blur-md animate-in slide-in-from-top-2 duration-200 transition-all ${getBorderColor(
            toast.type,
          )}`}
          style={{
            backgroundColor: "rgba(20, 27, 38, 0.95)",
            color: "var(--text-main)",
          }}
        >
          <div className="flex items-center gap-2 text-xs">
            {getToastIcon(toast.type)}
            <span>{toast.message}</span>
          </div>
          <button
            onClick={() => removeToast(toast.id)}
            className="text-[#8B9BB4] hover:text-white p-0.5 transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
};

