<script lang="ts">
  import { onDestroy } from "svelte";
  import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type SimulationNodeDatum } from "d3-force";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tn, translateError } from "../lib/i18n.svelte";
  import type { KnowledgeMap } from "../lib/types";

  // Knowledge map: concepts, methods, questions, chapters and arguments,
  // connected by their links (solid line) and by the mention of a concept (dotted).
  // Shape and colour show the type; hover: neighbours; click: the note.
  let { visible = true }: { visible?: boolean } = $props();
  type N = SimulationNodeDatum & { id: string; title: string; kind: string; degree: number };
  type L = { source: N | string; target: N | string; kind: string };
  let map = $state<KnowledgeMap | null>(null);
  let nodes = $state<N[]>([]);
  let links = $state<L[]>([]);
  let tick = $state(0);
  let withSources = $state(false);
  let hidden = $state<Record<string, boolean>>({});
  let hover = $state<string | null>(null);
  let error = $state("");
  let host = $state<HTMLDivElement>();
  let w = $state(900);
  let h = $state(600);
  let sim: ReturnType<typeof forceSimulation<N>> | null = null;

  const KINDS: { id: string; label: string; color: string }[] = [
    { id: "concept", label: t("Concepts"), color: "var(--series-1)" },
    { id: "methode", label: t("Méthodes"), color: "var(--series-2)" },
    { id: "question", label: t("Questions"), color: "var(--series-3)" },
    { id: "chapitre", label: t("Chapitres"), color: "var(--series-4)" },
    { id: "argument", label: t("Argumentations"), color: "var(--series-5)" },
    { id: "source", label: t("Sources"), color: "var(--text-faint)" },
    { id: "fiche", label: t("Fiches"), color: "var(--text-faint)" },
  ];
  const color = (k: string) => KINDS.find((x) => x.id === k)?.color ?? "var(--text-faint)";

  async function load() {
    try {
      map = await api.knowledgeMap(withSources);
      build();
    } catch (e) {
      error = translateError(e);
    }
  }
  $effect(() => {
    void withSources;
    void app.revision;
    if (visible) load();
  });
  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver(() => {
      w = host!.clientWidth;
      h = host!.clientHeight;
      sim?.force("center", forceCenter(w / 2, h / 2)).alpha(0.3).restart();
    });
    ro.observe(host);
    return () => ro.disconnect();
  });

  function build() {
    if (!map) return;
    const old = new Map(nodes.map((n) => [n.id, n]));
    nodes = map.nodes.filter((n) => !hidden[n.kind]).map((n) => ({ ...n, x: old.get(n.id)?.x, y: old.get(n.id)?.y }));
    const ids = new Set(nodes.map((n) => n.id));
    links = map.edges.filter(([a, b]) => ids.has(a) && ids.has(b)).map(([a, b, k]) => ({ source: a, target: b, kind: k }));
    sim?.stop();
    sim = forceSimulation<N>(nodes)
      .force("link", forceLink<N, L & { source: N; target: N }>(links as never).id((d) => d.id).distance((l) => (l.kind === "lien" ? 150 : 200)))
      .force("charge", forceManyBody().strength(-700))
      .force("center", forceCenter(w / 2, h / 2))
      .force("x", forceX(w / 2).strength(0.02))
      .force("y", forceY(h / 2).strength(0.03))
      .force("collide", forceCollide<N>().radius((d) => r(d) + 48))
      .on("tick", () => tick++);
  }
  function toggle(kind: string) {
    hidden[kind] = !hidden[kind];
    build();
  }
  const r = (n: N) => 6 + Math.sqrt(n.degree) * 3;
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
  /** Position (read at each simulation step; elements stay in place, and so does the click). */
  const at = (n: N) => (void tick, `${n.x ?? w / 2},${n.y ?? h / 2}`);
  function seg(l: L): [number, number, number, number, string, string] | null {
    void tick;
    const a = pos(l.source);
    const b = pos(l.target);
    if (!a || !b || a.x == null || b.x == null) return null;
    return [a.x, a.y ?? 0, b.x, b.y ?? 0, a.id, b.id];
  }
  const short = (s: string) => (s.length > 26 ? `${s.slice(0, 25)}…` : s);
  /** Shape per type (besides colour): circle, square, diamond, rectangle, triangle. */
  function shape(n: N): string {
    const s = r(n);
    switch (n.kind) {
      case "methode":
        return `M${-s},${-s}H${s}V${s}H${-s}Z`;
      case "question":
        return `M0,${-s * 1.3}L${s * 1.3},0L0,${s * 1.3}L${-s * 1.3},0Z`;
      case "chapitre":
        return `M${-s * 1.6},${-s * 0.8}H${s * 1.6}V${s * 0.8}H${-s * 1.6}Z`;
      case "argument":
        return `M0,${-s * 1.3}L${s * 1.2},${s}L${-s * 1.2},${s}Z`;
      default:
        return `M${-s},0a${s},${s} 0 1,0 ${s * 2},0a${s},${s} 0 1,0 ${-s * 2},0`;
    }
  }
  onDestroy(() => sim?.stop());
</script>

<div class="km">
  <div class="bar">
    <b>{t("Carte des connaissances")}</b>
    {#if map}<span class="faint">{tn(nodes.length, "{n} nœud", "{n} nœuds")} · {tn(links.length, "{n} relation", "{n} relations")}</span>{/if}
    <span class="grow"></span>
    <div class="legend" role="group" aria-label={t("Types affichés")}>
      {#each KINDS.filter((k) => withSources || (k.id !== "source" && k.id !== "fiche")) as k (k.id)}
        <button class="lg" class:off={hidden[k.id]} aria-pressed={!hidden[k.id]} onclick={() => toggle(k.id)} title={t("Afficher ou masquer : {k}", { k: k.label })}>
          <svg width="14" height="14" viewBox="-8 -8 16 16" aria-hidden="true"><path d={shape({ id: "", title: "", kind: k.id, degree: 0 })} fill={k.color} /></svg>
          {k.label}
        </button>
      {/each}
    </div>
    <label class="check"><input type="checkbox" bind:checked={withSources} /> {t("Sources et fiches")}</label>
  </div>
  <p class="hint">{t("Trait plein : lien entre les notes. Pointillés : le nom d'un concept ou d'une méthode est mentionné. Survol : voisins ; clic : ouvrir.")}</p>
  {#if error}<p class="error">{error}</p>{/if}
  <div class="canvas" bind:this={host}>
    {#if !map && !error}<p class="empty loading" role="status">{t("Analyse de tes notes…")}</p>{/if}
    {#if map && !nodes.length}
      <p class="empty">{t("Rien à cartographier : crée des concepts, des questions ou des chapitres et relie-les.")}</p>
    {:else if map && !map.nodes.some((n) => ["concept", "methode", "question"].includes(n.kind))}
      <p class="tip">{t("Ta carte ne contient encore que des chapitres. Crée des concepts (vue Concepts, ou « Concepts de cette note » avec l'IA), des notes de méthode ou des questions de recherche : elles s'y relieront d'elles-mêmes.")}</p>
    {/if}
    <svg width={w} height={h} role="img" aria-label={t("Carte des connaissances")}>
      {#each links as l, i (i)}
        {@const c = seg(l)}
        {#if c}
          <line x1={c[0]} y1={c[1]} x2={c[2]} y2={c[3]} class:mention={l.kind === "mention"} class:dim={related && !(related.has(c[4]) && related.has(c[5]))} />
        {/if}
      {/each}
      {#each nodes as n (n.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <g class="node" transform="translate({at(n)})" class:dim={related && !related.has(n.id)} onmouseenter={() => (hover = n.id)} onmouseleave={() => (hover = null)} onclick={() => app.openNote(n.id, { newTab: true })}>
          <path d={shape(n)} fill={color(n.kind)} />
          <text y={-r(n) - 6}>{short(n.title)}</text>
          <title>{n.title} · {KINDS.find((k) => k.id === n.kind)?.label ?? n.kind} · {tn(n.degree, "{n} relation", "{n} relations")}</title>
        </g>
      {/each}
    </svg>
  </div>
</div>

<style>
  .km {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px 4px;
    font-size: 13px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .lg {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    padding: 1px 9px;
    font: 12px var(--font-ui);
    color: var(--text);
    cursor: pointer;
  }
  .lg.off {
    opacity: 0.45;
    text-decoration: line-through;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
  }
  .hint {
    margin: 0;
    padding: 0 16px 6px;
    font-size: 12px;
    color: var(--text-soft);
    border-bottom: 1px solid var(--line);
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
  .tip {
    position: absolute;
    left: 16px;
    right: 16px;
    bottom: 14px;
    margin: 0;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg-soft);
    font-size: 12.5px;
    color: var(--text-soft);
  }
  line {
    stroke: var(--line-strong);
    stroke-width: 1.4;
  }
  line.mention {
    stroke-dasharray: 4 4;
  }
  .node {
    cursor: pointer;
  }
  .node path {
    stroke: var(--bg);
    stroke-width: 2;
  }
  text {
    font: 11.5px var(--font-ui);
    fill: var(--text);
    text-anchor: middle;
    pointer-events: none;
    paint-order: stroke;
    stroke: var(--bg);
    stroke-width: 3px;
  }
  .dim {
    opacity: 0.15;
  }
  .error {
    color: var(--danger);
    padding: 0 16px;
  }
</style>
