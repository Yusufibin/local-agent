<script lang="ts">
  import type { ExtensionUiRequest } from "../protocol/events";
  import { onMount } from "svelte";

  let {
    request,
    onrespond,
  }: {
    request: ExtensionUiRequest;
    onrespond: (payload: Record<string, unknown>) => void;
  } = $props();

  let inputValue = $state("");
  $effect(() => {
    if (request.prefill) inputValue = request.prefill;
  });
  let dialogEl: HTMLDivElement | undefined;

  onMount(() => {
    const root = dialogEl;
    if (!root) return;
    const focusables = () =>
      Array.from(
        root.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        ),
      ).filter((el) => !el.hasAttribute("disabled"));
    focusables()[0]?.focus();
    function trap(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onrespond({ cancelled: true });
        return;
      }
      if (e.key !== "Tab") return;
      const list = focusables();
      if (list.length === 0) return;
      const first = list[0];
      const last = list[list.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
    root.addEventListener("keydown", trap);
    return () => root.removeEventListener("keydown", trap);
  });
</script>

<div class="modal-backdrop" data-testid="approval-modal">
  <div class="modal" bind:this={dialogEl} role="dialog" aria-modal="true" tabindex="-1">
    <h2>{request.title ?? request.method}</h2>
    {#if request.message}
      <pre style="white-space:pre-wrap">{request.message}</pre>
    {/if}
    {#if request.method === "select"}
      <div class="actions">
        {#each request.options ?? [] as opt}
          <button type="button" class="primary" onclick={() => onrespond({ value: opt })}>{opt}</button>
        {/each}
        <button type="button" onclick={() => onrespond({ cancelled: true })}>Cancel</button>
      </div>
    {:else if request.method === "confirm"}
      <div class="actions">
        <button type="button" onclick={() => onrespond({ confirmed: false })}>Block</button>
        <button type="button" class="primary" onclick={() => onrespond({ confirmed: true })}>Allow</button>
      </div>
    {:else if request.method === "input" || request.method === "editor"}
      <textarea rows={request.method === "editor" ? 8 : 2} bind:value={inputValue} placeholder={request.placeholder}></textarea>
      <div class="actions">
        <button type="button" onclick={() => onrespond({ cancelled: true })}>Cancel</button>
        <button type="button" class="primary" onclick={() => onrespond({ value: inputValue })}>OK</button>
      </div>
    {/if}
  </div>
</div>