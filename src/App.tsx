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
import { Plus, UserCheck, Loader2 } from "lucide-react";

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
      <div className="flex h-screen w-screen items-center justify-center bg-[#0b0d13]">
        <Loader2 className="w-8 h-8 text-orange-500 animate-spin" />
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
    <div className="flex flex-col h-screen w-screen bg-[#0b0d13] text-zinc-100 select-none overflow-hidden">
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

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
        {/* Action Controls Bar */}
        <div className="flex items-center justify-between gap-3 bg-[#13161f] p-3 rounded-2xl border border-white/5 shadow-sm">
          <div className="flex items-center gap-2">
            <button
              onClick={handleAddAccount}
              disabled={Boolean(snapshot?.busy)}
              className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl text-xs font-semibold bg-gradient-to-r from-orange-500 to-amber-600 hover:from-orange-600 hover:to-amber-700 text-white shadow-lg shadow-orange-500/20 transition active:scale-95 disabled:opacity-50"
            >
              <Plus className="w-4 h-4" />
              {t.addAccount}
            </button>

            <button
              onClick={handleRegisterCurrent}
              disabled={Boolean(snapshot?.busy)}
              className="flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium bg-white/5 hover:bg-white/10 text-zinc-200 border border-white/10 transition active:scale-95 disabled:opacity-50"
            >
              <UserCheck className="w-4 h-4 text-zinc-400" />
              {t.registerCurrent}
            </button>
          </div>

          {snapshot?.adding && (
            <div className="flex items-center gap-2 text-xs text-orange-400 font-medium animate-pulse">
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              Очікування входу в браузері...
            </div>
          )}
        </div>

        {/* Accounts List */}
        {sortedRows.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center px-4">
            <div className="w-16 h-16 rounded-3xl bg-zinc-900 border border-white/5 flex items-center justify-center text-zinc-500 mb-4 shadow-inner">
              <UserCheck className="w-8 h-8" />
            </div>
            <p className="text-xs text-zinc-400 max-w-xs leading-relaxed">
              {t.noAccounts}
            </p>
          </div>
        ) : (
          <div className="space-y-3 pb-4">
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
