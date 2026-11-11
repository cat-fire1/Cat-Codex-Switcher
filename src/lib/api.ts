import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Snapshot, Settings } from "../types";

// Check if running in Tauri environment
export const isTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

// Fallback mock data for browser preview/development
const mockSnapshot: Snapshot = {
  rows: [
    {
      profile: {
        id: "a1111111-1111-1111-1111-111111111111",
        displayName: "Cat Main",
        email: "cat.work@openai.com",
        accountID: "org-catmain01",
        userID: "user-123456",
        createdAt: new Date().toISOString(),
        lastUsedAt: new Date().toISOString(),
      },
      title: "cat.work@openai.com",
      usage: {
        remainingPercent: 84,
        resetsAt: new Date(Date.now() + 3 * 86400000).toISOString(),
        fiveHourRemainingPercent: 92,
        fiveHourResetsAt: new Date(Date.now() + 140 * 60000).toISOString(),
        resetCreditsRemaining: 2,
        resetCredits: null,
      },
      stale: false,
      fetchedAt: new Date().toISOString(),
      error: null,
      subscription: [
        new Date(Date.now() - 30 * 86400000).toISOString(),
        new Date(Date.now() + 30 * 86400000).toISOString(),
      ],
    },
    {
      profile: {
        id: "b2222222-2222-2222-2222-222222222222",
        displayName: "Personal Pro",
        email: "cat.personal@gmail.com",
        accountID: "org-personal99",
        userID: "user-987654",
        createdAt: new Date().toISOString(),
        lastUsedAt: null,
      },
      title: "cat.personal@gmail.com",
      usage: {
        remainingPercent: 35,
        resetsAt: new Date(Date.now() + 5 * 86400000).toISOString(),
        fiveHourRemainingPercent: 18,
        fiveHourResetsAt: new Date(Date.now() + 45 * 60000).toISOString(),
        resetCreditsRemaining: 0,
        resetCredits: null,
      },
      stale: false,
      fetchedAt: new Date().toISOString(),
      error: null,
      subscription: null,
    },
  ],
  active: "a1111111-1111-1111-1111-111111111111",
  settings: {
    formatVersion: 1,
    language: "uk",
    showsMenuBarPercentage: true,
    showsFiveHourUsage: true,
    statusBarUsageWindow: "fiveHour",
    resetExpiryExperimentEnabled: false,
    launchAtLogin: false,
    automaticallyCheckUpdates: true,
    updateRepository: "cat-fire1/Cat-Codex-Switcher",
  },
  busy: false,
  adding: false,
  refreshing: false,
  error: null,
  preview: null,
  updateUrl: null,
  updateStatus: "",
};

let currentMock = { ...mockSnapshot };

export const api = {
  async getSnapshot(): Promise<Snapshot> {
    if (!isTauri()) return currentMock;
    return await invoke<Snapshot>("get_snapshot");
  },

  async refresh(): Promise<void> {
    if (!isTauri()) {
      currentMock = { ...currentMock, refreshing: true };
      setTimeout(() => {
        currentMock = { ...currentMock, refreshing: false };
      }, 800);
      return;
    }
    await invoke("refresh");
  },

  async addAccount(): Promise<void> {
    if (!isTauri()) {
      alert("Browser login is only available when running in Tauri desktop app.");
      return;
    }
    await invoke("add_account");
  },

  async registerCurrent(): Promise<void> {
    if (!isTauri()) {
      alert("Register current login is only available in Tauri desktop app.");
      return;
    }
    await invoke("register_current");
  },

  async switchAccount(id: string): Promise<void> {
    if (!isTauri()) {
      currentMock = { ...currentMock, active: id };
      return;
    }
    await invoke("switch_account", { id });
  },

  async removeAccount(id: string): Promise<void> {
    if (!isTauri()) {
      currentMock = {
        ...currentMock,
        rows: currentMock.rows.filter((r) => r.profile.id !== id),
      };
      return;
    }
    await invoke("remove_account", { id });
  },

  async saveSettings(settings: Settings): Promise<void> {
    if (!isTauri()) {
      currentMock = { ...currentMock, settings };
      return;
    }
    await invoke("save_settings", { settings });
  },

  async previewImport(path: string): Promise<void> {
    if (!isTauri()) return;
    await invoke("preview_import", { path });
  },

  async confirmImport(path: string): Promise<void> {
    if (!isTauri()) return;
    await invoke("confirm_import", { path });
  },

  async checkUpdates(): Promise<void> {
    if (!isTauri()) return;
    await invoke("check_updates");
  },

  async dismissError(): Promise<void> {
    if (!isTauri()) {
      currentMock = { ...currentMock, error: null };
      return;
    }
    await invoke("dismiss_error");
  },

  async selectFolder(): Promise<string | null> {
    if (!isTauri()) {
      return "C:\\Users\\cat\\AppData\\Local\\Codex Account Switcher";
    }
    return await invoke<string | null>("select_folder");
  },

  async openUrl(url: string): Promise<void> {
    if (!isTauri()) {
      window.open(url, "_blank");
      return;
    }
    await invoke("open_external_url", { url });
  },

  onStateUpdate(callback: () => void): () => void {
    if (!isTauri()) return () => {};
    const unlistenPromise = listen("state-update", () => {
      callback();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  },
};
