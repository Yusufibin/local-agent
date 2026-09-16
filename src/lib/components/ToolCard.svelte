<script lang="ts">
  import type { ToolCard } from "../store/session";
  import { MAX_TOOL_BODY } from "../store/session";
  let { card }: { card: ToolCard } = $props();
  let open = $state(false);
  let body = $derived(
    card.body.length > MAX_TOOL_BODY ? card.body.slice(0, MAX_TOOL_BODY) + "\n…" : card.body,
  );
</script>

<div class="tool-card" class:error={card.isError} data-testid="tool-card">
  <div class="row">
    <strong>{card.toolName}</strong>
    <span class="muted">{card.status}</span>
    {#if card.isError}<span class="muted" style="color:var(--danger)">error</span>{/if}
    <button type="button" onclick={() => (open = !open)}>{open ? "hide args" : "args"}</button>
  </div>
  {#if open}
    <pre>{JSON.stringify(card.args, null, 2)}</pre>
  {/if}
  {#if card.diff || card.patch}
    <pre>{card.diff || card.patch}</pre>
  {:else if body}
    <pre>{body}</pre>
  {/if}
</div>