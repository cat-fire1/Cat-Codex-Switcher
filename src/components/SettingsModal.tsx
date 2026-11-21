import React, { useState } from "react";
import {
  Settings16Regular,
  Globe16Regular,
  ArrowSync16Regular,
  Open16Regular,
  Dismiss16Regular,
} from "@fluentui/react-icons";
import { Settings } from "../types";
import { getT, Language } from "../lib/i18n";

interface SettingsModalProps {
  isOpen: boolean;
  settings: Settings;
  updateStatus: string;
  updateUrl: string | null;
  onClose: () => void;
  onSave: (settings: Settings) => void;
  onCheckUpdates: () => void;
  onOpenUrl: (url: string) => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = (props) =>
  props.isOpen ? <SettingsForm {...props} /> : null;

const SettingsForm: React.FC<SettingsModalProps> = ({
  isOpen,
  settings: initialSettings,
  updateStatus,
  updateUrl,
  onClose,
  onSave,
  onCheckUpdates,
  onOpenUrl,
}) => {
  const [settings, setSettings] = useState<Settings>({ ...initialSettings });
  const t = getT(settings.language);

  if (!isOpen) return null;

  const handleToggle = (key: keyof Settings) => {
    setSettings((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  };

  const handleSave = () => {
    onSave(settings);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-md fluent-dialog overflow-hidden flex flex-col max-h-[90vh] select-none">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <h2 className="text-[13px] font-semibold text-white flex items-center gap-2">
            <Settings16Regular className="w-4 h-4 text-[#ff7a45]" />
            {t.settings}
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
          {/* Language selector */}
          <div className="space-y-1">
            <label className="text-[11px] font-medium text-zinc-300 flex items-center gap-1.5">
              <Globe16Regular className="w-3.5 h-3.5 text-[#ff7a45]" />
              {t.languageLabel}
            </label>
            <select
              value={settings.language}
              onChange={(e) =>
                setSettings({ ...settings, language: e.target.value as Language })
              }
              className="w-full px-2.5 py-1.5 rounded bg-black/30 border border-white/10 text-white text-xs focus:outline-none focus:border-[#ff7a45]"
            >
              <option value="uk" className="bg-[#2b2b2b]">Українська</option>
              <option value="en" className="bg-[#2b2b2b]">English</option>
              <option value="ru" className="bg-[#2b2b2b]">Русский</option>
            </select>
          </div>

          <div className="h-[1px] bg-white/5 my-1" />

          {/* Fluent Toggle Switches */}
          <div className="space-y-2">
            {[
              {
                id: "showsMenuBarPercentage",
                label: t.showTrayPercentage,
                value: settings.showsMenuBarPercentage,
              },
              {
                id: "showsFiveHourUsage",
                label: t.showFiveHourUsage,
                value: settings.showsFiveHourUsage,
              },
              {
                id: "launchAtLogin",
                label: t.launchAtLogin,
                value: settings.launchAtLogin,
              },
              {
                id: "automaticallyCheckUpdates",
                label: t.autoUpdates,
                value: settings.automaticallyCheckUpdates,
              },
              {
                id: "resetExpiryExperimentEnabled",
                label: t.experimentEnabled,
                value: settings.resetExpiryExperimentEnabled,
              },
            ].map((item) => (
              <label
                key={item.id}
                className="flex items-center justify-between p-2 rounded hover:bg-white/5 transition-colors cursor-pointer select-none"
              >
                <span className="text-xs text-zinc-200">{item.label}</span>
                <input
                  type="checkbox"
                  checked={Boolean(item.value)}
                  onChange={() => handleToggle(item.id as keyof Settings)}
                  className="w-4 h-4 rounded text-[#ff7a45] bg-black/40 border-white/20 focus:ring-0 focus:ring-offset-0 cursor-pointer accent-[#ff7a45]"
                />
              </label>
            ))}
          </div>

          <div className="h-[1px] bg-white/5 my-1" />

          {/* Update Repository */}
          <div className="space-y-1.5">
            <label className="text-[11px] font-medium text-zinc-300">
              {t.updateRepo}
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="owner/repository"
                value={settings.updateRepository}
                onChange={(e) =>
                  setSettings({ ...settings, updateRepository: e.target.value })
                }
                className="flex-1 px-2.5 py-1.5 rounded bg-black/30 border border-white/10 text-white text-xs font-mono focus:outline-none focus:border-[#ff7a45]"
              />
              <button
                type="button"
                onClick={onCheckUpdates}
                className="fluent-btn-standard px-2.5 py-1.5 flex items-center gap-1.5"
              >
                <ArrowSync16Regular className="w-3.5 h-3.5" />
                <span>{t.checkUpdatesNow}</span>
              </button>
            </div>

            {updateStatus && (
              <p className="text-[11px] text-zinc-400 mt-1">{updateStatus}</p>
            )}

            {updateUrl && (
              <button
                onClick={() => onOpenUrl(updateUrl)}
                className="inline-flex items-center gap-1 text-xs text-[#ff7a45] hover:underline font-medium mt-1"
              >
                <Open16Regular className="w-3.5 h-3.5" />
                {t.downloadUpdate}
              </button>
            )}
          </div>
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
            onClick={handleSave}
            className="fluent-btn-accent px-4 py-1"
          >
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
};
