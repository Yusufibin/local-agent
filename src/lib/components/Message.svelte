<script lang="ts">
  import type { ContentBlock, TranscriptMessage, ToolCard } from "../store/session";
  import ToolCardView from "./ToolCard.svelte";

  let {
    message,
    extraText = "",
    cards = [],
  }: {
    message: TranscriptMessage;
    extraText?: string;
    cards?: ToolCard[];
  } = $props();

  function blockText(b: ContentBlock): string {
    if (b.type === "text") return b.text;
    if (b.type === "thinking") return b.thinking;
    return "";
  }
</script>

<article class="msg {message.role}" data-testid="message">
  <div class="role">{message.role}</div>
  <div class="bubble">
    {#each message.content as block}
      {#if block.type === "thinking"}
        <details><summary>thinking</summary><pre>{block.thinking}</pre></details>
      {:else if block.type === "text"}
        <div>{block.text}</div>
      {/if}
    {/each}
    {#if extraText}<div>{extraText}</div>{/if}
    {#each cards as card}
      <ToolCardView {card} />
    {/each}
  </div>
</article>