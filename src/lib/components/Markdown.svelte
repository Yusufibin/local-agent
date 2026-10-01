<script lang="ts">
  import { markdown } from "../markdown";
  import Inline from "./Inline.svelte";
  let { text }: { text: string } = $props();
  let copyStatus = $state("");
  async function copy(code: string) {
    try { await navigator.clipboard.writeText(code); copyStatus = "Copied"; }
    catch { copyStatus = "Unable to copy. Select the code to copy it."; }
  }
</script>
<div class="markdown">
  {#each markdown(text) as block}
    {#if block.kind === "code"}
      <div class="code-block">
        <div class="row"><span class="muted grow">{block.language || "code"}</span>
          <button type="button" onclick={() => copy(block.text)}>Copy code</button></div>
        <pre><code>{block.text}</code></pre>
      </div>
    {:else if block.kind === "heading"}
      <svelte:element this={"h" + block.level}><Inline text={block.text} /></svelte:element>
    {:else if block.kind === "list"}
      <svelte:element this={block.ordered ? "ol" : "ul"}>
        {#each block.items as item}<li><Inline text={item} /></li>{/each}
      </svelte:element>
    {:else if block.kind === "rule"}<hr />
    {:else if block.kind === "quote"}<blockquote><Inline text={block.text} /></blockquote>
    {:else}<p><Inline text={block.text} /></p>{/if}
  {/each}
  {#if copyStatus}<span class="sr-only" role="status">{copyStatus}</span>{/if}
</div>
