<script lang="ts">
  let {
    value = $bindable(""),
    running = false,
    onsubmit,
    onabort,
  }: {
    value?: string;
    running?: boolean;
    onsubmit: (kind: "enter" | "alt-enter") => void;
    onabort: () => void;
  } = $props();

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onabort();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      onsubmit(e.altKey ? "alt-enter" : "enter");
    }
  }
</script>

<div class="composer-wrap">
  <div class="composer-card">
    <label class="sr-only" for="composer-input">{running ? "steer / Alt+Enter follow-up" : "message"}</label>
    <textarea
      id="composer-input"
      data-testid="composer-input"
      rows="3"
      bind:value
      onkeydown={keydown}
      placeholder={running ? "Steer the agent… (Alt+Enter queues a follow-up)" : "Ask the local agent…"}
    ></textarea>
    <div class="composer-actions">
      <span class="muted grow">Enter send · Esc abort</span>
      <button type="button" class="danger" onclick={onabort}>Abort</button>
      <button type="button" class="primary" onclick={() => onsubmit("enter")}>
        {running ? "Steer" : "Send"}
      </button>
    </div>
  </div>
</div>