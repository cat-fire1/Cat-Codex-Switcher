import React, { useState } from "react";
import { X, Folder, Eye, CheckCircle2, AlertTriangle } from "lucide-react";
import { ImportPreview } from "../types";
import { getT } from "../lib/i18n";

interface ImportModalProps {
  isOpen: boolean;
  language: string;
  preview: ImportPreview | null;
  onClose: () => void;
  onSelectFolder: () => Promise<string | null>;
  onPreview: (path: string) => void;
  onConfirm: (path: string) => void;
}

export const ImportModal: React.FC<ImportModalProps> = ({
  isOpen,
  language,
  preview,
  onClose,
  onSelectFolder,
  onPreview,
  onConfirm,
}) => {
  const [path, setPath] = useState(
    "C:\\Users\\cat\\AppData\\Local\\Codex Account Switcher"
  );
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
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="w-full max-w-md rounded-2xl glass-modal shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-white/10">
          <h2 className="text-base font-bold text-white flex items-center gap-2">
            <Folder className="w-4 h-4 text-orange-400" />
            {t.importData}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-zinc-400 hover:text-white hover:bg-white/5 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5 space-y-4 overflow-y-auto">
          <p className="text-xs text-zinc-400 leading-relaxed">
            {t.importTitle}
          </p>

          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-zinc-300">
              {t.importPathLabel}
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                className="flex-1 px-3 py-1.5 rounded-xl bg-zinc-900 border border-white/10 text-white text-xs font-mono focus:outline-none focus:border-orange-500"
              />
              <button
                type="button"
                onClick={handleBrowse}
                className="px-3 py-1.5 rounded-xl bg-white/5 hover:bg-white/10 text-zinc-200 text-xs font-medium border border-white/10 transition active:scale-95"
              >
                {t.browse}
              </button>
            </div>
          </div>

          <div className="flex justify-end">
            <button
              type="button"
              onClick={handlePreview}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-white/5 hover:bg-white/10 text-zinc-200 text-xs font-medium border border-white/10 transition active:scale-95"
            >
              <Eye className="w-3.5 h-3.5" />
              {t.previewImport}
            </button>
          </div>

          {/* Preview Results */}
          {preview && (
            <div className="p-3.5 rounded-xl bg-zinc-900/80 border border-white/10 space-y-2">
              <div className="flex items-center gap-2 text-xs font-semibold text-white">
                <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                {t.importPreviewCount
                  .replace("{count}", String(preview.titles.length))
                  .replace("{duplicates}", String(preview.duplicates))}
              </div>

              {preview.titles.length > 0 && (
                <ul className="text-xs text-zinc-300 space-y-1 max-h-32 overflow-y-auto pr-1">
                  {preview.titles.map((name, idx) => (
                    <li
                      key={idx}
                      className="px-2 py-1 rounded bg-white/5 font-mono text-[11px]"
                    >
                      {name}
                    </li>
                  ))}
                </ul>
              )}

              {preview.duplicates > 0 && (
                <p className="flex items-center gap-1 text-[11px] text-amber-400">
                  <AlertTriangle className="w-3.5 h-3.5" />
                  Дублікати будуть пропущені
                </p>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-white/10 bg-black/20">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-xs font-medium text-zinc-400 hover:text-white hover:bg-white/5 transition"
          >
            {t.cancel}
          </button>
          <button
            disabled={!preview || preview.titles.length === 0}
            onClick={handleConfirm}
            className="px-4 py-2 rounded-xl text-xs font-medium bg-gradient-to-r from-orange-500 to-amber-600 hover:from-orange-600 hover:to-amber-700 text-white shadow-lg shadow-orange-500/20 transition active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t.confirmImport}
          </button>
        </div>
      </div>
    </div>
  );
};
