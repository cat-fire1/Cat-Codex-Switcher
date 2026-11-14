import React from "react";
import {
  ArrowSwap16Regular,
  Delete16Regular,
  Dismiss16Regular,
} from "@fluentui/react-icons";
import { getT } from "../lib/i18n";

interface ConfirmModalProps {
  isOpen: boolean;
  type: "switch" | "remove";
  accountName?: string;
  language: string;
  busy?: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export const ConfirmModal: React.FC<ConfirmModalProps> = ({
  isOpen,
  type,
  accountName = "",
  language,
  busy = false,
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
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-sm fluent-dialog overflow-hidden flex flex-col select-none">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <h2 className="text-[13px] font-semibold text-white flex items-center gap-2">
            {type === "remove" ? (
              <Delete16Regular className="w-4 h-4 text-red-400" />
            ) : (
              <ArrowSwap16Regular className="w-4 h-4 text-[#ff7a45]" />
            )}
            {title}
          </h2>
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10 text-zinc-400 hover:text-white transition-colors"
          >
            <Dismiss16Regular className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4">
          <p className="text-[12px] text-zinc-300 leading-relaxed">{description}</p>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-2.5 bg-black/20 border-t border-white/5">
          <button
            onClick={onClose}
            className="fluent-btn-standard px-3 py-1"
          >
            {t.cancel}
          </button>
          <button
            disabled={busy}
            onClick={() => {
              onConfirm();
              onClose();
            }}
            className={`px-3 py-1 rounded text-xs font-semibold text-white transition-colors ${
              type === "remove"
                ? "bg-red-600 hover:bg-red-500"
                : "fluent-btn-accent"
            }`}
          >
            {t.confirm}
          </button>
        </div>
      </div>
    </div>
  );
};
