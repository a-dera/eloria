<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import * as pdfjs from "pdfjs-dist";
  import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import "../lib/pdf-textlayer.css";
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import { t, tk, tn, translateError } from "../lib/i18n.svelte";
  import type { PdfAnnotation } from "../lib/types";
  import Icon from "./Icon.svelte";
  import { docText } from "../lib/pdftext";

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  // Annotatable PDF reader: highlight (colours), comment, page notes. The
  // annotations are saved in the vault and copied into the PDF's source.
  // Empty `rel`: PDF outside the vault, read-only.
  // `onPick`: double-click on a page (LaTeX preview: go to the source); `target`: point to
  // show (page, height in PDF points), replayed on each change of `n`.
  let {
    url,
    rel,
    page = undefined,
    name,
    onPick = undefined,
    target = undefined,
  }: { url: string; rel: string; page?: number; name: string; onPick?: (page: number, y: number) => void; target?: { page: number; y: number; n: number } | null } = $props();
  let flash = $state<{ page: number; y: number } | null>(null);
  $effect(() => {
    const tg = target;
    if (!tg || !sizes.length || !scroller) return;
    const el = pageEls[tg.page - 1];
    if (!el) return;
    scroller.scrollTop = el.offsetTop + tg.y * scale - scroller.clientHeight / 3;
    flash = { page: tg.page, y: tg.y };
    setTimeout(() => (flash = null), 1600);
  });
  function onDbl(e: MouseEvent, page: number) {
    if (!onPick) return;
    const box = pageEls[page - 1].getBoundingClientRect();
    onPick(page, (e.clientY - box.top) / scale);
  }

  const COLORS = [
    { id: "jaune", label: tk("Jaune"), css: "#ffd400" },
    { id: "vert", label: tk("Vert"), css: "#5fb236" },
    { id: "bleu", label: tk("Bleu"), css: "#2ea8e5" },
    { id: "rouge", label: tk("Rouge"), css: "#ff6666" },
    { id: "violet", label: tk("Violet"), css: "#a28ae5" },
  ];
  const cssOf = (c: string) => COLORS.find((x) => x.id === c)?.css ?? "#ffd400";

  let scroller = $state<HTMLDivElement>();
  let doc: pdfjs.PDFDocumentProxy | null = null;
  let task: ReturnType<typeof pdfjs.getDocument> | null = null;
  let numPages = $state(0);
  let sizes = $state<{ w: number; h: number }[]>([]);
  let scale = $state(1.25);
  let current = $state(1);
  let error = $state("");
  let annots = $state<PdfAnnotation[]>([]);
  let showSide = $state(true);
  let selected = $state<string | null>(null);
  /** Text selection waiting for a colour. */
  let pending = $state<{ page: number; rects: [number, number, number, number][]; text: string; x: number; y: number } | null>(null);
  const pageEls: HTMLDivElement[] = [];
  const rendered = new Map<number, number>(); // page -> rendered scale
  let observer: IntersectionObserver | null = null;
  /** "Figure" mode: frame an area of a page to add it to the source. */
  let figureMode = $state(false);
  let drag = $state<{ page: number; x0: number; y0: number; x1: number; y1: number } | null>(null);
  let capturing = $state(false);

  function figPoint(e: PointerEvent, page: number) {
    const box = pageEls[page - 1].getBoundingClientRect();
    return { x: Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)), y: Math.min(1, Math.max(0, (e.clientY - box.top) / box.height)) };
  }
  function figDown(e: PointerEvent, page: number) {
    const p = figPoint(e, page);
    drag = { page, x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function figMove(e: PointerEvent, page: number) {
    if (!drag || drag.page !== page) return;
    const p = figPoint(e, page);
    drag.x1 = p.x;
    drag.y1 = p.y;
  }
  async function figUp() {
    if (!drag) return;
    const d = drag;
    const rect = [Math.min(d.x0, d.x1), Math.min(d.y0, d.y1), Math.abs(d.x1 - d.x0), Math.abs(d.y1 - d.y0)];
    if (rect[2] < 0.03 || rect[3] < 0.02) {
      drag = null;
      return;
    }
    await captureFigure(d.page, rect as [number, number, number, number]);
    drag = null;
  }

  /** The area is rendered in high resolution, saved as PNG, then added to the source. */
  async function captureFigure(page: number, [x, y, w, h]: [number, number, number, number]) {
    if (!doc || !rel) return;
    const caption = await app.ask(t("Légende de la figure (facultatif)"), "");
    if (caption === null) return;
    capturing = true;
    try {
      const p = await doc.getPage(page);
      const vp = p.getViewport({ scale: 3 });
      const full = document.createElement("canvas");
      full.width = Math.floor(vp.width);
      full.height = Math.floor(vp.height);
      await p.render({ canvas: full, viewport: vp }).promise;
      const out = document.createElement("canvas");
      out.width = Math.max(1, Math.round(w * full.width));
      out.height = Math.max(1, Math.round(h * full.height));
      out.getContext("2d")!.drawImage(full, Math.round(x * full.width), Math.round(y * full.height), out.width, out.height, 0, 0, out.width, out.height);
      const data = out.toDataURL("image/png").split(",")[1];
      const stem = rel.split("/").pop()!.replace(/\.pdf$/i, "");
      const folder = `${app.settings?.attachments_folder || t("Pièces jointes")}/Figures`;
      const img = await api.saveAttachment(folder, `${stem} p${page}.png`, data);
      const updated = await api.attachFigure(rel, img, page, caption);
      if (updated.length) {
        await app.reloadOpen(updated);
        app.revision++;
        app.toast(t("Figure de la page {n} ajoutée à la source", { n: page }), "success");
      } else {
        await app.refresh();
        app.toast(t("Figure enregistrée : {path} (aucune source pour ce PDF)", { path: img }), "info");
      }
      figureMode = false;
    } catch (e) {
      app.toast(translateError(e), "error");
    } finally {
      capturing = false;
    }
  }

  async function load() {
    try {
      const data = await fetch(url).then((r) => {
        if (!r.ok) throw new Error(`${r.status}`);
        return r.arrayBuffer();
      });
      task = pdfjs.getDocument({ data });
      doc = await task.promise;
      numPages = doc.numPages;
      const s: { w: number; h: number }[] = [];
      for (let i = 1; i <= numPages; i++) {
        const vp = (await doc.getPage(i)).getViewport({ scale: 1 });
        s.push({ w: vp.width, h: vp.height });
      }
      sizes = s;
      // Narrow panel: the annotation list stays folded to keep the page readable.
      const total = scroller?.parentElement?.clientWidth ?? 900;
      showSide = total > 820;
      await tick();
      // Initial zoom: the page fits the available width.
      if (scroller && s[0]) scale = Math.min(1.6, Math.max(0.5, Math.floor(((scroller.clientWidth - 40) / s[0].w) * 20) / 20));
      if (rel) annots = await api.pdfAnnotations(rel);
      await tick();
      observe();
      if (page) goto(page);
      // First visit to this PDF: its text is kept for search and Ask Eloria.
      if (rel && doc && !(await api.hasPdfText(rel).catch(() => true))) {
        const d = doc;
        docText(d)
          .then((text) => api.storePdfText(rel, text))
          .then((done) => {
            if (done.length) app.toast(t("Résumé lu dans le PDF et ajouté à la source"), "success");
          })
          .catch(() => {});
      }
    } catch (e) {
      error = t("PDF illisible : {err}", { err: translateError(e) });
    }
  }

  function observe() {
    observer?.disconnect();
    observer = new IntersectionObserver(
      (entries) => {
        for (const en of entries) {
          const n = Number((en.target as HTMLElement).dataset.page);
          if (en.isIntersecting) {
            renderPage(n);
            if (en.intersectionRatio > 0.3) current = n;
          }
        }
      },
      { root: scroller, rootMargin: "400px 0px", threshold: [0, 0.3, 0.6] },
    );
    pageEls.forEach((el) => el && observer!.observe(el));
  }

  async function renderPage(n: number) {
    if (!doc || rendered.get(n) === scale) return;
    rendered.set(n, scale);
    const el = pageEls[n - 1];
    if (!el) return;
    const p = await doc.getPage(n);
    const vp = p.getViewport({ scale });
    const canvas = el.querySelector("canvas")!;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.floor(vp.width * ratio);
    canvas.height = Math.floor(vp.height * ratio);
    canvas.style.width = `${vp.width}px`;
    canvas.style.height = `${vp.height}px`;
    await p.render({ canvas, viewport: vp, transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined }).promise.catch(() => {});
    const layer = el.querySelector(".textLayer") as HTMLDivElement;
    layer.replaceChildren();
    layer.style.setProperty("--scale-factor", String(scale));
    layer.style.setProperty("--total-scale-factor", String(scale));
    await new pdfjs.TextLayer({ textContentSource: p.streamTextContent(), container: layer, viewport: vp }).render().catch(() => {});
  }

  function setScale(s: number) {
    scale = Math.min(3, Math.max(0.5, Math.round(s * 100) / 100));
    rendered.clear();
    tick().then(() => pageEls.forEach((el, i) => el && el.getBoundingClientRect().bottom > 0 && renderPage(i + 1)));
  }

  function goto(n: number) {
    pageEls[n - 1]?.scrollIntoView({ block: "start" });
    current = n;
  }

  /** Text selection in a page: offers the highlight colours. */
  function onMouseUp() {
    if (!rel) return;
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) return;
    const range = sel.getRangeAt(0);
    const pageEl = (range.commonAncestorContainer instanceof Element ? range.commonAncestorContainer : range.commonAncestorContainer.parentElement)?.closest(".pg") as HTMLDivElement | null;
    if (!pageEl) return;
    const text = sel.toString().trim();
    if (!text) return;
    const box = pageEl.getBoundingClientRect();
    const rects = [...range.getClientRects()]
      .filter((r) => r.width > 1 && r.height > 1)
      .map((r) => [(r.left - box.left) / box.width, (r.top - box.top) / box.height, r.width / box.width, r.height / box.height] as [number, number, number, number]);
    if (!rects.length) return;
    const last = range.getClientRects()[range.getClientRects().length - 1];
    const host = scroller!.getBoundingClientRect();
    pending = { page: Number(pageEl.dataset.page), rects: mergeRects(rects), text, x: last.right - host.left + scroller!.scrollLeft, y: last.bottom - host.top + scroller!.scrollTop + 6 };
  }

  /** Merges the pieces of a same line (the text layer splits by word). */
  function mergeRects(rs: [number, number, number, number][]) {
    const out: [number, number, number, number][] = [];
    for (const r of rs.sort((a, b) => a[1] - b[1] || a[0] - b[0])) {
      const prev = out[out.length - 1];
      if (prev && Math.abs(prev[1] - r[1]) < 0.006 && r[0] <= prev[0] + prev[2] + 0.02) {
        const right = Math.max(prev[0] + prev[2], r[0] + r[2]);
        prev[2] = right - prev[0];
        prev[3] = Math.max(prev[3], r[3]);
      } else out.push([...r]);
    }
    return out;
  }

  const uid = () => `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

  async function persist() {
    if (!rel) return;
    try {
      const updated = await api.savePdfAnnotations(rel, $state.snapshot(annots) as PdfAnnotation[]);
      if (updated.length) {
        await app.reloadOpen(updated);
        app.revision++;
      }
    } catch (e) {
      app.toast(translateError(e), "error");
    }
  }

  function addHighlight(color: string, comment = false) {
    if (!pending) return;
    const a: PdfAnnotation = { id: uid(), page: pending.page, kind: "highlight", color, text: pending.text, comment: "", rects: pending.rects, created: new Date().toISOString() };
    annots = [...annots, a];
    pending = null;
    window.getSelection()?.removeAllRanges();
    selected = a.id;
    persist();
    if (comment) {
      showSide = true;
      tick().then(() => (document.getElementById(`c-${a.id}`) as HTMLTextAreaElement | null)?.focus());
    }
  }

  async function addPageNote() {
    if (!rel) return;
    const text = await app.ask(t("Note sur la page {n}", { n: current }), "");
    if (!text) return;
    annots = [...annots, { id: uid(), page: current, kind: "note", color: "bleu", text: "", comment: text, rects: [], created: new Date().toISOString() }];
    persist();
  }

  function remove(id: string) {
    annots = annots.filter((a) => a.id !== id);
    persist();
  }

  function setComment(id: string, comment: string) {
    const a = annots.find((x) => x.id === id);
    if (!a || a.comment === comment) return;
    a.comment = comment;
    persist();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      pending = null;
      figureMode = false;
      drag = null;
    }
  }

  onMount(load);
  // Text selection ends when the mouse is released in the page.
  $effect(() => {
    const el = scroller;
    if (!el) return;
    el.addEventListener("mouseup", onMouseUp);
    return () => el.removeEventListener("mouseup", onMouseUp);
  });
  onDestroy(() => {
    observer?.disconnect();
    task?.destroy();
  });

  const sorted = $derived([...annots].sort((a, b) => a.page - b.page || (a.rects[0]?.[1] ?? 0) - (b.rects[0]?.[1] ?? 0)));
</script>

<svelte:window onkeydown={onKey} />

<div class="pa">
  <div class="bar">
    <Icon name="pdf" size={15} />
    <span class="name" title={name}>{name}</span>
    {#if numPages}<span class="faint">{t("page {n} sur {total}", { n: current, total: numPages })}</span>{/if}
    <span class="grow"></span>
    <button class="icon-btn" title={t("Réduire")} aria-label={t("Réduire")} onclick={() => setScale(scale - 0.15)}>−</button>
    <span class="faint">{Math.round(scale * 100)} %</span>
    <button class="icon-btn" title={t("Agrandir")} aria-label={t("Agrandir")} onclick={() => setScale(scale + 0.15)}>+</button>
    {#if rel}
      <button class="btn small" class:on={figureMode} aria-pressed={figureMode} title={t("Encadre une figure dans une page : elle est ajoutée à la source du document")} onclick={() => (figureMode = !figureMode)}>{capturing ? t("Capture…") : t("Figure")}</button>
      <button class="btn small" onclick={addPageNote}>{t("Note sur la page")}</button>
      <button class="btn small ghost" class:on={showSide} onclick={() => (showSide = !showSide)}>{tn(annots.length, "{n} annotation", "{n} annotations")}</button>
    {:else}
      {#if onPick}
        <span class="faint hint" title={t("Double-clic dans le PDF : la ligne correspondante du source")}>{t("double-clic : aller au source")}</span>
      {:else}
        <span class="faint" title={t("Les PDF du coffre s'annotent ; celui-ci est dans le dossier de Zotero.")}>{t("lecture seule")}</span>
      {/if}
    {/if}
  </div>
  <div class="body">
    <div class="scroll" bind:this={scroller} role="document">
      {#if error}<p class="error">{error}</p>{/if}
      {#each sizes as s, i (i)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="pg" data-page={i + 1} bind:this={pageEls[i]} style:width="{s.w * scale}px" style:height="{s.h * scale}px" ondblclick={(e) => onDbl(e, i + 1)}>
          {#if flash && flash.page === i + 1}<div class="syncmark" style:top="{flash.y * scale - 14}px"></div>{/if}
          <canvas></canvas>
          <div class="hl">
            {#each annots.filter((a) => a.page === i + 1 && a.kind === "highlight") as a (a.id)}
              {#each a.rects as r, k (k)}
                <button
                  class="mark"
                  class:sel={selected === a.id}
                  style:left="{r[0] * 100}%"
                  style:top="{r[1] * 100}%"
                  style:width="{r[2] * 100}%"
                  style:height="{r[3] * 100}%"
                  style:background={cssOf(a.color)}
                  title={a.comment || a.text}
                  aria-label={t("Surlignage page {n}", { n: a.page })}
                  onclick={() => {
                    selected = a.id;
                    showSide = true;
                  }}
                ></button>
              {/each}
            {/each}
          </div>
          <div class="textLayer"></div>
          {#if figureMode}
            <div
              class="figlayer"
              role="presentation"
              onpointerdown={(e) => figDown(e, i + 1)}
              onpointermove={(e) => figMove(e, i + 1)}
              onpointerup={figUp}
            ></div>
          {/if}
          {#if drag && drag.page === i + 1}
            <div class="figrect" style:left="{Math.min(drag.x0, drag.x1) * 100}%" style:top="{Math.min(drag.y0, drag.y1) * 100}%" style:width="{Math.abs(drag.x1 - drag.x0) * 100}%" style:height="{Math.abs(drag.y1 - drag.y0) * 100}%"></div>
          {/if}
          {#if annots.some((a) => a.page === i + 1 && a.kind === "note")}<span class="pnote" title={t("Note sur la page")}>✎</span>{/if}
        </div>
      {/each}
      {#if pending}
        <div class="pop" style:left="{pending.x}px" style:top="{pending.y}px" role="toolbar" aria-label={t("Surligner")}>
          {#each COLORS as c (c.id)}
            <button class="dot" style:background={c.css} title={t(c.label)} aria-label={t("Surligner en {c}", { c: t(c.label) })} onclick={() => addHighlight(c.id)}></button>
          {/each}
          <button class="btn small ghost" onclick={() => addHighlight("jaune", true)}>{t("Commenter")}</button>
        </div>
      {/if}
    </div>
    {#if rel && showSide}
      <aside class="side">
        {#each sorted as a (a.id)}
          <div class="ann" class:sel={selected === a.id}>
            <div class="ah">
              <span class="cdot" style:background={cssOf(a.color)}></span>
              <button class="go" onclick={() => { selected = a.id; goto(a.page); }}>p. {a.page}{a.kind === "note" ? ` · ${t("note")}` : ""}</button>
              <span class="grow"></span>
              <button class="icon-btn" title={t("Supprimer")} aria-label={t("Supprimer l'annotation")} onclick={() => remove(a.id)}><Icon name="trash" size={13} /></button>
            </div>
            {#if a.text}<blockquote>{a.text}</blockquote>{/if}
            <textarea id="c-{a.id}" rows="2" placeholder={t("Commentaire…")} value={a.comment} onchange={(e) => setComment(a.id, (e.target as HTMLTextAreaElement).value)}></textarea>
          </div>
        {:else}
          <p class="faint small">{t("Sélectionne du texte dans le PDF pour le surligner. Les annotations sont enregistrées dans le coffre et recopiées dans la source.")}</p>
        {/each}
      </aside>
    {/if}
  </div>
</div>

<style>
  .pa {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  /* Optional hint: shrinks first when the bar runs out of space. */
  .bar .hint {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
    height: 38px;
    padding: 0 10px 0 14px;
    color: var(--text-soft);
    font-size: 12.5px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--line);
  }
  .name {
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex-shrink: 1;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .small {
    font-size: 12px;
  }
  .on {
    color: var(--accent-text);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    position: relative;
    background: var(--bg-sunk);
    padding: 16px 0;
  }
  .pg {
    position: relative;
    margin: 0 auto 14px;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
  }
  .pg canvas {
    position: absolute;
    inset: 0;
  }
  .hl {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .mark {
    position: absolute;
    border: 0;
    padding: 0;
    opacity: 0.38;
    mix-blend-mode: multiply;
    pointer-events: auto;
    cursor: pointer;
    border-radius: 2px;
  }
  .mark.sel {
    outline: 2px solid var(--accent);
    opacity: 0.5;
  }
  .pg :global(.textLayer) {
    position: absolute;
    inset: 0;
    z-index: 1;
  }
  .hl {
    z-index: 2;
  }
  .pnote {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 3;
    color: var(--accent);
  }
  .pop {
    position: absolute;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow, 0 4px 14px rgba(0, 0, 0, 0.2));
    transform: translateX(-50%);
  }
  .dot {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid rgba(0, 0, 0, 0.15);
    cursor: pointer;
  }
  .side {
    width: 280px;
    flex-shrink: 0;
    overflow: auto;
    border-left: 1px solid var(--line);
    padding: 10px;
    background: var(--bg-soft);
  }
  .ann {
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px;
    margin-bottom: 8px;
    font-size: 12.5px;
  }
  .ann.sel {
    border-color: var(--accent);
  }
  .ah {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cdot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .go {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font: 600 12px var(--font-ui);
    cursor: pointer;
  }
  blockquote {
    margin: 6px 0;
    padding-left: 8px;
    border-left: 2px solid var(--line-strong);
    color: var(--text-soft);
  }
  textarea {
    width: 100%;
    resize: vertical;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text);
    font: 12.5px var(--font-ui);
    padding: 4px 6px;
  }
  .error {
    color: var(--danger);
    padding: 16px;
  }
  .figlayer {
    position: absolute;
    inset: 0;
    z-index: 5;
    cursor: crosshair;
  }
  .figrect {
    position: absolute;
    z-index: 6;
    border: 2px dashed var(--accent);
    background: var(--accent-soft);
    pointer-events: none;
  }
  .syncmark {
    position: absolute;
    left: 0;
    right: 0;
    height: 18px;
    z-index: 4;
    background: var(--accent-soft);
    border-left: 3px solid var(--accent);
    pointer-events: none;
    animation: syncfade 1.6s ease-out forwards;
  }
  @keyframes syncfade {
    to {
      opacity: 0;
    }
  }
</style>
