<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let promptInput = $state<HTMLInputElement>();
  let menuEl = $state<HTMLDivElement>();
  let menuPos = $state({ x: 0, y: 0 });

  // The field gets the focus as soon as it exists: a fast keystroke (Enter) goes to it.
  $effect(() => {
    if (app.prompt && promptInput) {
      promptInput.focus();
      promptInput.select();
    }
  });

  // Keeps the menu inside the window
  $effect(() => {
    if (!app.menu) return;
    menuPos = { x: app.menu.x, y: app.menu.y };
    requestAnimationFrame(() => {
      if (!menuEl || !app.menu) return;
      const r = menuEl.getBoundingClientRect();
      menuPos = {
        x: Math.min(app.menu.x, window.innerWidth - r.width - 8),
        y: Math.min(app.menu.y, window.innerHeight - r.height - 8),
      };
    });
  });

  function answerPrompt(v: string | null) {
    const p = app.prompt;
    app.prompt = null;
    p?.resolve(v === null ? null : v.trim() || null);
  }

  function answerConfirm(v: boolean) {
    const c = app.confirmBox;
    app.confirmBox = null;
    c?.resolve(v);
  }
</script>

<svelte:window
  onmousedown={(e) => {
    if (app.menu && !(e.target as HTMLElement).closest(".menu")) app.menu = null;
  }}
  onkeydown={(e) => {
    if (e.key !== "Escape") return;
    // Esc cancels the confirmation, like "Annuler" (and like other windows).
    if (app.confirmBox) {
      e.preventDefault();
      e.stopPropagation();
      answerConfirm(false);
    } else if (app.menu) app.menu = null;
  }}
  onblur={() => (app.menu = null)}
/>

{#if app.prompt}
  {@const p = app.prompt}
  <div class="backdrop" role="presentation" onmousedown={() => answerPrompt(null)}>
    <div class="dialog-wrap" role="dialog" aria-modal="true" aria-label={p.title} tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
    <form
      class="dialog"
      onsubmit={(e) => {
        e.preventDefault();
        answerPrompt(p.value);
      }}
    >
      <label class="dhead" for="prompt-input">{p.title}</label>
      <div class="dbody">
      <input
        id="prompt-input"
        class="input"
        bind:this={promptInput}
        bind:value={p.value}
        placeholder={p.placeholder}
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") answerPrompt(null);
        }}
      />
      </div>
      <div class="row">
        <button type="button" class="btn" onclick={() => answerPrompt(null)}>{t("Annuler")}</button>
        <button type="submit" class="btn primary">{t("Valider")}</button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if app.confirmBox}
  {@const c = app.confirmBox}
  <div class="backdrop" role="presentation" onmousedown={() => answerConfirm(false)}>
    <div class="dialog" role="alertdialog" aria-modal="true" aria-label={c.title} tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <h3 class="dhead">{c.title}</h3>
      <div class="dbody"><p>{c.message}</p></div>
      <div class="row">
        <button class="btn" onclick={() => answerConfirm(false)}>{t("Annuler")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn" class:primary={!c.danger} class:danger-solid={c.danger} autofocus onclick={() => answerConfirm(true)}>
          {c.confirmLabel ?? (c.danger ? t("Supprimer") : t("Confirmer"))}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if app.menu}
  <div class="menu" bind:this={menuEl} style:left="{menuPos.x}px" style:top="{menuPos.y}px" role="menu">
    {#each app.menu.items as it, i (i)}
      {#if it.separator}
        <div class="msep"></div>
      {:else}
        <button
          class="mi"
          class:danger={it.danger}
          role="menuitem"
          disabled={it.disabled}
          onclick={() => {
            app.menu = null;
            it.run?.();
          }}
        >
          <span class="mico">{#if it.icon}<Icon name={it.icon} size={15} />{/if}</span>
          {it.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<div class="toasts" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast {toast.kind}">
      <span class="tt">{toast.text}</span>
      {#if toast.action}
        <button
          class="btn small"
          onclick={() => {
            toast.action!.run();
            app.dismiss(toast.id);
          }}>{toast.action.label}</button
        >
      {/if}
      <button class="icon-btn" aria-label={t("Fermer")} onclick={() => app.dismiss(toast.id)}><Icon name="x" size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgb(20 15 30 / 22%);
    display: grid;
    place-items: center;
  }
  .dialog-wrap {
    display: contents;
  }
  .dialog {
    width: min(420px, 92vw);
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  /* Same frame as the other windows (Modal): title, content and footer separated. */
  .dialog .dhead {
    font-weight: 600;
    font-size: 15px;
    margin: 0;
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--line);
  }
  .dbody {
    padding: 16px 20px;
  }
  .dialog p {
    margin: 0;
    color: var(--text-soft);
    line-height: 1.55;
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
  }
  .danger-solid {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .menu {
    position: fixed;
    z-index: 300;
    min-width: 210px;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 4px;
  }
  .mi {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    border: 0;
    background: transparent;
    padding: 6px 10px;
    border-radius: 5px;
    text-align: left;
  }
  .mi:hover:not(:disabled) {
    background: var(--accent-soft);
  }
  .mi.danger {
    color: var(--danger);
  }
  .mico {
    width: 16px;
    display: inline-grid;
    color: var(--text-faint);
  }
  .mi.danger .mico {
    color: inherit;
  }
  .msep {
    height: 1px;
    background: var(--line);
    margin: 4px 6px;
  }
  .toasts {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: 38px;
    z-index: 400;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    max-width: min(560px, calc(100vw - 32px));
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 440px;
    padding: 8px 8px 8px 14px;
    background: var(--bg-raised);
    border: 1px solid var(--line);
    border-left: 3px solid var(--text-faint);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    animation: rise 0.18s var(--ease);
  }
  .toast.success {
    border-left-color: var(--success);
  }
  .toast.error {
    border-left-color: var(--danger);
  }
  .toast.info {
    border-left-color: var(--accent);
  }
  .tt {
    flex: 1;
    line-height: 1.45;
    user-select: text;
  }
  @keyframes rise {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
</style>
