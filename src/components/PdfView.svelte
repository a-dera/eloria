<script lang="ts">
  import { assetUrl, isTauri } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import PdfAnnotator from "./PdfAnnotator.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  // `path`: absolute path of the file; `page`: physical page (1 = first).
  // `version`: changes on every recompilation (LaTeX preview) to reload the file.
  let {
    path,
    page,
    version = 0,
    onPick = undefined,
    target = undefined,
  }: { path: string; page?: number; version?: number; onPick?: (page: number, y: number) => void; target?: { page: number; y: number; n: number } | null } = $props();

  let src = $state("");
  $effect(() => {
    // WebView2's PDF reader (Edge's) understands #page=N.
    const target = path;
    const p = page;
    const v = version;
    assetUrl(target).then((u) => (src = u ? `${u}${v ? `?v=${v}` : ""}${p ? `#page=${p}` : ""}` : ""));
  });

  const name = $derived(path.split(/[\\/]/).pop() ?? path);
  /** Path in the vault (/ separator); empty if the PDF is elsewhere (Zotero folder). */
  const rel = $derived.by(() => {
    const root = app.vault?.root;
    if (!root) return "";
    const norm = (s: string) => s.replace(/\\/g, "/");
    const p = norm(path);
    const r = norm(root).replace(/\/$/, "") + "/";
    return p.toLowerCase().startsWith(r.toLowerCase()) ? p.slice(r.length) : "";
  });
  /** Annotatable reader (PDF.js); in mock mode, on a demo PDF. */
  let annUrl = $state("");
  $effect(() => {
    const target = path;
    if (!isTauri) annUrl = target.startsWith("/demo/") ? "/demo/exemple.pdf" : "";
    else assetUrl(target).then((u) => (annUrl = u));
  });
  /** WebView2's built-in reader if preferred (no annotation). */
  let native = $state(false);
</script>

{#if annUrl && !native && (version === 0 || onPick)}
  {#key annUrl + page + version}
    <!-- LaTeX preview (onPick): read-only, reloaded on every compilation. -->
    <PdfAnnotator url={version ? `${annUrl}?v=${version}` : annUrl} rel={onPick ? "" : rel} {page} {name} {onPick} {target} />
  {/key}
{:else}
<div class="pdf">
  <div class="bar">
    <Icon name="pdf" size={15} />
    <span class="name" title={path}>{name}</span>
    {#if page}<span class="faint">{t("page {n}", { n: page })}</span>{/if}
  </div>
  {#if isTauri && src}
    {#key src}
      <iframe title={name} {src}></iframe>
    {/key}
  {:else}
    <p class="empty">{t("Aperçu du PDF indisponible en mode démonstration (navigateur).")}</p>
  {/if}
</div>
{/if}

<style>
  .pdf {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
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
  }
  .faint {
    color: var(--text-faint);
  }
  iframe {
    flex: 1;
    width: 100%;
    border: 0;
    background: var(--bg-sunk);
  }
</style>
