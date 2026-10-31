import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
const job = {
  id: "job_1",
  status: "queued",
  model: "model",
  prompt: "hello",
  document_id: "doc",
  base_revision: 0,
  layer_id: "layer",
  frame_id: "frame",
  width: 16,
  height: 16,
  pixels: null,
  error:
    "Easel accepted job job_1, local storage failed. Do not resubmit; check status.",
};
beforeEach(() => {
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
it("retains an accepted job after storage failure and recovers it without resubmitting", async () => {
  const api: DocumentBackend = {
    snapshot: vi.fn().mockResolvedValue(snapshot),
    create: vi.fn(),
    command: vi.fn(),
    save: vi.fn(),
    load: vi.fn(),
    export: vi.fn(),
    models: vi.fn().mockResolvedValue({ configured: true, models: ["model"] }),
    submit: vi.fn().mockResolvedValue(job),
    poll: vi
      .fn()
      .mockRejectedValueOnce("Storage unavailable")
      .mockResolvedValueOnce({
        ...job,
        status: "completed",
        error: null,
        pixels: Array(1024).fill(0),
      }),
    reject: vi.fn(),
    accept: vi.fn(),
    frameSnapshot: vi.fn(),
  };
  render(<App backend={api} />);
  await screen.findByLabelText("Pixel canvas");
  fireEvent.change(screen.getByLabelText("Prompt"), {
    target: { value: "hello" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Generate candidate" }));
  await screen.findByText(/Easel accepted job job_1/);
  expect(
    (
      screen.getByRole("button", {
        name: "Generate candidate",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await screen.findByText(/Polling failed/, {}, { timeout: 3000 });
  fireEvent.click(screen.getByRole("button", { name: "Retry status" }));
  await screen.findByText("Status: completed");
  expect(api.submit).toHaveBeenCalledTimes(1);
  expect(api.poll).toHaveBeenCalledTimes(2);
});
