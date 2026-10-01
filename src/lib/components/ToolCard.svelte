<script lang="ts">
  import type { ToolCard } from "../store/session";
  let { card }: { card: ToolCard } = $props();
  let open = $state(false);
  const diff = $derived(card.diff || card.patch || "");
</script>
<div class="tool-card" class:error={card.isError} data-testid="tool-card">
  <div class="row">
    <strong>{card.toolName}</strong><span class="muted">{card.status}</span>
    {#if card.isError}<span class="muted">Error</span>{/if}
    <button type="button" aria-expanded={open} onclick={() => (open = !open)}>{open ? "Hide arguments" : "Arguments"}</button>
  </div>
  {#if open}<pre>{JSON.stringify(card.args, null, 2)}</pre>{/if}
  {#if diff}
    <pre class="diff" aria-label="File changes">{#each diff.split("\n") as line}<span class:added={line.startsWith("+")} class:removed={line.startsWith("-")}>{line + "\n"}</span>{/each}</pre>
  {:else if card.body}<pre>{card.body}</pre>{/if}
</div>
