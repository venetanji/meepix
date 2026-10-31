import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Point = { x: number; y: number };
type Color = [number, number, number, number];
export type Snapshot = {
  document_id: string;
  revision: number;
  width: number;
  height: number;
  layers: { id: string; name: string; visible: boolean }[];
  frames: { id: string; duration_ms: number }[];
  active_layer_id: string;
  active_frame_id: string;
  palette: Color[];
  pixels: number[];
  can_undo: boolean;
  can_redo: boolean;
  dirty: boolean;
};
type Command = { type: string; [key: string]: unknown };
type Generation = {
  id: string;
  status: string;
  model: string;
  prompt: string;
  document_id: string;
  base_revision: number;
  layer_id: string;
  frame_id: string;
  width: number;
  height: number;
  pixels: number[] | null;
  error: string | null;
};
export interface DocumentBackend {
  snapshot(): Promise<Snapshot>;
  create(width: number, height: number): Promise<Snapshot>;
  command(expectedRevision: number, command: Command): Promise<Snapshot>;
  save(name: string): Promise<Snapshot>;
  load(name: string): Promise<Snapshot>;
  export(name: string): Promise<string>;
  models(): Promise<{ configured: boolean; models: string[] }>;
  submit(
    expectedRevision: number,
    model: string,
    prompt: string,
    mode: string,
  ): Promise<Generation>;
  poll(id: string): Promise<Generation>;
  reject(id: string): Promise<void>;
  accept(id: string, expectedRevision: number): Promise<Snapshot>;
  frameSnapshot(frameId: string): Promise<number[]>;
}
export const tauriBackend: DocumentBackend = {
  snapshot: () => invoke("document_snapshot"),
  create: (width, height) => invoke("document_create", { width, height }),
  command: (expectedRevision, command) =>
    invoke("document_command", { expectedRevision, command }),
  save: (name) => invoke("project_save", { name }),
  load: (name) => invoke("project_load", { name }),
  export: (name) => invoke("project_export", { name }),
  models: () => invoke("easel_models"),
  submit: (expectedRevision, model, prompt, mode) =>
    invoke("generation_submit", { expectedRevision, model, prompt, mode }),
  poll: (id) => invoke("generation_poll", { id }),
  reject: (id) => invoke("generation_reject", { id }),
  accept: (id, expectedRevision) =>
    invoke("generation_accept", { id, expectedRevision }),
  frameSnapshot: (frameId) => invoke("frame_snapshot", { frameId }),
};
export function mapPoint(
  clientX: number,
  clientY: number,
  rect: Pick<DOMRect, "left" | "top" | "width" | "height">,
  width: number,
  height: number,
): Point | null {
  const x = Math.floor(((clientX - rect.left) * width) / rect.width),
    y = Math.floor(((clientY - rect.top) * height) / rect.height);
  return x >= 0 && y >= 0 && x < width && y < height ? { x, y } : null;
}
function rgba(hex: string): Color {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
    255,
  ];
}
function hex(c: Color) {
  return (
    "#" +
    c
      .slice(0, 3)
      .map((n) => n.toString(16).padStart(2, "0"))
      .join("")
  );
}
function PixelImage({
  pixels,
  width,
  height,
  label,
  canvasRef,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
  style,
}: {
  pixels: number[];
  width: number;
  height: number;
  label: string;
  canvasRef?: React.RefObject<HTMLCanvasElement | null>;
  onPointerDown?: React.PointerEventHandler<HTMLCanvasElement>;
  onPointerMove?: React.PointerEventHandler<HTMLCanvasElement>;
  onPointerUp?: React.PointerEventHandler<HTMLCanvasElement>;
  onPointerCancel?: React.PointerEventHandler<HTMLCanvasElement>;
  style?: React.CSSProperties;
}) {
  const own = useRef<HTMLCanvasElement>(null);
  const ref = canvasRef || own;
  useEffect(() => {
    const ctx = ref.current?.getContext("2d");
    if (!ctx || pixels.length !== width * height * 4) return;
    const image = ctx.createImageData(width, height);
    image.data.set(pixels);
    ctx.putImageData(image, 0, 0);
  }, [pixels, width, height, ref]);
  return (
    <canvas
      ref={ref}
      width={width}
      height={height}
      aria-label={label}
      style={style}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
    />
  );
}
export default function App({
  backend = tauriBackend,
}: {
  backend?: DocumentBackend;
}) {
  const [doc, setDoc] = useState<Snapshot | null>(null);
  const docRef = useRef<Snapshot | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [tool, setTool] = useState<"pencil" | "eraser" | "fill" | "pan">(
    "pencil",
  );
  const [color, setColor] = useState("#f0f2ff");
  const [zoom, setZoom] = useState(8);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [grid, setGrid] = useState(true);
  const [onion, setOnion] = useState(false);
  const [previous, setPrevious] = useState<number[] | null>(null);
  const [name, setName] = useState("untitled");
  const [size, setSize] = useState(64);
  const [playing, setPlaying] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [configured, setConfigured] = useState(false);
  const [model, setModel] = useState("");
  const [mode, setMode] = useState("generate");
  const [prompt, setPrompt] = useState("");
  const [generation, setGeneration] = useState<Generation | null>(null);
  const [pollError, setPollError] = useState(false);
  const [polling, setPolling] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [drawing, setDrawing] = useState(false);
  const stroke = useRef<{
    points: Point[];
    revision: number;
    layerId: string;
    frameId: string;
    color: Color;
  } | null>(null);
  const documentRequest = useRef(0);
  const candidateRequest = useRef(0);
  const panStart = useRef<{
    x: number;
    y: number;
    px: number;
    py: number;
  } | null>(null);
  const lock = useRef(false);
  const update = useCallback((next: Snapshot) => {
    docRef.current = next;
    setDoc(next);
  }, []);
  useEffect(() => {
    let active = true;
    const request = ++documentRequest.current;
    backend
      .snapshot()
      .then((snapshot) => {
        if (active && request === documentRequest.current) update(snapshot);
      })
      .catch((e) => {
        if (active && request === documentRequest.current)
          setError(`Could not open document: ${String(e)}`);
      });
    backend
      .models()
      .then((result) => {
        if (active) {
          setConfigured(result.configured);
          setModels(result.models);
          setModel(result.models[0] || "");
        }
      })
      .catch(() => {
        if (active) setConfigured(false);
      });
    return () => {
      active = false;
    };
  }, [backend, update]);
  const run = useCallback(
    async (fn: () => Promise<Snapshot>, message = "") => {
      if (lock.current) return;
      lock.current = true;
      documentRequest.current++;
      setBusy(true);
      setError("");
      try {
        update(await fn());
        if (message) setNotice(message);
      } catch (e) {
        setPlaying(false);
        setError(String(e));
      } finally {
        lock.current = false;
        setBusy(false);
      }
    },
    [update],
  );
  const command = useCallback(
    (cmd: Command) => {
      const current = docRef.current;
      if (current) void run(() => backend.command(current.revision, cmd));
    },
    [backend, run],
  );
  useEffect(() => {
    if (!doc || !onion || doc.frames.length < 2) {
      setPrevious(null);
      return;
    }
    const index = doc.frames.findIndex((f) => f.id === doc.active_frame_id);
    if (index <= 0) {
      setPrevious(null);
      return;
    }
    let active = true;
    backend
      .frameSnapshot(doc.frames[index - 1].id)
      .then((p) => {
        if (active) setPrevious(p);
      })
      .catch(() => {
        if (active) setPrevious(null);
      });
    return () => {
      active = false;
    };
  }, [backend, doc, onion]);
  useEffect(() => {
    if (!playing || busy || drawing || !doc || doc.frames.length < 2) return;
    const i = doc.frames.findIndex((f) => f.id === doc.active_frame_id);
    const timer = setTimeout(
      () =>
        command({
          type: "select_frame",
          frame_id: doc.frames[(i + 1) % doc.frames.length].id,
        }),
      doc.frames[i]?.duration_ms || 100,
    );
    return () => clearTimeout(timer);
  }, [playing, busy, drawing, doc, command]);
  const pollStatus = useCallback(
    async (id: string) => {
      const request = candidateRequest.current;
      setPolling(true);
      setPollError(false);
      setError("");
      try {
        const result = await backend.poll(id);
        if (request === candidateRequest.current) setGeneration(result);
      } catch (e) {
        if (request === candidateRequest.current) {
          setPollError(true);
          setError(`Polling failed: ${String(e)}`);
        }
      } finally {
        if (request === candidateRequest.current) setPolling(false);
      }
    },
    [backend],
  );
  useEffect(() => {
    if (
      !generation ||
      pollError ||
      polling ||
      !["queued", "in_progress"].includes(generation.status)
    )
      return;
    const timer = setTimeout(() => void pollStatus(generation.id), 1500);
    return () => clearTimeout(timer);
  }, [generation, pollError, polling, pollStatus]);
  function point(e: React.PointerEvent<HTMLCanvasElement>) {
    return (
      doc &&
      mapPoint(
        e.clientX,
        e.clientY,
        e.currentTarget.getBoundingClientRect(),
        doc.width,
        doc.height,
      )
    );
  }
  function down(e: React.PointerEvent<HTMLCanvasElement>) {
    if (!doc || busy || lock.current || e.button !== 0) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    if (tool === "pan") {
      panStart.current = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
      return;
    }
    const p = point(e);
    if (!p) return;
    if (tool === "fill") {
      command({
        type: "fill",
        layer_id: doc.active_layer_id,
        frame_id: doc.active_frame_id,
        ...p,
        color: rgba(color),
      });
      return;
    }
    stroke.current = {
      points: [p],
      revision: doc.revision,
      layerId: doc.active_layer_id,
      frameId: doc.active_frame_id,
      color: tool === "eraser" ? [0, 0, 0, 0] : rgba(color),
    };
    setDrawing(true);
  }
  function move(e: React.PointerEvent<HTMLCanvasElement>) {
    if (panStart.current) {
      setPan({
        x: panStart.current.px + e.clientX - panStart.current.x,
        y: panStart.current.py + e.clientY - panStart.current.y,
      });
      return;
    }
    if (!stroke.current) return;
    const p = point(e);
    const points = stroke.current.points;
    const last = points[points.length - 1];
    if (p && (p.x !== last.x || p.y !== last.y)) points.push(p);
  }
  function up(e: React.PointerEvent<HTMLCanvasElement>) {
    if (panStart.current) {
      panStart.current = null;
      return;
    }
    if (!stroke.current || !doc) return;
    const p = point(e);
    const started = stroke.current;
    const points = started.points;
    if (
      p &&
      (p.x !== points[points.length - 1].x ||
        p.y !== points[points.length - 1].y)
    )
      points.push(p);
    stroke.current = null;
    setDrawing(false);
    void run(() =>
      backend.command(started.revision, {
        type: "paint",
        layer_id: started.layerId,
        frame_id: started.frameId,
        points,
        color: started.color,
      }),
    );
  }
  async function submit() {
    if (!doc || !model || !prompt.trim() || lock.current || drawing) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      setPollError(false);
      setGeneration(await backend.submit(doc.revision, model, prompt, mode));
    } catch (e) {
      setError(`Generation failed: ${String(e)}`);
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  async function reject() {
    if (!generation || lock.current || drawing) return;
    lock.current = true;
    setBusy(true);
    try {
      await backend.reject(generation.id);
      candidateRequest.current++;
      setPolling(false);
      setPollError(false);
      setGeneration(null);
      setNotice("Candidate rejected");
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  const activeFrame = doc?.frames.find((f) => f.id === doc.active_frame_id);
  return (
    <div className="app">
      <header>
        <div className="brand">
          MeePix <span>STUDIO</span>
        </div>
        <div className="document-title">
          {name} <small>{doc?.dirty ? "Unsaved changes" : "Saved"}</small>
        </div>
        <div className="header-actions">
          <button
            disabled={busy || drawing || !doc?.can_undo}
            onClick={() => command({ type: "undo" })}
          >
            Undo
          </button>
          <button
            disabled={busy || drawing || !doc?.can_redo}
            onClick={() => command({ type: "redo" })}
          >
            Redo
          </button>
          <button
            disabled={busy || drawing || !doc}
            onClick={() => void run(() => backend.save(name), "Project saved")}
          >
            Save
          </button>
        </div>
      </header>
      {(error || notice) && (
        <div className={error ? "feedback error" : "feedback"} role="status">
          {error || notice}
          <button
            onClick={() => {
              setError("");
              setNotice("");
            }}
            aria-label="Dismiss message"
          >
            ×
          </button>
        </div>
      )}
      <main>
        <aside className="left">
          <section>
            <h2>Tools</h2>
            <div className="tool-grid">
              {(["pencil", "eraser", "fill", "pan"] as const).map((t) => (
                <button
                  key={t}
                  aria-pressed={tool === t}
                  onClick={() => setTool(t)}
                >
                  {t[0].toUpperCase() + t.slice(1)}
                </button>
              ))}
            </div>
            <label className="field">
              Color{" "}
              <input
                aria-label="Color"
                type="color"
                value={color}
                onChange={(e) => setColor(e.target.value)}
              />
            </label>
            <div className="swatches">
              {doc?.palette.map((c, i) => (
                <button
                  key={i}
                  aria-label={`Palette color ${i + 1}`}
                  style={{ background: hex(c) }}
                  onClick={() => setColor(hex(c))}
                />
              ))}
            </div>
          </section>
          <section>
            <div className="section-heading">
              <h2>Layers</h2>
              <button
                disabled={busy || drawing || !doc || doc.layers.length >= 16}
                onClick={() =>
                  command({
                    type: "add_layer",
                    name: `Layer ${(doc?.layers.length || 0) + 1}`,
                  })
                }
              >
                + Add
              </button>
            </div>
            <div className="layer-list">
              {doc?.layers
                .slice()
                .reverse()
                .map((layer) => (
                  <div className="layer" key={layer.id}>
                    <button
                      disabled={busy || drawing}
                      className="visibility"
                      aria-label={`${layer.visible ? "Hide" : "Show"} ${layer.name}`}
                      onClick={() =>
                        command({
                          type: "set_layer_visibility",
                          layer_id: layer.id,
                          visible: !layer.visible,
                        })
                      }
                    >
                      {layer.visible ? "●" : "○"}
                    </button>
                    <button
                      disabled={busy || drawing}
                      aria-pressed={layer.id === doc.active_layer_id}
                      onClick={() =>
                        command({ type: "select_layer", layer_id: layer.id })
                      }
                    >
                      {layer.name}
                    </button>
                  </div>
                ))}
            </div>
          </section>
          <section>
            <h2>Project</h2>
            <label className="stack">
              Project name
              <input value={name} onChange={(e) => setName(e.target.value)} />
            </label>
            <div className="button-row">
              <button
                disabled={busy || drawing}
                onClick={() => {
                  if (
                    doc?.dirty &&
                    !window.confirm(
                      "Load a project and discard unsaved changes?",
                    )
                  )
                    return;
                  void run(() => backend.load(name), "Project loaded");
                }}
              >
                Load
              </button>
              <button
                disabled={busy || drawing || !doc}
                onClick={() => {
                  setError("");
                  backend
                    .export(name)
                    .then((path) => setNotice(`Exported to ${path}`))
                    .catch((e) => setError(String(e)));
                }}
              >
                Export sheet
              </button>
            </div>
            <div className="button-row">
              <select
                aria-label="New document size"
                value={size}
                onChange={(e) => setSize(Number(e.target.value))}
              >
                {[16, 32, 64, 128].map((n) => (
                  <option key={n} value={n}>
                    {n} × {n}
                  </option>
                ))}
              </select>
              <button
                disabled={busy || drawing}
                onClick={() => {
                  if (
                    doc?.dirty &&
                    !window.confirm(
                      "Create a new document and discard unsaved changes?",
                    )
                  )
                    return;
                  void run(
                    () => backend.create(size, size),
                    "New document created",
                  );
                }}
              >
                New
              </button>
            </div>
          </section>
        </aside>
        <section className="center">
          <div className="workspace-toolbar">
            <span>
              {doc ? `${doc.width} × ${doc.height} px` : "Opening document…"}
            </span>
            <div>
              <button
                onClick={() => setZoom(Math.max(1, zoom - 1))}
                aria-label="Zoom out"
              >
                −
              </button>
              <span className="zoom">{zoom * 100}%</span>
              <button
                onClick={() => setZoom(Math.min(20, zoom + 1))}
                aria-label="Zoom in"
              >
                +
              </button>
              <button aria-pressed={grid} onClick={() => setGrid(!grid)}>
                Grid
              </button>
              <button aria-pressed={onion} onClick={() => setOnion(!onion)}>
                Previous frame
              </button>
              <button onClick={() => setPan({ x: 0, y: 0 })}>Reset pan</button>
            </div>
          </div>
          <div className="workspace">
            <div
              className="artboard"
              style={{
                width: (doc?.width || 0) * zoom,
                height: (doc?.height || 0) * zoom,
                transform: `translate(${pan.x}px,${pan.y}px)`,
              }}
            >
              {doc && (
                <>
                  {previous && (
                    <PixelImage
                      pixels={previous}
                      width={doc.width}
                      height={doc.height}
                      label="Previous frame"
                      style={{ opacity: 0.3 }}
                    />
                  )}
                  <PixelImage
                    canvasRef={canvas}
                    pixels={doc.pixels}
                    width={doc.width}
                    height={doc.height}
                    label="Pixel canvas"
                    onPointerDown={down}
                    onPointerMove={move}
                    onPointerUp={up}
                    onPointerCancel={() => {
                      stroke.current = null;
                      setDrawing(false);
                      panStart.current = null;
                    }}
                  />
                  <div
                    className="grid"
                    style={{
                      display: grid && zoom >= 4 ? "block" : "none",
                      backgroundSize: `${zoom}px ${zoom}px`,
                    }}
                  />
                </>
              )}
            </div>
          </div>
          <div className="timeline">
            <div className="section-heading">
              <h2>Timeline</h2>
              <div className="button-row">
                <button
                  disabled={busy || drawing || !doc || doc.frames.length < 2}
                  aria-pressed={playing}
                  onClick={() => setPlaying(!playing)}
                >
                  {playing ? "Pause" : "Play"}
                </button>
                <button
                  disabled={busy || drawing || !doc || doc.frames.length >= 64}
                  onClick={() => command({ type: "add_frame" })}
                >
                  + Frame
                </button>
              </div>
            </div>
            <div className="frames">
              {doc?.frames.map((frame, i) => (
                <button
                  disabled={busy || drawing}
                  key={frame.id}
                  className="frame"
                  aria-pressed={frame.id === doc.active_frame_id}
                  onClick={() => {
                    setPlaying(false);
                    command({ type: "select_frame", frame_id: frame.id });
                  }}
                >
                  <strong>{String(i + 1).padStart(2, "0")}</strong>
                  <small>{frame.duration_ms} ms</small>
                </button>
              ))}
            </div>
            {activeFrame && (
              <label className="duration">
                Frame duration{" "}
                <input
                  disabled={busy || drawing}
                  type="number"
                  min="20"
                  max="10000"
                  key={activeFrame.id + ":" + activeFrame.duration_ms}
                  defaultValue={activeFrame.duration_ms}
                  onBlur={(e) => {
                    const n = Number(e.target.value);
                    if (n >= 20 && n <= 10000 && n !== activeFrame.duration_ms)
                      command({
                        type: "set_duration",
                        frame_id: activeFrame.id,
                        duration_ms: n,
                      });
                  }}
                />{" "}
                ms
              </label>
            )}
          </div>
        </section>
        <aside className="right">
          <section>
            <h2>Generate</h2>
            <p>
              Create a candidate to review before applying it to the active
              layer and frame.
            </p>
            {!configured ? (
              <p className="muted">
                Easel is not configured. Manual editing is available.
              </p>
            ) : (
              <>
                <label className="stack">
                  Model
                  <select
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                  >
                    {models.map((m) => (
                      <option key={m}>{m}</option>
                    ))}
                  </select>
                </label>
                <label className="stack">
                  Mode
                  <select
                    value={mode}
                    onChange={(e) => setMode(e.target.value)}
                  >
                    <option value="generate">Generate</option>
                    <option value="edit">Edit active cel</option>
                  </select>
                </label>
                <label className="stack">
                  Prompt
                  <textarea
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    rows={4}
                  />
                </label>
                <button
                  className="primary"
                  disabled={
                    busy ||
                    drawing ||
                    !doc ||
                    !model ||
                    !prompt.trim() ||
                    !!generation
                  }
                  onClick={() => void submit()}
                >
                  Generate candidate
                </button>
              </>
            )}
            {generation && (
              <div className="candidate">
                <h3>Candidate</h3>
                <p>Status: {generation.status.replace("_", " ")}</p>
                {["queued", "in_progress"].includes(generation.status) && (
                  <p>Working… progress unavailable</p>
                )}
                {(pollError ||
                  ![
                    "queued",
                    "in_progress",
                    "completed",
                    "failed",
                    "cancelled",
                  ].includes(generation.status)) && (
                  <div>
                    <p>
                      Progress is unavailable. You can check the status again.
                    </p>
                    <button
                      disabled={polling || busy}
                      onClick={() => void pollStatus(generation.id)}
                    >
                      Retry status
                    </button>
                  </div>
                )}
                {generation.error && (
                  <p className="error-text">{generation.error}</p>
                )}
                {generation.pixels && (
                  <div className="preview">
                    <PixelImage
                      pixels={generation.pixels}
                      width={generation.width}
                      height={generation.height}
                      label="Candidate preview"
                    />
                  </div>
                )}
                <p>
                  Preview is resized to document dimensions using
                  nearest-neighbor conversion.
                </p>
                <div className="button-row">
                  <button
                    disabled={busy || drawing}
                    onClick={() => void reject()}
                  >
                    Reject
                  </button>
                  <button
                    className="primary"
                    disabled={
                      busy ||
                      drawing ||
                      generation.status !== "completed" ||
                      !generation.pixels
                    }
                    onClick={() => {
                      if (!doc) return;
                      void run(async () => {
                        const result = await backend.accept(
                          generation.id,
                          doc.revision,
                        );
                        candidateRequest.current++;
                        setGeneration(null);
                        return result;
                      }, "Candidate applied");
                    }}
                  >
                    Accept
                  </button>
                </div>
              </div>
            )}
          </section>
        </aside>
      </main>
      <footer>
        <span>
          {tool.toUpperCase()} ·{" "}
          {doc
            ? `Frame ${doc.frames.findIndex((f) => f.id === doc.active_frame_id) + 1} / ${doc.frames.length}`
            : "No document"}
        </span>
        <span>{busy ? "Working…" : "Ready"}</span>
      </footer>
    </div>
  );
}
