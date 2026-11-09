<script lang="ts">
  import { t } from "../lib/i18n.svelte";

  // Generic resize handle. While dragging, a veil covers the whole
  // screen: PDFs (iframe, canvas) can no longer capture the mouse and make
  // the handle "let go". Double-click: default size. Arrows: keyboard adjustment.
  let {
    orientation = "vertical",
    value,
    min,
    max,
    defaultValue,
    invert = false,
    onchange,
    oncollapse,
    collapseBelow,
    onend,
    label,
  }: {
    /** vertical: vertical handle setting a width; horizontal: a height */
    orientation?: "vertical" | "horizontal";
    value: number;
    min: number;
    max: number;
    defaultValue: number;
    /** The size grows when dragging to the left (or up) */
    invert?: boolean;
    onchange: (v: number) => void;
    /** Dragged below `collapseBelow`: the panel collapses */
    oncollapse?: () => void;
    collapseBelow?: number;
    onend?: () => void;
    label?: string;
  } = $props();

  let dragging = $state(false);
  /** Time of the last click without movement (to recognise a double-click). */
  let lastClick = 0;

  function start(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    // Double-click: default size. Detected here (two close presses): the veil
    // laid during the drag captures the second click, and `detail` is 0 on a pointer.
    if (Date.now() - lastClick < 400) {
      lastClick = 0;
      onchange(defaultValue);
      onend?.();
      return;
    }
    const start = orientation === "vertical" ? e.clientX : e.clientY;
    const startV = value;
    dragging = true;
    document.body.classList.add(orientation === "vertical" ? "resizing-x" : "resizing-y");
    const move = (ev: PointerEvent) => {
      const pos = orientation === "vertical" ? ev.clientX : ev.clientY;
      const d = (pos - start) * (invert ? -1 : 1);
      const raw = startV + d;
      if (oncollapse && collapseBelow && raw < collapseBelow) return;
      onchange(Math.max(min, Math.min(max, raw)));
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      dragging = false;
      document.body.classList.remove("resizing-x", "resizing-y");
      const pos = orientation === "vertical" ? ev.clientX : ev.clientY;
      lastClick = Math.abs(pos - start) < 3 ? Date.now() : 0;
      const raw = startV + (pos - start) * (invert ? -1 : 1);
      if (oncollapse && collapseBelow && raw < collapseBelow) oncollapse();
      onend?.();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 10;
    const less = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
    const more = orientation === "vertical" ? "ArrowRight" : "ArrowDown";
    if (e.key !== less && e.key !== more) return;
    e.preventDefault();
    const d = (e.key === more ? step : -step) * (invert ? -1 : 1);
    onchange(Math.max(min, Math.min(max, value + d)));
    onend?.();
  }
</script>

<!-- A focusable separator with a value is a control per ARIA (keyboard arrows). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="resizer {orientation}"
  class:dragging
  role="separator"
  tabindex="0"
  aria-orientation={orientation}
  aria-valuenow={Math.round(value)}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-label={label ?? t("Redimensionner le panneau")}
  title={t("Glisser pour redimensionner · double-clic : taille par défaut")}
  onpointerdown={start}
  onkeydown={key}
></div>
{#if dragging}<div class="veil {orientation}"></div>{/if}

<style>
  .resizer {
    position: relative;
    z-index: 5;
    flex-shrink: 0;
    outline: none;
  }
  .resizer.vertical {
    width: 7px;
    margin: 0 -3px;
    cursor: col-resize;
  }
  .resizer.horizontal {
    height: 7px;
    margin: -3px 0;
    cursor: row-resize;
  }
  .resizer::after {
    content: "";
    position: absolute;
    background: var(--line);
    transition: background 0.15s;
  }
  .resizer.vertical::after {
    inset: 0 3px;
  }
  .resizer.horizontal::after {
    inset: 3px 0;
  }
  .resizer:hover::after,
  .resizer:focus-visible::after,
  .resizer.dragging::after {
    background: var(--accent);
  }
  .veil {
    position: fixed;
    inset: 0;
    z-index: 9999;
  }
  .veil.vertical {
    cursor: col-resize;
  }
  .veil.horizontal {
    cursor: row-resize;
  }
</style>
