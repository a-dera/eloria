<script lang="ts">
  import { t, tn } from "../lib/i18n.svelte";
  import { onDestroy, onMount } from "svelte";
  import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type Simulation, type SimulationLinkDatum, type SimulationNodeDatum } from "d3-force";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import type { Graph, GraphNode } from "../lib/types";
  import Icon from "./Icon.svelte";

  let {
    visible = true,
    local = null,
    depth = 1,
    includeTags = false,
  }: { visible?: boolean; local?: string | null; depth?: number; includeTags?: boolean } = $props();

  type N = GraphNode & SimulationNodeDatum & { r: number };
  type L = SimulationLinkDatum<N> & { kind: string };

  let wrap: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let w = 0;
  let h = 0;
  let dpr = 1;
  let nodes: N[] = [];
  let links: L[] = [];
  let neighbors = new Map<string, Set<string>>();
  let sim: Simulation<N, L> | null = null;
  let tf = { x: 0, y: 0, k: 1 };
  let hover: N | null = null;
  let dragging: N | null = null;
  let panning: { x: number; y: number; tx: number; ty: number } | null = null;
  let moved = false;
  let fitted = false;
  /** Automatic framing as long as the user has not moved or zoomed the view. */
  let autoFit = true;
  let ticks = 0;

  // Options (global view)
  // Options remembered between sessions; tags are shown by default:
  // they are what connects readings by topic.
  const saved = (() => {
    try {
      return JSON.parse(localStorage.getItem("eloria:graph") ?? "{}") as { tags?: boolean; orphans?: boolean; missing?: boolean };
    } catch {
      return {};
    }
  })();
  let showTags = $state(saved.tags ?? true);
  let showOrphans = $state(saved.orphans ?? true);
  let showMissing = $state(saved.missing ?? false);
  $effect(() => {
    try {
      localStorage.setItem("eloria:graph", JSON.stringify({ tags: showTags, orphans: showOrphans, missing: showMissing }));
    } catch {}
  });
  let folder = $state("");
  let filter = $state("");
  let stats = $state({ nodes: 0, edges: 0 });

  const PALETTE = ["#7c5cff", "#2c8c99", "#d4880f", "#c2413b", "#2f8a57", "#3a7bd5", "#b8721c", "#a43fa9", "#5b8c2a", "#8a5cd1"];
  const groupColor = new Map<string, string>();
  function colorOf(n: N): string {
    if (n.kind === "tag") return "#2c8c99";
    if (n.kind === "missing") return "transparent";
    if (!n.group) return cssVar("--text-faint");
    if (!groupColor.has(n.group)) groupColor.set(n.group, PALETTE[groupColor.size % PALETTE.length]);
    return groupColor.get(n.group)!;
  }

  let cssCache: Record<string, string> = {};
  function cssVar(name: string): string {
    if (!cssCache[name]) cssCache[name] = getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#888";
    return cssCache[name];
  }

  const folders = $derived([...new Set(app.notes.filter((n) => n.path.includes("/")).map((n) => n.path.split("/")[0]))].sort());

  async function load() {
    let g: Graph;
    try {
      g = local
        ? await api.localGraph(local, depth, includeTags)
        : await api.graph({
            include_tags: showTags,
            include_orphans: showOrphans,
            include_missing: showMissing,
            folder: folder || null,
            exclude: [app.settings?.templates_folder ?? "Modèles"],
          });
    } catch {
      return;
    }
    const old = new Map(nodes.map((n) => [n.id, n]));
    nodes = g.nodes.map((n) => {
      const o = old.get(n.id);
      return { ...n, r: (n.kind === "tag" ? 3 : 4) + Math.sqrt(n.degree) * 1.7, x: o?.x, y: o?.y, vx: o?.vx, vy: o?.vy };
    });
    const ids = new Set(nodes.map((n) => n.id));
    links = g.edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ source: e.source, target: e.target, kind: e.kind })) as unknown as L[];
    neighbors = new Map();
    for (const e of g.edges) {
      if (!neighbors.has(e.source)) neighbors.set(e.source, new Set());
      if (!neighbors.has(e.target)) neighbors.set(e.target, new Set());
      neighbors.get(e.source)!.add(e.target);
      neighbors.get(e.target)!.add(e.source);
    }
    stats = { nodes: nodes.length, edges: links.length };
    fitted = old.size > 0;
    ticks = 0;
    restart(old.size === 0 ? 1 : 0.4);
  }

  function restart(alpha: number) {
    sim?.stop();
    const small = nodes.length < 60;
    sim = forceSimulation<N, L>(nodes)
      .force("link", forceLink<N, L>(links).id((d) => d.id).distance(local ? 70 : small ? 90 : 50).strength(0.4))
      .force("charge", forceManyBody().strength(local ? -260 : small ? -420 : -140).distanceMax(700))
      .force("center", forceCenter(0, 0))
      .force("x", forceX(0).strength(small ? 0.03 : 0.05))
      .force("y", forceY(0).strength(small ? 0.03 : 0.05))
      .force("collide", forceCollide<N>().radius((d) => d.r + (small ? 14 : 4)))
      .alpha(alpha)
      .on("tick", () => {
        ticks++;
        if (autoFit && !fitted && ticks % 4 === 0) fit();
        if (sim && !fitted && sim.alpha() < 0.03) {
          fitted = true;
          if (autoFit) fit();
        }
        draw();
      });
    if (!visible) sim.stop();
  }

  function resize() {
    if (!wrap || !canvas) return;
    const r = wrap.getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    w = r.width;
    h = r.height;
    canvas.width = Math.max(1, w * dpr);
    canvas.height = Math.max(1, h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    if (tf.x === 0 && tf.y === 0) tf = { x: w / 2, y: h / 2, k: local ? 1.1 : 1 };
    if (autoFit) fit();
    draw();
  }

  function draw() {
    if (!ctx) return;
    cssCache = {};
    const text = cssVar("--text");
    const faint = cssVar("--text-faint");
    const line = cssVar("--line-strong");
    const accent = cssVar("--accent");
    const bg = cssVar("--bg");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.translate(tf.x, tf.y);
    ctx.scale(tf.k, tf.k);

    const focus = hover ?? (local ? nodes.find((n) => n.id === local) ?? null : null);
    const near = focus ? neighbors.get(focus.id) ?? new Set<string>() : null;
    const q = filter.trim().toLowerCase();
    const match = (n: N) => !q || n.label.toLowerCase().includes(q);

    // A single path per style (links, tags, highlighted links): one stroke() call per
    // link becomes very slow beyond a few thousand links.
    const passes: { kind: string; lit: boolean }[] = [
      { kind: "link", lit: false },
      { kind: "citation", lit: false },
      { kind: "tag", lit: false },
      { kind: "link", lit: true },
      { kind: "citation", lit: true },
      { kind: "tag", lit: true },
    ];
    for (const pass of passes) {
      if (pass.lit && !focus) continue;
      ctx.beginPath();
      let any = false;
      for (const l of links) {
        const s = l.source as N;
        const tgt = l.target as N;
        if (s.x === undefined || tgt.x === undefined || l.kind !== pass.kind) continue;
        const lit = !!focus && (s.id === focus.id || tgt.id === focus.id);
        if (lit !== pass.lit) continue;
        ctx.moveTo(s.x, s.y!);
        ctx.lineTo(tgt.x, tgt.y!);
        any = true;
      }
      if (!any) continue;
      ctx.strokeStyle = pass.lit ? accent : line;
      ctx.globalAlpha = focus && !pass.lit ? 0.25 : pass.kind === "tag" ? 0.5 : 0.8;
      ctx.lineWidth = (pass.lit ? 1.6 : 1) / Math.max(0.6, tf.k);
      // Tags dotted, citations long-dashed, links solid.
      ctx.setLineDash(pass.kind === "tag" ? [3, 3] : pass.kind === "citation" ? [8, 4] : []);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;

    for (const n of nodes) {
      if (n.x === undefined) continue;
      const dim = (focus && n.id !== focus.id && !near!.has(n.id)) || !match(n);
      ctx.globalAlpha = dim ? 0.22 : 1;
      ctx.beginPath();
      ctx.arc(n.x, n.y!, n.r, 0, Math.PI * 2);
      if (n.kind === "missing") {
        ctx.fillStyle = bg;
        ctx.fill();
        ctx.strokeStyle = faint;
        ctx.setLineDash([2, 2]);
        ctx.lineWidth = 1.2 / tf.k;
        ctx.stroke();
        ctx.setLineDash([]);
      } else {
        ctx.fillStyle = n.id === local || n.id === app.activeNotePath ? accent : colorOf(n);
        ctx.fill();
        if (n.kind === "tag") {
          ctx.strokeStyle = bg;
          ctx.lineWidth = 1.5 / tf.k;
          ctx.stroke();
        }
      }
      if (focus && n.id === focus.id) {
        ctx.strokeStyle = accent;
        ctx.lineWidth = 2 / tf.k;
        ctx.beginPath();
        ctx.arc(n.x, n.y!, n.r + 3, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
    // Labels
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    const fs = 11.5 / Math.max(0.7, tf.k);
    ctx.font = `500 ${fs}px ${cssVar("--font-ui")}`;
    for (const n of nodes) {
      if (n.x === undefined) continue;
      const important = focus ? n.id === focus.id || near!.has(n.id) : tf.k > 1.15 || n.degree >= 6 || nodes.length < 40;
      if (!important || !match(n)) continue;
      ctx.globalAlpha = focus && n.id !== focus.id && !near!.has(n.id) ? 0.2 : 1;
      const label = n.label.length > 34 ? n.label.slice(0, 33) + "…" : n.label;
      ctx.lineWidth = 3 / tf.k;
      ctx.strokeStyle = bg;
      ctx.strokeText(label, n.x, n.y! + n.r + 3);
      ctx.fillStyle = n.kind === "tag" ? "#2c8c99" : text;
      ctx.fillText(label, n.x, n.y! + n.r + 3);
    }
    ctx.globalAlpha = 1;
  }

  function toWorld(e: PointerEvent | WheelEvent | MouseEvent) {
    const r = canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left - tf.x) / tf.k, y: (e.clientY - r.top - tf.y) / tf.k };
  }

  function pick(e: PointerEvent | MouseEvent): N | null {
    const p = toWorld(e);
    let best: N | null = null;
    let bd = Infinity;
    for (const n of nodes) {
      if (n.x === undefined) continue;
      const d = Math.hypot(n.x - p.x, n.y! - p.y);
      if (d < n.r + 5 / tf.k && d < bd) {
        bd = d;
        best = n;
      }
    }
    return best;
  }

  function onDown(e: PointerEvent) {
    canvas.setPointerCapture(e.pointerId);
    moved = false;
    const n = pick(e);
    if (n) {
      dragging = n;
      n.fx = n.x;
      n.fy = n.y;
      sim?.alphaTarget(0.25).restart();
    } else {
      panning = { x: e.clientX, y: e.clientY, tx: tf.x, ty: tf.y };
      autoFit = false;
    }
  }

  function onMove(e: PointerEvent) {
    if (dragging) {
      const p = toWorld(e);
      dragging.fx = p.x;
      dragging.fy = p.y;
      moved = true;
      return;
    }
    if (panning) {
      tf.x = panning.tx + e.clientX - panning.x;
      tf.y = panning.ty + e.clientY - panning.y;
      moved = true;
      draw();
      return;
    }
    const n = pick(e);
    if (n !== hover) {
      hover = n;
      canvas.style.cursor = n ? "pointer" : "grab";
      draw();
    }
  }

  function onUp(e: PointerEvent) {
    if (dragging) {
      const n = dragging;
      dragging.fx = null;
      dragging.fy = null;
      dragging = null;
      sim?.alphaTarget(0);
      if (!moved) openNode(n, e);
    }
    panning = null;
  }

  function openNode(n: N, e: MouseEvent) {
    if (n.kind === "note") app.openNote(n.id, { newTab: true, otherPane: e.altKey });
    else if (n.kind === "tag") {
      app.searchQuery = n.label;
      app.leftPanel = "search";
      app.leftOpen = true;
    } else if (n.kind === "missing" && local) app.followLink(n.label, local);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    autoFit = false;
    const r = canvas.getBoundingClientRect();
    const mx = e.clientX - r.left;
    const my = e.clientY - r.top;
    const k = Math.min(4, Math.max(0.15, tf.k * Math.exp(-e.deltaY * 0.0015)));
    tf.x = mx - ((mx - tf.x) * k) / tf.k;
    tf.y = my - ((my - tf.y) * k) / tf.k;
    tf.k = k;
    draw();
  }

  /** Fits the whole graph in the view. */
  function fit() {
    const pts = nodes.filter((n) => n.x !== undefined);
    if (!pts.length || !w || !h) return;
    const xs = pts.map((n) => n.x!);
    const ys = pts.map((n) => n.y!);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const pad = Math.min(70, Math.min(w, h) * 0.12);
    const k = Math.min(2.2, Math.max(0.2, Math.min((w - pad * 2) / Math.max(1, x1 - x0), (h - pad * 2) / Math.max(1, y1 - y0))));
    tf = { k, x: w / 2 - ((x0 + x1) / 2) * k, y: h / 2 - ((y0 + y1) / 2) * k };
  }

  function recenter() {
    autoFit = true;
    fit();
    draw();
  }

  let ro: ResizeObserver;
  onMount(() => {
    ctx = canvas.getContext("2d")!;
    ro = new ResizeObserver(() => resize());
    ro.observe(wrap);
    resize();
  });
  onDestroy(() => {
    sim?.stop();
    ro?.disconnect();
  });

  $effect(() => {
    void local;
    void depth;
    void includeTags;
    void showTags;
    void showOrphans;
    void showMissing;
    void folder;
    void app.revision;
    load();
  });

  $effect(() => {
    if (visible) {
      resize();
      sim?.restart();
    } else sim?.stop();
  });

  $effect(() => {
    void filter;
    void app.activeNotePath;
    draw();
  });
</script>

<div class="graph" class:local={!!local}>
  {#if !local}
    <div class="toolbar">
      <input class="input" placeholder={t("Filtrer les nœuds")} bind:value={filter} />
      <select class="input" bind:value={folder} title={t("Limiter à un dossier")}>
        <option value="">{t("Tout le coffre")}</option>
        {#each folders as f (f)}<option value={f}>{f}</option>{/each}
      </select>
      <label><input type="checkbox" bind:checked={showTags} /> {t("Tags")}</label>
      <label><input type="checkbox" bind:checked={showOrphans} /> {t("Notes isolées")}</label>
      <label><input type="checkbox" bind:checked={showMissing} /> {t("Notes à créer")}</label>
      <span class="grow"></span>
      <span class="faint">{tn(stats.nodes, "{n} note", "{n} notes")} · {tn(stats.edges, "{n} lien", "{n} liens")}</span>
      <button class="icon-btn" title={t("Recentrer")} aria-label={t("Recentrer")} onclick={recenter}><Icon name="focus" /></button>
    </div>
  {/if}
  <div class="canvas" bind:this={wrap}>
    <canvas bind:this={canvas} onpointerdown={onDown} onpointermove={onMove} onpointerup={onUp} onwheel={onWheel} onpointerleave={() => { hover = null; draw(); }}></canvas>
    {#if !local && groupColor.size}
      <div class="legend">
        {#each [...groupColor.entries()] as [g, c] (g)}<span><i style:background={c}></i>{g}</span>{/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .graph {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12.5px;
    color: var(--text-soft);
    flex-wrap: wrap;
  }
  .toolbar .input {
    width: 180px;
    height: 28px;
  }
  .toolbar select.input {
    width: 150px;
  }
  .toolbar label {
    display: flex;
    gap: 5px;
    align-items: center;
  }
  .toolbar input[type="checkbox"] {
    accent-color: var(--accent);
  }
  .grow {
    flex: 1;
  }
  .canvas {
    flex: 1;
    position: relative;
    min-height: 0;
    overflow: hidden;
  }
  canvas {
    display: block;
    cursor: grab;
    touch-action: none;
  }
  .legend {
    position: absolute;
    left: 12px;
    bottom: 10px;
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 11.5px;
    color: var(--text-soft);
    pointer-events: none;
    background: color-mix(in srgb, var(--bg) 80%, transparent);
    padding: 4px 8px;
    border-radius: 6px;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .legend i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
</style>
