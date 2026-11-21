import React, { useState } from "react";
import {
  FolderOpen16Regular,
  Folder16Regular,
  Eye16Regular,
  CheckmarkCircle16Regular,
  Warning16Regular,
  Dismiss16Regular,
} from "@fluentui/react-icons";
import { ImportPreview } from "../types";
import { getT } from "../lib/i18n";

interface ImportModalProps {
  isOpen: boolean;
  language: string;
  preview: ImportPreview | null;
  busy?: boolean;
  onClose: () => void;
  onSelectFolder: () => Promise<string | null>;
  onPreview: (path: string) => void;
  onConfirm: (path: string) => void;
}

export const ImportModal: React.FC<ImportModalProps> = ({
  isOpen,
  language,
  preview: suppliedPreview,
  busy = false,
  onClose,
  onSelectFolder,
  onPreview,
  onConfirm,
}) => {
  const [path, setPath] = useState("");
  const preview = suppliedPreview?.source === path.trim() ? suppliedPreview : null;
  const t = getT(language);

  if (!isOpen) return null;

  const handleBrowse = async () => {
    const selected = await onSelectFolder();
    if (selected) {
      setPath(selected);
      onPreview(selected);
    }
  };

  const handlePreview = () => {
    if (path.trim()) {
      onPreview(path.trim());
    }
  };

  const handleConfirm = () => {
    if (path.trim()) {
      onConfirm(path.trim());
      onClose();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-md fluent-dialog overflow-hidden flex flex-col max-h-[90vh] select-none">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <h2 className="text-[13px] font-semibold text-white flex items-center gap-2">
            <FolderOpen16Regular className="w-4 h-4 text-[#ff7a45]" />
            {t.importData}
          </h2>
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center rounded hover:bg-white/10 text-zinc-400 hover:text-white transition-colors"
          >
            <Dismiss16Regular className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-3.5 overflow-y-auto">
          <p className="text-xs text-zinc-300 leading-relaxed">
            {t.importTitle}
          </p>

          <div className="space-y-1">
            <label className="text-[11px] font-medium text-zinc-300">
              {t.importPathLabel}
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                disabled={busy}
                value={path}
                onChange={(e) => setPath(e.target.value)}
                className="flex-1 px-2.5 py-1.5 rounded bg-black/30 border border-white/10 text-white text-xs font-mono focus:outline-none focus:border-[#ff7a45]"
              />
              <button
                type="button"
                onClick={handleBrowse}
                disabled={busy}
                className="fluent-btn-standard px-2.5 py-1.5"
              >
                {t.browse}
              </button>
            </div>
          </div>

          <div className="flex justify-end">
            <button
              type="button"
              onClick={handlePreview}
              disabled={busy || !path.trim()}
              className="fluent-btn-standard px-2.5 py-1.5 flex items-center gap-1.5"
            >
              <Eye16Regular className="w-3.5 h-3.5" />
              <span>{t.previewImport}</span>
            </button>
          </div>

          {/* Preview Results */}
          {preview && (
            <div className="p-3 rounded bg-black/30 border border-white/10 space-y-2">
              <div className="flex items-center gap-2 text-xs font-medium text-white">
                <CheckmarkCircle16Regular className="w-4 h-4 text-[#6ccb5f]" />
                {t.importPreviewCount
                  .replace("{count}", String(preview.titles.length))
                  .replace("{duplicates}", String(preview.duplicates))}
              </div>

              {preview.titles.length > 0 && (
                <ul className="text-xs text-zinc-300 space-y-1 max-h-32 overflow-y-auto pr-1">
                  {preview.titles.map((name, idx) => (
                    <li
                      key={idx}
                      className="px-2 py-0.5 rounded bg-white/5 font-mono text-[11px]"
                    >
                      {name}
                    </li>
                  ))}
                </ul>
              )}

              {preview.duplicates > 0 && (
                <p className="flex items-center gap-1 text-[11px] text-amber-400">
                  <Warning16Regular className="w-3.5 h-3.5" />
                  Дублікати будуть пропущені
                </p>
              )}
            </div>
          )}
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
            disabled={busy || !preview || preview.titles.length === 0}
            onClick={handleConfirm}
            className="fluent-btn-accent px-4 py-1 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t.confirmImport}
          </button>
        </div>
      </div>
    </div>
  );
};
