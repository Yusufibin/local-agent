<script lang="ts">
  import type { SessionState, ToolCard, TranscriptMessage } from "../store/session";
  import { liveAssistantText } from "../store/session";
  import Message from "./Message.svelte";
  let { session, historical = false, hasEarlier = false, loading = false, onearlier, onlatest }: {
    session: SessionState; historical?: boolean; hasEarlier?: boolean; loading?: boolean;
    onearlier?: () => void; onlatest?: () => void;
  } = $props();
  function cardsFor(message: TranscriptMessage): ToolCard[] {
    const ids = new Set(message.content.filter((block) => block.type === "toolCall").map((block) => block.id));
    return Object.values(session.toolCards).filter((card) => card.messageId === message.id || ids.has(card.toolCallId));
  }
</script>

<div class="transcript" data-testid="transcript" aria-busy={loading}>
  {#if historical}
    <div class="banner" role="status">
      <span class="grow">Viewing earlier messages</span>
      <button type="button" disabled={loading} onclick={onlatest}>Back to latest</button>
    </div>
  {/if}
  {#each session.banners as banner}
    <div class="banner" role="status">
      {#if banner === "compaction"}Compacting context…
      {:else if banner === "retry"}Retrying…
      {:else if banner === "restarting"}Restarting agent…
      {:else}{banner}{/if}
    </div>
  {/each}
  {#if session.missingPi}<div class="banner">Install Pi: {session.missingPi}</div>{/if}
  {#each Object.values(session.widgets) as lines}<pre class="muted">{lines.join("\n")}</pre>{/each}
  {#if hasEarlier}
    <button type="button" class="muted" data-testid="load-earlier" disabled={loading} onclick={onearlier}>
      {loading ? "Loading…" : "Load earlier messages"}
    </button>
  {/if}
  {#each session.messages as message (message.id)}
    {#if message.role !== "toolResult" || cardsFor(message).length}
      <Message {message} cards={cardsFor(message)}
        extraText={message.id === session.currentMessageId ? liveAssistantText(session) : ""} />
    {/if}
  {/each}
  {#if session.messages.length === 0}
    <div class="empty-hero"><h1>Local Agent</h1><p class="muted">Choose a workspace and a model, then send your first message.</p></div>
  {/if}
</div>
