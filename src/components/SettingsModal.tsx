import React, { useState } from "react";
import { X, Check, Globe, RefreshCw, ExternalLink } from "lucide-react";
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

export const SettingsModal: React.FC<SettingsModalProps> = ({
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
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="w-full max-w-md rounded-2xl glass-modal shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-white/10">
          <h2 className="text-base font-bold text-white flex items-center gap-2">
            {t.settings}
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
          {/* Language selector */}
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-zinc-300 flex items-center gap-1.5">
              <Globe className="w-3.5 h-3.5 text-orange-400" />
              {t.languageLabel}
            </label>
            <select
              value={settings.language}
              onChange={(e) =>
                setSettings({ ...settings, language: e.target.value as Language })
              }
              className="w-full px-3 py-2 rounded-xl bg-zinc-900 border border-white/10 text-white text-sm focus:outline-none focus:border-orange-500"
            >
              <option value="uk">Українська</option>
              <option value="en">English</option>
              <option value="ru">Русский</option>
            </select>
          </div>

          <div className="h-[1px] bg-white/5 my-2" />

          {/* Toggles */}
          <div className="space-y-3">
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
                className="flex items-center justify-between p-2.5 rounded-xl hover:bg-white/5 transition cursor-pointer select-none"
              >
                <span className="text-xs font-medium text-zinc-300">{item.label}</span>
                <input
                  type="checkbox"
                  checked={Boolean(item.value)}
                  onChange={() => handleToggle(item.id as keyof Settings)}
                  className="w-4 h-4 rounded text-orange-500 bg-zinc-900 border-zinc-700 focus:ring-orange-500 focus:ring-offset-0"
                />
              </label>
            ))}
          </div>

          <div className="h-[1px] bg-white/5 my-2" />

          {/* Update Repository */}
          <div className="space-y-2">
            <label className="text-xs font-semibold text-zinc-300">
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
                className="flex-1 px-3 py-1.5 rounded-xl bg-zinc-900 border border-white/10 text-white text-xs font-mono focus:outline-none focus:border-orange-500"
              />
              <button
                type="button"
                onClick={onCheckUpdates}
                className="px-3 py-1.5 rounded-xl bg-white/5 hover:bg-white/10 text-zinc-200 text-xs font-medium border border-white/10 transition active:scale-95 flex items-center gap-1.5"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                {t.checkUpdatesNow}
              </button>
            </div>

            {updateStatus && (
              <p className="text-xs text-zinc-400 mt-1">{updateStatus}</p>
            )}

            {updateUrl && (
              <button
                onClick={() => onOpenUrl(updateUrl)}
                className="inline-flex items-center gap-1 text-xs text-orange-400 hover:text-orange-300 font-semibold mt-1"
              >
                <ExternalLink className="w-3.5 h-3.5" />
                {t.downloadUpdate}
              </button>
            )}
          </div>
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
            onClick={handleSave}
            className="px-4 py-2 rounded-xl text-xs font-medium bg-gradient-to-r from-orange-500 to-amber-600 hover:from-orange-600 hover:to-amber-700 text-white shadow-lg shadow-orange-500/20 transition active:scale-95"
          >
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
};
