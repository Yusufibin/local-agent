<script lang="ts">
  let {
    models = [],
    modelId = null,
    thinking = "off",
    onmodel,
    onthinking,
  }: {
    models?: { id?: string; provider?: string; name?: string }[];
    modelId?: string | null;
    thinking?: string | null;
    onmodel: (provider: string, id: string) => void;
    onthinking: (level: string) => void;
  } = $props();

  const levels = ["off", "minimal", "low", "medium", "high"];
</script>

<div class="panel">
  <label class="muted" for="model-select">Model</label>
  <select
    id="model-select"
    value={modelId ?? ""}
    onchange={(e) => {
      const v = (e.currentTarget as HTMLSelectElement).value;
      const m = models.find((x) => x.id === v);
      if (m?.id) onmodel(String(m.provider ?? ""), m.id);
    }}
  >
    <option value="">—</option>
    {#each models as m}
      <option value={m.id}>{m.provider}/{m.id}</option>
    {/each}
  </select>
  <label class="muted" for="think-select" style="margin-top:0.4rem; display:block">Thinking</label>
  <select
    id="think-select"
    value={thinking ?? "off"}
    onchange={(e) => onthinking((e.currentTarget as HTMLSelectElement).value)}
  >
    {#each levels as l}
      <option value={l}>{l}</option>
    {/each}
  </select>
</div>