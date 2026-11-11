import React from "react";
import { RotateCw, Settings, FolderInput, Cat } from "lucide-react";
import { getT } from "../lib/i18n";

interface HeaderProps {
  language: string;
  refreshing: boolean;
  onRefresh: () => void;
  onOpenSettings: () => void;
  onOpenImport: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  language,
  refreshing,
  onRefresh,
  onOpenSettings,
  onOpenImport,
}) => {
  const t = getT(language);

  return (
    <header className="sticky top-0 z-30 flex items-center justify-between px-5 py-3.5 glass-panel border-b border-white/5">
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-amber-500/30 to-orange-500/20 p-[1px] shadow-lg shadow-orange-500/10 border border-white/10 overflow-hidden flex items-center justify-center bg-[#151821]">
          <img
            src="/assets/account-switcher-logo.png"
            alt="Cat Codex Switcher"
            className="w-full h-full object-cover rounded-[11px]"
            onError={(e) => {
              // Fallback if image path has issue
              (e.target as HTMLElement).style.display = "none";
            }}
          />
        </div>
        <div>
          <h1 className="text-sm font-bold tracking-tight text-white flex items-center gap-2">
            {t.appTitle}
            <span className="text-[10px] uppercase font-semibold tracking-wider px-1.5 py-0.5 rounded-full bg-orange-500/10 text-orange-400 border border-orange-500/20">
              v0.2
            </span>
          </h1>
          <p className="text-[11px] text-zinc-400">Codex Account Manager</p>
        </div>
      </div>

      <div className="flex items-center gap-1.5">
        <button
          onClick={onRefresh}
          disabled={refreshing}
          title={t.refresh}
          className="p-2 rounded-lg text-zinc-400 hover:text-white hover:bg-white/5 transition active:scale-95 disabled:opacity-50"
        >
          <RotateCw className={`w-4 h-4 ${refreshing ? "animate-spin text-orange-400" : ""}`} />
        </button>

        <button
          onClick={onOpenImport}
          title={t.importData}
          className="p-2 rounded-lg text-zinc-400 hover:text-white hover:bg-white/5 transition active:scale-95"
        >
          <FolderInput className="w-4 h-4" />
        </button>

        <button
          onClick={onOpenSettings}
          title={t.settings}
          className="p-2 rounded-lg text-zinc-400 hover:text-white hover:bg-white/5 transition active:scale-95"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
