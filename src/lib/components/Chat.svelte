<script lang="ts">
  import type { SessionState, ToolCard } from "../store/session";
  import { liveAssistantText, visibleMessages } from "../store/session";
  import Message from "./Message.svelte";

  let { session }: { session: SessionState } = $props();
  let extra = $state(0);

  function cardsFor(): ToolCard[] {
    return Object.values(session.toolCards);
  }

  let windowed = $derived(visibleMessages(session.messages, extra));
</script>

<div class="transcript" data-testid="transcript">
  {#each session.banners as b}
    <div class="banner">
      {#if b === "compaction"}Compacting context…
      {:else if b === "retry"}Retrying…
      {:else if b === "restarting"}Restarting sidecar…
      {:else}{b}{/if}
    </div>
  {/each}
  {#if session.missingPi}
    <div class="banner">Installe Pi : {session.missingPi}</div>
  {/if}
  {#each Object.values(session.widgets) as lines}
    <pre class="muted">{lines.join("\n")}</pre>
  {/each}
  {#if windowed.hidden > 0}
    <button
      type="button"
      class="muted"
      data-testid="load-earlier"
      onclick={() => (extra += 50)}
    >
      Load earlier ({windowed.hidden} hidden)
    </button>
  {/if}
  {#each windowed.slice as message, i}
    <Message
      {message}
      extraText={i === windowed.slice.length - 1 && session.runState === "running"
        ? liveAssistantText(session)
        : ""}
      cards={i === windowed.slice.length - 1 ? cardsFor() : []}
    />
  {/each}
  {#if session.messages.length === 0}
    <div class="empty-hero">
      <h1>Local Agent</h1>
      <p class="muted">Pick a workspace, then send a prompt. LLM calls stay inside the Pi sidecar.</p>
    </div>
  {/if}
</div>