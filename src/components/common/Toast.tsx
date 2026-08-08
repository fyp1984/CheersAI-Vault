import { useEffect } from "react";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";

export interface ToastProps {
  message: string;
  type?: "success" | "error" | "info" | "warning";
  onClose: () => void;
  duration?: number;
}

export default function Toast({ message, type = "success", onClose, duration = 3000 }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onClose();
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onClose]);

  const palette = {
    success: {
      panel: "border-emerald-200 bg-emerald-50 text-emerald-900",
      icon: "text-emerald-600",
      Icon: CheckCircle2,
    },
    error: {
      panel: "border-red-200 bg-red-50 text-red-900",
      icon: "text-red-600",
      Icon: XCircle,
    },
    warning: {
      panel: "border-amber-200 bg-amber-50 text-amber-900",
      icon: "text-amber-600",
      Icon: AlertTriangle,
    },
    info: {
      panel: "border-blue-200 bg-blue-50 text-blue-900",
      icon: "text-blue-600",
      Icon: Info,
    },
  }[type];
  const Icon = palette.Icon;

  return (
    <div className="fixed right-4 top-4 z-50 animate-in slide-in-from-top-3 fade-in duration-300">
      <div
        className={`flex min-w-[320px] max-w-md items-start gap-3 rounded-2xl border px-4 py-3 shadow-lg shadow-slate-900/10 ${palette.panel}`}
        role="status"
        aria-live="polite"
      >
        <div className={`mt-0.5 shrink-0 ${palette.icon}`}>
          <Icon className="h-5 w-5" />
        </div>
        <p className="flex-1 text-sm font-medium leading-6">{message}</p>
        <button
          onClick={onClose}
          className="shrink-0 rounded-md p-1 text-slate-400 transition-colors hover:bg-white/70 hover:text-slate-600"
          aria-label="关闭提示"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
