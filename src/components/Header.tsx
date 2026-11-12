import React from "react";
import {
  ArrowSync16Regular,
  FolderOpen16Regular,
  Settings16Regular,
} from "@fluentui/react-icons";
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
    <header className="sticky top-0 z-30 flex items-center justify-between px-4 py-2.5 bg-[#202020] border-b border-[rgba(255,255,255,0.06)] select-none">
      {/* App brand */}
      <div className="flex items-center gap-2.5">
        <div className="w-7 h-7 rounded-md overflow-hidden bg-white/5 border border-white/10 flex items-center justify-center p-0.5 shadow-sm">
          <img
            src="/assets/account-switcher-logo.png"
            alt="Cat Codex Switcher"
            className="w-full h-full object-contain"
            onError={(e) => {
              (e.target as HTMLElement).style.display = "none";
            }}
          />
        </div>
        <div className="flex items-baseline gap-2">
          <span className="text-[13px] font-semibold tracking-tight text-white font-sans">
            {t.appTitle}
          </span>
          <span className="text-[10px] text-zinc-400 font-normal">
            Codex Manager
          </span>
        </div>
      </div>

      {/* Fluent Command Buttons */}
      <div className="flex items-center gap-1">
        <button
          onClick={onRefresh}
          disabled={refreshing}
          title={t.refresh}
          className="w-8 h-8 flex items-center justify-center rounded hover:bg-white/5 active:bg-white/10 text-zinc-300 hover:text-white transition-colors disabled:opacity-50"
        >
          <ArrowSync16Regular
            className={`w-4 h-4 ${refreshing ? "animate-spin text-[#ff7a45]" : ""}`}
          />
        </button>

        <button
          onClick={onOpenImport}
          title={t.importData}
          className="w-8 h-8 flex items-center justify-center rounded hover:bg-white/5 active:bg-white/10 text-zinc-300 hover:text-white transition-colors"
        >
          <FolderOpen16Regular className="w-4 h-4" />
        </button>

        <button
          onClick={onOpenSettings}
          title={t.settings}
          className="w-8 h-8 flex items-center justify-center rounded hover:bg-white/5 active:bg-white/10 text-zinc-300 hover:text-white transition-colors"
        >
          <Settings16Regular className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
