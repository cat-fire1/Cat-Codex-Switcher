import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SettingsModal } from "./SettingsModal";
import type { Settings } from "../types";

const settings: Settings = {
  formatVersion: 1, language: "en", showsMenuBarPercentage: true,
  showsFiveHourUsage: false, statusBarUsageWindow: "fiveHour",
  resetExpiryExperimentEnabled: false, launchAtLogin: false,
  automaticallyCheckUpdates: false, updateRepository: "",
};

describe("settings draft", () => {
  it("loads current settings when opened after the initial snapshot", () => {
    const onSave = vi.fn();
    const props = { settings, updateStatus: "", updateUrl: null, onClose: vi.fn(),
      onSave, onCheckUpdates: vi.fn(), onOpenUrl: vi.fn() };
    const view = render(<SettingsModal {...props} isOpen={false} />);
    view.rerender(<SettingsModal {...props} settings={{ ...settings, updateRepository: "owner/current" }} isOpen />);
    expect((screen.getByPlaceholderText("owner/repository") as HTMLInputElement).value).toBe("owner/current");
    fireEvent.click(screen.getByText("Save"));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ updateRepository: "owner/current" }));
  });

  it("discards unsaved edits on close and reopen", () => {
    const props = { settings, updateStatus: "", updateUrl: null, onClose: vi.fn(),
      onSave: vi.fn(), onCheckUpdates: vi.fn(), onOpenUrl: vi.fn() };
    const view = render(<SettingsModal {...props} isOpen />);
    fireEvent.change(screen.getByPlaceholderText("owner/repository"), { target: { value: "unsaved/draft" } });
    view.rerender(<SettingsModal {...props} isOpen={false} />);
    view.rerender(<SettingsModal {...props} isOpen />);
    expect((screen.getByPlaceholderText("owner/repository") as HTMLInputElement).value).toBe("");
  });
});
