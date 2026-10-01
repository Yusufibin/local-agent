<script lang="ts">
  import { inline } from "../markdown";
  let { text }: { text: string } = $props();
</script>
{#each inline(text) as token}
  {#if token.kind === "code"}<code>{token.text}</code>
  {:else if token.kind === "strong"}<strong>{token.text}</strong>
  {:else if token.kind === "em"}<em>{token.text}</em>
  {:else if token.kind === "link"}<a href={token.href} target="_blank" rel="noopener noreferrer">{token.text}</a>
  {:else}{token.text}{/if}
{/each}
