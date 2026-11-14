import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ImportModal } from "./ImportModal";

it("requires a preview matching the current path", async () => {
  const props = { isOpen: true, language: "en", onClose: vi.fn(),
    onSelectFolder: vi.fn().mockResolvedValue("C:\\Test"), onPreview: vi.fn(), onConfirm: vi.fn() };
  const preview = { source: "C:\\Test", titles: ["Test account"], duplicates: 0 };
  const view = render(<ImportModal {...props} preview={null} />);
  fireEvent.click(screen.getByText("Browse..."));
  await waitFor(() => expect(props.onPreview).toHaveBeenCalledWith("C:\\Test"));
  view.rerender(<ImportModal {...props} preview={preview} />);
  expect((screen.getByText("Import Selected") as HTMLButtonElement).disabled).toBe(false);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "C:\\Other" } });
  expect((screen.getByText("Import Selected") as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryByText("Test account")).toBeNull();
});

it("blocks import interactions while the worker is busy", () => {
  render(<ImportModal isOpen language="en" preview={null} busy onClose={vi.fn()}
    onSelectFolder={vi.fn()} onPreview={vi.fn()} onConfirm={vi.fn()} />);
  for (const name of ["Browse...", "Preview", "Import Selected"]) {
    expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
  }
});
