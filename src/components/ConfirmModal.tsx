import React from "react";
import { AlertCircle, X } from "lucide-react";
import { getT } from "../lib/i18n";

interface ConfirmModalProps {
  isOpen: boolean;
  type: "switch" | "remove";
  accountName?: string;
  language: string;
  onClose: () => void;
  onConfirm: () => void;
}

export const ConfirmModal: React.FC<ConfirmModalProps> = ({
  isOpen,
  type,
  accountName = "",
  language,
  onClose,
  onConfirm,
}) => {
  const t = getT(language);

  if (!isOpen) return null;

  const title =
    type === "switch" ? t.switchConfirmTitle : t.removeConfirmTitle;
  const description =
    type === "switch"
      ? t.switchConfirmDesc
      : t.removeConfirmDesc.replace("{name}", accountName);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="w-full max-w-sm rounded-2xl glass-modal shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-white/10">
          <h2 className="text-sm font-bold text-white flex items-center gap-2">
            <AlertCircle
              className={`w-4 h-4 ${
                type === "remove" ? "text-rose-400" : "text-orange-400"
              }`}
            />
            {title}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-zinc-400 hover:text-white hover:bg-white/5 transition"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5">
          <p className="text-xs text-zinc-300 leading-relaxed">{description}</p>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-white/10 bg-black/20">
          <button
            onClick={onClose}
            className="px-3.5 py-1.5 rounded-xl text-xs font-medium text-zinc-400 hover:text-white hover:bg-white/5 transition"
          >
            {t.cancel}
          </button>
          <button
            onClick={() => {
              onConfirm();
              onClose();
            }}
            className={`px-3.5 py-1.5 rounded-xl text-xs font-semibold text-white shadow-lg transition active:scale-95 ${
              type === "remove"
                ? "bg-rose-600 hover:bg-rose-700 shadow-rose-600/20"
                : "bg-gradient-to-r from-orange-500 to-amber-600 hover:from-orange-600 shadow-orange-500/20"
            }`}
          >
            {t.confirm}
          </button>
        </div>
      </div>
    </div>
  );
};
