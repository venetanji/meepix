const native = vi.hoisted(() => ({
  enabled: false,
  handler: undefined as
    undefined | ((event: { preventDefault: () => void }) => void),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  isTauri: () => native.enabled,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: typeof native.handler) => {
      native.handler = handler;
      return () => {
        native.handler = undefined;
      };
    },
  }),
}));
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App, { type DocumentBackend, type Snapshot } from "../../src/App";
const snapshot: Snapshot = {
  document_id: "doc",
  revision: 0,
  width: 16,
  height: 16,
  layers: [{ id: "layer", name: "Layer 1", visible: true }],
  frames: [{ id: "frame", duration_ms: 100 }],
  active_layer_id: "layer",
  active_frame_id: "frame",
  palette: [],
  pixels: Array(1024).fill(0),
  can_undo: false,
  can_redo: false,
  dirty: false,
};
beforeEach(() => {
  vi.stubGlobal("PointerEvent", MouseEvent);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
    () =>
      ({
        createImageData: (w: number, h: number) => ({
          data: new Uint8ClampedArray(w * h * 4),
        }),
        putImageData: () => {},
      }) as unknown as CanvasRenderingContext2D,
  );
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
it("saves a new drawing under a fresh name after saving project A", async () => {
  const api: DocumentBackend = {
    snapshot: vi.fn().mockResolvedValue(snapshot),
    create: vi.fn().mockResolvedValue({ ...snapshot, document_id: "new" }),
    command: vi.fn().mockResolvedValue({
      ...snapshot,
      document_id: "new",
      revision: 1,
      dirty: true,
    }),
    save: vi.fn().mockResolvedValue(snapshot),
    load: vi.fn(),
    export: vi.fn(),
    models: vi.fn().mockResolvedValue({ configured: false, models: [] }),
    submit: vi.fn(),
    poll: vi.fn(),
    reject: vi.fn(),
    accept: vi.fn(),
    frameSnapshot: vi.fn(),
  };
  render(<App backend={api} />);
  const canvas = await screen.findByLabelText("Pixel canvas");
  fireEvent.change(screen.getByLabelText("Project name"), {
    target: { value: "A" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("A"));
  fireEvent.click(screen.getByRole("button", { name: "New" }));
  await waitFor(() => expect(api.create).toHaveBeenCalled());
  await waitFor(() =>
    expect(
      (screen.getByLabelText("Project name") as HTMLInputElement).value,
    ).not.toBe("A"),
  );
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 160,
    height: 160,
  } as DOMRect);
  fireEvent.pointerDown(canvas, { clientX: 1, clientY: 1, button: 0 });
  fireEvent.pointerUp(canvas, { clientX: 1, clientY: 1 });
  await screen.findByText("Unsaved changes");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledTimes(2));
  expect(vi.mocked(api.save).mock.calls[1][0]).not.toBe("A");
});
it("prevents browser unload when the document is dirty", async () => {
  const api: DocumentBackend = {
    snapshot: vi.fn().mockResolvedValue({ ...snapshot, dirty: true }),
    create: vi.fn(),
    command: vi.fn(),
    save: vi.fn(),
    load: vi.fn(),
    export: vi.fn(),
    models: vi.fn().mockResolvedValue({ configured: false, models: [] }),
    submit: vi.fn(),
    poll: vi.fn(),
    reject: vi.fn(),
    accept: vi.fn(),
    frameSnapshot: vi.fn(),
  };
  render(<App backend={api} />);
  await screen.findByText("Unsaved changes");
  const event = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
});

it("confirms native close for dirty documents and allows close after saving", async () => {
  native.enabled = true;
  const api: DocumentBackend = {
    snapshot: vi.fn().mockResolvedValue({ ...snapshot, dirty: true }),
    create: vi.fn(),
    command: vi.fn(),
    save: vi.fn().mockResolvedValue(snapshot),
    load: vi.fn(),
    export: vi.fn(),
    models: vi.fn().mockResolvedValue({ configured: false, models: [] }),
    submit: vi.fn(),
    poll: vi.fn(),
    reject: vi.fn(),
    accept: vi.fn(),
    frameSnapshot: vi.fn(),
  };
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  render(<App backend={api} />);
  await screen.findByText("Unsaved changes");
  await waitFor(() => expect(native.handler).toBeDefined());
  const preventDefault = vi.fn();
  native.handler!({ preventDefault });
  expect(confirm).toHaveBeenCalled();
  expect(preventDefault).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  preventDefault.mockClear();
  confirm.mockClear();
  native.handler!({ preventDefault });
  expect(confirm).not.toHaveBeenCalled();
  expect(preventDefault).not.toHaveBeenCalled();
  native.enabled = false;
});
