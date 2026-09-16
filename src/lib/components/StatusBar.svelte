<script lang="ts">
  let {
    modelId,
    tokensPercent,
    sessionCost,
    permissionMode,
    activeRoot,
    extras = {},
    expanded = false,
  }: {
    modelId: string | null;
    tokensPercent: number | null;
    sessionCost: number | null;
    permissionMode: string;
    activeRoot: string | null;
    extras?: Record<string, string>;
    expanded?: boolean;
  } = $props();
</script>

<footer class="status" data-testid="status">
  <span class="status-model">{modelId ?? "no model"}</span>
  <span class="status-cost">{sessionCost != null ? `$${sessionCost.toFixed(4)}` : "$—"}</span>
  {#if expanded}
    <span>{tokensPercent != null ? `${tokensPercent}%` : "tokens —"}</span>
    <span>{permissionMode}</span>
    <span class="grow" title={activeRoot ?? ""}>{activeRoot ?? "no root"}</span>
  {/if}
  {#each Object.entries(extras) as [k, v]}
    <span>{k}: {v}</span>
  {/each}
</footer>