<script lang="ts">
  let {
    sessions = [],
    onnew,
    onopen,
  }: {
    sessions?: { path: string; id?: string; name?: string; timestamp?: string }[];
    onnew: () => void;
    onopen: (path: string) => void;
  } = $props();
</script>

<div class="panel" data-testid="sessions">
  <div class="row">
    <strong class="grow">Sessions</strong>
    <button type="button" class="primary" onclick={onnew}>New</button>
  </div>
  <ul class="list" style="margin-top:0.5rem; max-height: 40vh;">
    {#each sessions as s}
      <li>
        <button type="button" onclick={() => onopen(s.path)}>
          {s.name || s.id || s.path.split("/").pop()}
          <div class="muted">{s.timestamp ?? ""}</div>
        </button>
      </li>
    {:else}
      <li class="muted">No sessions yet</li>
    {/each}
  </ul>
</div>