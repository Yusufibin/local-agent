<script lang="ts">
  import type { TranscriptMessage, ToolCard } from "../store/session";
  import ToolCardView from "./ToolCard.svelte";
  import Markdown from "./Markdown.svelte";
  let { message, extraText = "", cards = [] }: {
    message: TranscriptMessage; extraText?: string; cards?: ToolCard[];
  } = $props();
</script>
<article class="msg {message.role}" data-testid="message">
  <div class="role">{message.role}</div>
  <div class="bubble">
    {#each message.content as block}
      {#if block.type === "thinking"}
        <details><summary>Thinking</summary><pre>{block.thinking}</pre></details>
      {:else if block.type === "text"}
        <Markdown text={block.text} />
      {/if}
    {/each}
    {#if extraText}<Markdown text={extraText} />{/if}
    {#each cards as card (card.toolCallId)}<ToolCardView {card} />{/each}
  </div>
</article>
