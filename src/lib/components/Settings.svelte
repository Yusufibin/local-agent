<script lang="ts">
  let {
    permissionMode,
    onmode,
    onsecret,
    onworkspace,
    onroot,
    expert = false,
  }: {
    permissionMode: string;
    onmode: (mode: string) => void;
    onsecret: (provider: string, key: string) => void;
    onworkspace: () => void;
    onroot: () => void;
    expert?: boolean;
  } = $props();

  let provider = $state("anthropic");
  let key = $state("");
</script>

<div class="panel">
  <strong>Settings</strong>
  <div style="margin-top:0.5rem">
    <label class="muted" for="perm">Permission</label>
    <select id="perm" value={permissionMode} onchange={(e) => onmode((e.currentTarget as HTMLSelectElement).value)}>
      <option value="readonly">readonly</option>
      <option value="ask">ask</option>
      <option value="full">full</option>
    </select>
  </div>
  <div class="row" style="margin-top:0.5rem">
    <button type="button" onclick={onworkspace}>Workspace…</button>
    <button type="button" onclick={onroot}>Add root…</button>
  </div>
  <div class="secret-box">
    <label class="muted" for="prov">Provider key</label>
    <input id="prov" bind:value={provider} />
    <input type="password" bind:value={key} placeholder="API key (stored chmod 600)" style="margin-top:0.3rem" />
    <button
      type="button"
      style="margin-top:0.3rem"
      onclick={() => {
        if (key) {
          onsecret(provider, key);
          key = "";
        }
      }}>Save key</button
    >
  </div>
  {#if expert}
    <p class="muted">expertApprove is off unless set in settings.json</p>
  {/if}
</div>