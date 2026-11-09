<script lang="ts">
  import { onDestroy } from "svelte";
  import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation, type SimulationNodeDatum } from "d3-force";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { CitationNetwork } from "../lib/types";

  // Citation network of the library: who cites whom among your documents (OpenAlex).
  // References are fetched once, then kept in .eloria/citations.json.
  let { visible = true }: { visible?: boolean } = $props();
  type N = SimulationNodeDatum & { id: string; label: string; title: string; deg: number; out: number };
  type L = { source: N | string; target: N | string };
  let net = $state<CitationNetwork | null>(null);
  let nodes = $state<N[]>([]);
  let links = $state<L[]>([]);
  let tick = $state(0);
  let fetching = $state<number | null>(null);
  let error = $state("");
  let onlyLinked = $state(true);
  let hover = $state<string | null>(null);
  let host = $state<HTMLDivElement>();
  let w = $state(900);
  let h = $state(600);
  let sim: ReturnType<typeof forceSimulation<N>> | null = null;

  async function load() {
    try {
      net = await api.citationNetwork();
      build();
    } catch (e) {
      error = translateError(e);
    }
  }
  $effect(() => {
    if (visible && !net) load();
  });
  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver(() => {
      w = host!.clientWidth;
      h = host!.clientHeight;
    });
    ro.observe(host);
    return () => ro.disconnect();
  });

  function build() {
    if (!net) return;
    const deg = new Map<string, number>();
    const out = new Map<string, number>();
    for (const [a, b] of net.edges) {
      deg.set(b, (deg.get(b) ?? 0) + 1);
      out.set(a, (out.get(a) ?? 0) + 1);
    }
    const keep = net.nodes.filter((n) => !onlyLinked || deg.has(n.citekey) || out.has(n.citekey));
    nodes = keep.map((n) => ({ id: n.citekey, label: n.label, title: n.title, deg: deg.get(n.citekey) ?? 0, out: out.get(n.citekey) ?? 0 }));
    const ids = new Set(nodes.map((n) => n.id));
    links = net.edges.filter(([a, b]) => ids.has(a) && ids.has(b)).map(([a, b]) => ({ source: a, target: b }));
    sim?.stop();
    sim = forceSimulation<N>(nodes)
      .force("link", forceLink<N, L & { source: N; target: N }>(links as never).id((d) => d.id).distance(120))
      .force("charge", forceManyBody().strength(-320))
      .force("center", forceCenter(w / 2, h / 2))
      .force("collide", forceCollide<N>().radius((d) => r(d) + 28))
      .on("tick", () => tick++);
  }
  const r = (n: N) => 5 + Math.sqrt(n.deg) * 4;

  async function fetchMissing() {
    if (!net) return;
    fetching = 0;
    error = "";
    try {
      net = await api.citationFetch(80, (n) => (fetching = n));
      build();
    } catch (e) {
      error = translateError(e);
    } finally {
      fetching = null;
    }
  }

  function open(citekey: string) {
    const p = app.fiches[citekey.toLowerCase()];
    if (p) app.openNote(p, { newTab: true });
  }
  const pos = (x: N | string) => (typeof x === "string" ? nodes.find((n) => n.id === x) : x);
  const related = $derived.by(() => {
    void tick;
    if (!hover) return null;
    const s = new Set([hover]);
    for (const l of links) {
      const a = pos(l.source)?.id;
      const b = pos(l.target)?.id;
      if (a === hover && b) s.add(b);
      if (b === hover && a) s.add(a);
    }
    return s;
  });
  const at = (n: N) => (void tick, `${n.x ?? w / 2},${n.y ?? h / 2}`);
  /** Segment shortened so that the arrow stops at the edge of the target node. */
  function seg(l: L): [number, number, number, number, string, string] | null {
    void tick;
    const a = pos(l.source);
    const b = pos(l.target);
    if (!a || !b || a.x == null || b.x == null) return null;
    const dx = b.x - a.x;
    const dy = (b.y ?? 0) - (a.y ?? 0);
    const len = Math.max(1, Math.hypot(dx, dy));
    return [a.x, a.y ?? 0, b.x - (dx / len) * (r(b) + 2), (b.y ?? 0) - (dy / len) * (r(b) + 2), a.id, b.id];
  }
  onDestroy(() => sim?.stop());
</script>

<div class="cg">
  <div class="bar">
    <b>{t("Réseau de citations")}</b>
    {#if net}
      <span class="faint">{tn(nodes.length, "{n} document", "{n} documents")} · {tn(links.length, "{n} citation", "{n} citations")}</span>
    {/if}
    <span class="grow"></span>
    <label class="check"><input type="checkbox" bind:checked={onlyLinked} onchange={build} /> {t("Seulement les documents reliés")}</label>
    {#if net?.missing.length}
      <button class="btn small primary" disabled={fetching !== null} onclick={fetchMissing}>
        {fetching !== null ? t("{n} sur {total}…", { n: fetching, total: Math.min(80, net.missing.length) }) : tn(net.missing.length, "Récupérer les références ({n} document)", "Récupérer les références ({n} documents)")}
      </button>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <div class="canvas" bind:this={host}>
    {#if net && !nodes.length}
      <p class="empty">{net.missing.length ? t("Récupère d'abord les références de tes documents (OpenAlex) : le réseau montre ensuite qui cite qui dans ta bibliothèque.") : t("Aucune citation entre tes documents.")}</p>
    {/if}
    <svg width={w} height={h} role="img" aria-label={t("Réseau de citations")}>
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="var(--line-strong)" /></marker>
      </defs>
      {#each links as l, i (i)}
        {@const c = seg(l)}
        {#if c}
          <line x1={c[0]} y1={c[1]} x2={c[2]} y2={c[3]} class:dim={related && !(related.has(c[4]) && related.has(c[5]))} marker-end="url(#arrow)" />
        {/if}
      {/each}
      {#each nodes as n (n.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <g transform="translate({at(n)})" class:dim={related && !related.has(n.id)} onmouseenter={() => (hover = n.id)} onmouseleave={() => (hover = null)} onclick={() => open(n.id)}>
          <circle r={r(n)} />
          <text y={-r(n) - 4}>{n.label}</text>
          <title>{n.title} · {t("cité {a} fois, cite {b} documents de la bibliothèque", { a: n.deg, b: n.out })}</title>
        </g>
      {/each}
    </svg>
  </div>
</div>

<style>
  .cg {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
  }
  .canvas {
    flex: 1;
    position: relative;
    min-height: 0;
    overflow: hidden;
  }
  .empty {
    position: absolute;
    inset: 40% 15% auto;
    text-align: center;
    color: var(--text-soft);
  }
  line {
    stroke: var(--line-strong);
    stroke-width: 1.2;
  }
  circle {
    fill: var(--accent);
    stroke: var(--bg);
    stroke-width: 2;
    cursor: pointer;
  }
  text {
    font: 11px var(--font-ui);
    fill: var(--text-soft);
    text-anchor: middle;
    pointer-events: none;
  }
  .dim {
    opacity: 0.15;
  }
  .error {
    color: var(--danger);
    padding: 0 16px;
  }
</style>
