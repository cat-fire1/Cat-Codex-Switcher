import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import type { Snapshot } from "./types";

vi.mock("./lib/api", () => ({ api: {
  getSnapshot: vi.fn(), onStateUpdate: vi.fn(() => () => {}),
  refresh: vi.fn(), addAccount: vi.fn(), registerCurrent: vi.fn(),
  switchAccount: vi.fn(), removeAccount: vi.fn(), saveSettings: vi.fn(),
  dismissError: vi.fn(), cancelLogin: vi.fn(),
} }));

const snapshot: Snapshot = {
  rows: [{ profile: { id: "profile", displayName: "Test", email: null, accountID: "fake",
    createdAt: "2026-01-01T00:00:00Z", lastUsedAt: null }, title: "Test account",
    usage: null, stale: false, fetchedAt: null, error: null, subscription: null }],
  active: null, busy: true, adding: false, refreshing: false, error: null, preview: null,
  updateUrl: null, updateStatus: "", settings: {
    formatVersion: 1, language: "en", showsMenuBarPercentage: true, showsFiveHourUsage: false,
    statusBarUsageWindow: "fiveHour", resetExpiryExperimentEnabled: false,
    launchAtLogin: false, automaticallyCheckUpdates: false, updateRepository: "",
  },
};

beforeEach(() => { vi.clearAllMocks(); vi.mocked(api.getSnapshot).mockResolvedValue(snapshot); });

describe("worker integration", () => {
  it("disables account actions while the worker is busy", async () => {
    render(<App />);
    const button = await screen.findByRole("button", { name: "Switch" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(screen.queryByText("Switch Account")).toBeNull();
    expect((screen.getByTitle("Remove") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTitle("Settings") as HTMLButtonElement).disabled).toBe(true);
  });

  it("cancels browser login using its button", async () => {
    vi.mocked(api.getSnapshot).mockResolvedValue({ ...snapshot, adding: true });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(api.cancelLogin).toHaveBeenCalledOnce());
  });

  it("cancels browser login with Escape", async () => {
    vi.mocked(api.getSnapshot).mockResolvedValue({ ...snapshot, adding: true });
    render(<App />);
    await screen.findByText("Waiting for browser login...");
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(api.cancelLogin).toHaveBeenCalledOnce());
  });

  it("shows IPC rejection instead of losing the error", async () => {
    vi.mocked(api.getSnapshot).mockResolvedValue({ ...snapshot, busy: false });
    vi.mocked(api.refresh).mockRejectedValueOnce("Background worker stopped");
    render(<App />);
    fireEvent.click(await screen.findByTitle("Refresh"));
    expect(await screen.findByText("Background worker stopped")).toBeTruthy();
  });
});
