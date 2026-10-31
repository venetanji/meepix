import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const SIZE = 32;
const SCALE = 14;

export default function App() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [color, setColor] = useState("#252b43");
  const [tool, setTool] = useState<"pencil" | "eraser">("pencil");
  const [pixels, setPixels] = useState<(string | null)[]>(() => Array(SIZE * SIZE).fill(null));
  const [drawing, setDrawing] = useState(false);
  const [backend, setBackend] = useState("not checked");

  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, SIZE * SCALE, SIZE * SCALE);
    for (let y = 0; y < SIZE; y++) for (let x = 0; x < SIZE; x++) {
      ctx.fillStyle = pixels[y * SIZE + x] ?? ((x + y) % 2 ? "#e8e8ec" : "#fafafc");
      ctx.fillRect(x * SCALE, y * SCALE, SCALE, SCALE);
    }
  }, [pixels]);

  function paint(event: React.PointerEvent<HTMLCanvasElement>) {
    if (event.type === "pointermove" && !drawing) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const x = Math.floor((event.clientX - bounds.left) * SIZE / bounds.width);
    const y = Math.floor((event.clientY - bounds.top) * SIZE / bounds.height);
    if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return;
    setPixels(previous => {
      const next = previous.slice();
      next[y * SIZE + x] = tool === "eraser" ? null : color;
      return next;
    });
  }

  async function checkEasel() {
    try { setBackend(await invoke<string>("easel_health")); }
    catch (error) { setBackend(String(error)); }
  }

  return <div className="app">
    <header><strong>MeePix</strong><small>Local-first sprite studio · foundation prototype</small></header>
    <main><aside><h3>Tools</h3><button onClick={() => setTool("pencil")} aria-pressed={tool === "pencil"}>Pencil</button><button onClick={() => setTool("eraser")} aria-pressed={tool === "eraser"}>Eraser</button><label>Color <input type="color" value={color} onChange={e => setColor(e.target.value)} /></label><button onClick={() => setPixels(Array(SIZE * SIZE).fill(null))}>Clear</button></aside>
    <section className="workspace"><canvas ref={canvas} width={SIZE * SCALE} height={SIZE * SCALE} onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); setDrawing(true); paint(e); }} onPointerMove={paint} onPointerUp={() => setDrawing(false)} onPointerCancel={() => setDrawing(false)} aria-label="Pixel canvas" /><p>32 × 32 · Frontend-only pixel sketch. Rust document engine is next.</p></section>
    <aside><h3>Generation</h3><p>Easel integration placeholder</p><button onClick={checkEasel}>Check local Easel</button><small>{backend}</small><h3>Agent</h3><p>Provider selection and tool protocol coming next.</p></aside></main>
    <footer>Timeline · Not yet implemented</footer>
  </div>;
}
