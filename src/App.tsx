import React, { useEffect, useState } from "react";
import { Header } from "./components/Header";
import { AccountCard } from "./components/AccountCard";
import { SettingsModal } from "./components/SettingsModal";
import { ImportModal } from "./components/ImportModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { Banner } from "./components/Banner";
import { api } from "./lib/api";
import { getT } from "./lib/i18n";
import { Snapshot, Settings } from "./types";
import {
  PersonAdd16Regular,
  PersonAvailable16Regular,
  ArrowSync16Regular,
  Person16Regular,
} from "@fluentui/react-icons";

export const App: React.FC = () => {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [loading, setLoading] = useState(true);

  // Modals state
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isImportOpen, setIsImportOpen] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    type: "switch" | "remove";
    accountId: string;
    accountName: string;
  }>({
    isOpen: false,
    type: "switch",
    accountId: "",
    accountName: "",
  });

  const loadState = async () => {
    try {
      const data = await api.getSnapshot();
      setSnapshot(data);
    } catch (e) {
      console.error("Failed to load snapshot", e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadState();
    const unsubscribe = api.onStateUpdate(() => {
      loadState();
    });
    return () => {
      unsubscribe();
    };
  }, []);

  const language = snapshot?.settings.language || "uk";
  const t = getT(language);

  const handleRefresh = async () => {
    await api.refresh();
  };

  const handleAddAccount = async () => {
    await api.addAccount();
  };

  const handleRegisterCurrent = async () => {
    await api.registerCurrent();
  };

  const handleSwitchRequest = (id: string) => {
    const row = snapshot?.rows.find((r) => r.profile.id === id);
    setConfirmDialog({
      isOpen: true,
      type: "switch",
      accountId: id,
      accountName: row?.title || id,
    });
  };

  const handleRemoveRequest = (id: string) => {
    const row = snapshot?.rows.find((r) => r.profile.id === id);
    setConfirmDialog({
      isOpen: true,
      type: "remove",
      accountId: id,
      accountName: row?.title || id,
    });
  };

  const handleConfirmAction = async () => {
    if (confirmDialog.type === "switch") {
      await api.switchAccount(confirmDialog.accountId);
    } else {
      await api.removeAccount(confirmDialog.accountId);
    }
  };

  const handleSaveSettings = async (settings: Settings) => {
    await api.saveSettings(settings);
  };

  const handleDismissError = async () => {
    await api.dismissError();
  };

  if (loading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-[#202020]">
        <ArrowSync16Regular className="w-6 h-6 text-[#ff7a45] animate-spin" />
      </div>
    );
  }

  const rows = snapshot?.rows || [];
  const activeId = snapshot?.active;

  // Sort rows: active account first, then by title
  const sortedRows = [...rows].sort((a, b) => {
    if (a.profile.id === activeId) return -1;
    if (b.profile.id === activeId) return 1;
    return a.title.localeCompare(b.title);
  });

  return (
    <div className="flex flex-col h-screen w-screen bg-[#202020] text-white select-none overflow-hidden font-sans">
      {/* Top Header */}
      <Header
        language={language}
        refreshing={Boolean(snapshot?.refreshing)}
        onRefresh={handleRefresh}
        onOpenSettings={() => setIsSettingsOpen(true)}
        onOpenImport={() => setIsImportOpen(true)}
      />

      {/* Error or Warning Banner */}
      <Banner message={snapshot?.error || null} onDismiss={handleDismissError} />

      {/* Action CommandBar */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#252525] border-b border-[rgba(255,255,255,0.06)]">
        <div className="flex items-center gap-2">
          <button
            onClick={handleAddAccount}
            disabled={Boolean(snapshot?.busy)}
            className="fluent-btn-accent px-3 py-1.5 flex items-center gap-1.5 disabled:opacity-50"
          >
            <PersonAdd16Regular className="w-4 h-4" />
            <span>{t.addAccount}</span>
          </button>

          <button
            onClick={handleRegisterCurrent}
            disabled={Boolean(snapshot?.busy)}
            className="fluent-btn-standard px-3 py-1.5 flex items-center gap-1.5 disabled:opacity-50"
          >
            <PersonAvailable16Regular className="w-4 h-4 text-zinc-300" />
            <span>{t.registerCurrent}</span>
          </button>
        </div>

        {snapshot?.adding && (
          <div className="flex items-center gap-1.5 text-xs text-[#ff7a45] font-medium animate-pulse">
            <ArrowSync16Regular className="w-3.5 h-3.5 animate-spin" />
            <span>Очікування входу в браузері...</span>
          </div>
        )}
      </div>

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto px-4 py-3 space-y-2.5">
        {sortedRows.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center px-4">
            <div className="w-12 h-12 rounded-full bg-white/5 border border-white/10 flex items-center justify-center text-zinc-400 mb-3 shadow-inner">
              <Person16Regular className="w-6 h-6" />
            </div>
            <p className="text-xs text-zinc-400 max-w-xs leading-relaxed">
              {t.noAccounts}
            </p>
          </div>
        ) : (
          <div className="space-y-2.5 pb-2">
            {sortedRows.map((row) => (
              <AccountCard
                key={row.profile.id}
                row={row}
                isActive={row.profile.id === activeId}
                showFiveHour={Boolean(snapshot?.settings.showsFiveHourUsage)}
                language={language}
                onSwitch={handleSwitchRequest}
                onRemove={handleRemoveRequest}
              />
            ))}
          </div>
        )}
      </main>

      {/* Settings Modal */}
      {snapshot && (
        <SettingsModal
          isOpen={isSettingsOpen}
          settings={snapshot.settings}
          updateStatus={snapshot.updateStatus}
          updateUrl={snapshot.updateUrl}
          onClose={() => setIsSettingsOpen(false)}
          onSave={handleSaveSettings}
          onCheckUpdates={() => api.checkUpdates()}
          onOpenUrl={(url) => api.openUrl(url)}
        />
      )}

      {/* Import Modal */}
      <ImportModal
        isOpen={isImportOpen}
        language={language}
        preview={snapshot?.preview || null}
        onClose={() => setIsImportOpen(false)}
        onSelectFolder={() => api.selectFolder()}
        onPreview={(p) => api.previewImport(p)}
        onConfirm={(p) => api.confirmImport(p)}
      />

      {/* Confirm Modal */}
      <ConfirmModal
        isOpen={confirmDialog.isOpen}
        type={confirmDialog.type}
        accountName={confirmDialog.accountName}
        language={language}
        onClose={() =>
          setConfirmDialog((prev) => ({ ...prev, isOpen: false }))
        }
        onConfirm={handleConfirmAction}
      />
    </div>
  );
};
export default App;
