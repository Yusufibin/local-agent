<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Chat from "./lib/components/Chat.svelte";
  import Composer from "./lib/components/Composer.svelte";
  import ApprovalModal from "./lib/components/ApprovalModal.svelte";
  import SessionList from "./lib/components/SessionList.svelte";
  import ModelPicker from "./lib/components/ModelPicker.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import Settings from "./lib/components/Settings.svelte";
  import { api } from "./lib/api";
  import { listenHostEvents } from "./lib/events";
  import { initialState, planComposerSubmit, reduce, SILENCE_MS, type SessionState } from "./lib/store/session";
  import type { HostEvent } from "./lib/protocol/events";

  let state: SessionState = $state(initialState());
  let sessions: { path: string; id?: string; name?: string; timestamp?: string }[] = $state([]);
  let models: { id?: string; provider?: string; name?: string }[] = $state([]);
  let chromeOpen = $state(false);
  let starting = $state(true);
  let sending = $state(false);
  let transitioning = $state(false);
  let responding = $state(false);
  let loadingHistory = $state(false);
  let history: SessionState | null = $state(null);
  let historyBefore = $state(0);
  let hasEarlier = $state(false);
  let failure: { message: string; retry?: () => Promise<void> } | null = $state(null);
  let unlisten: (() => void) | undefined;
  let watchdogTimer: ReturnType<typeof setInterval> | undefined;
  let statsTimer: ReturnType<typeof setInterval> | undefined;
  let lastEventAt = Date.now();
  let epoch = 0;
  let destroyed = false;
  let statsPending = false;

  const busy = $derived(starting || transitioning);
  const status = $derived(starting ? "Starting…" : transitioning ? "Loading session…" :
    state.crashBanner ? "Agent unavailable" : state.pendingUi.length ? "Waiting for approval" :
    state.runState === "running" ? "Working…" : "Ready");

  function report(label: string, error: unknown, retry?: () => Promise<void>) {
    failure = { message: label + ": " + String(error), retry };
  }

  function apply(event: HostEvent) {
    if (destroyed || transitioning) return;
    lastEventAt = Date.now();
    state = reduce(state, { type: "host", event });
    if (event.kind === "rpc" && event.event.type === "message_start") hasEarlier = true;
    if (event.kind === "rpc" && event.event.type === "agent_settled") {
      void refreshStats();
      void refreshSessions();
    }
  }

  async function refreshSessions() {
    const requestEpoch = epoch;
    try {
      const result = await api.listSessions();
      if (!destroyed && requestEpoch === epoch) sessions = result as typeof sessions;
    } catch (error) { report("Unable to load sessions", error, refreshSessions); }
  }

  async function refreshModels() {
    const requestEpoch = epoch;
    try {
      const result = await api.getAvailableModels();
      if (!destroyed && requestEpoch === epoch) models = (result.models as typeof models) ?? [];
    } catch (error) { report("Unable to load models", error, refreshModels); }
  }

  async function refreshStats() {
    if (statsPending || busy || destroyed) return;
    statsPending = true;
    const requestEpoch = epoch;
    try {
      const stats = await api.getSessionStats();
      if (!destroyed && requestEpoch === epoch) state = reduce(state, { type: "set_stats", stats });
    } catch { /* Keep the last known counters; connection errors are shown by the host events. */ }
    finally { statsPending = false; }
  }

  async function hydrateFromSession() {
    const requestEpoch = epoch;
    const page = await api.getMessages();
    const current = await api.getState();
    if (destroyed || requestEpoch !== epoch) return;
    const model = current.model as { id?: string } | undefined;
    state = reduce(state, { type: "hydrate", messages: page.messages });
    state = reduce(state, { type: "set_model", modelId: model?.id ?? null,
      thinkingLevel: (current.thinkingLevel as string) ?? null });
    state = { ...state, runState: current.isStreaming ? "running" : "idle" };
    hasEarlier = page.hasMore;
    historyBefore = page.before;
    history = null;
  }

  async function bootstrap() {
    starting = true;
    failure = null;
    try {
      const settings = await api.getSettings();
      state = reduce(state, { type: "set_workspace", activeRoot: (settings.activeRoot as string) ?? null,
        permissionMode: (settings.permissionMode as string) ?? "ask" });
      if (!state.activeRoot) { chromeOpen = true; return; }
      await api.agentStart();
      await hydrateFromSession();
      await refreshSessions();
      await refreshModels();
    } catch (error) { report("Unable to start the agent", error, bootstrap); }
    finally { starting = false; await refreshStats(); }
  }

  onMount(() => {
    void (async () => {
      try {
        const stopListening = await listenHostEvents(apply);
        if (destroyed) { stopListening(); return; }
        unlisten = stopListening;
        await bootstrap();
      } catch (error) { starting = false; report("Unable to connect", error); }
    })();
    statsTimer = setInterval(() => {
      if (state.runState === "running") void refreshStats();
    }, 2000);
    watchdogTimer = setInterval(() => {
      if (state.runState !== "running" || state.silenceBanner || transitioning) return;
      const silenceMs = Date.now() - lastEventAt;
      if (silenceMs >= SILENCE_MS) state = reduce(state, { type: "silence", silenceMs });
    }, 5000);
  });

  onDestroy(() => {
    destroyed = true;
    epoch++;
    unlisten?.();
    if (statsTimer) clearInterval(statsTimer);
    if (watchdogTimer) clearInterval(watchdogTimer);
  });

  async function transition(label: string, action: () => Promise<unknown>, allowRunning = false) {
    if (busy || sending || responding) return;
    if (state.runState === "running" && !allowRunning) {
      report(label, "Stop the current task before changing sessions or settings.");
      return;
    }
    transitioning = true;
    epoch++;
    failure = null;
    try {
      await action();
      const settings = await api.getSettings();
      state = reduce(state, { type: "set_workspace", activeRoot: (settings.activeRoot as string) ?? null,
        permissionMode: (settings.permissionMode as string) ?? "ask" });
      if (state.activeRoot) {
        await hydrateFromSession();
        await refreshSessions();
        await refreshModels();
      }
    } catch (error) { report(label, error, async () => { await transition(label, action, allowRunning); }); }
    finally { transitioning = false; lastEventAt = Date.now(); await refreshStats(); }
  }

  async function restartSidecar() {
    await transition("Unable to restart", () => api.agentRestart(), true);
  }

  async function submit(kind: "enter" | "alt-enter") {
    if (sending || busy || !state.activeRoot || state.crashBanner) return;
    const intent = planComposerSubmit(state, state.composer, { kind });
    if (intent.op === "none" || intent.op === "abort") return;
    const draft = state.composer;
    const requestEpoch = epoch;
    sending = true;
    failure = null;
    history = null;
    try {
      if (intent.op === "prompt") await api.prompt(intent.text);
      if (intent.op === "steer") await api.steer(intent.text);
      if (intent.op === "follow_up") await api.followUp(intent.text);
      if (requestEpoch === epoch && state.composer === draft) {
        state = reduce(state, { type: "set_composer", text: "" });
      }
    } catch (error) { report("Message was not confirmed. Your draft is preserved", error); }
    finally { sending = false; }
  }

  async function abort() {
    if (busy) return;
    try {
      const data = await api.abort();
      state = reduce(state, { type: "abort_result", steering: data.steering ?? [], followUp: data.followUp ?? [] });
    } catch (error) { report("Unable to stop the task", error, abort); }
  }

  async function respond(id: string, payload: Record<string, unknown>) {
    if (responding) return;
    responding = true;
    try {
      const result = await api.uiRespond(id, payload);
      state = reduce(state, { type: "dismiss_ui", id });
      if (!result.accepted) report("Approval expired", "The agent is no longer waiting for this response.");
    } catch (error) { report("Unable to send approval", error); }
    finally { responding = false; }
  }

  async function loadEarlier() {
    if (loadingHistory || busy) return;
    const requestEpoch = epoch;
    loadingHistory = true;
    try {
      const before = history ? historyBefore : (await api.getMessages()).before;
      const page = await api.getMessages(before);
      if (requestEpoch !== epoch || destroyed) return;
      history = reduce(initialState(), { type: "hydrate", messages: page.messages });
      historyBefore = page.before;
      hasEarlier = page.hasMore;
    } catch (error) { report("Unable to load earlier messages", error, loadEarlier); }
    finally { loadingHistory = false; }
  }

  async function latest() {
    if (loadingHistory || busy) return;
    loadingHistory = true;
    try { await hydrateFromSession(); }
    catch (error) { report("Unable to load latest messages", error, latest); }
    finally { loadingHistory = false; }
  }

  async function changeModel(provider: string, id: string) {
    try {
      await api.setModel(provider, id);
      state = reduce(state, { type: "set_model", modelId: id });
    } catch (error) { report("Unable to change model", error); }
  }

  async function changeThinking(level: string) {
    try {
      await api.setThinkingLevel(level);
      state = reduce(state, { type: "set_model", modelId: state.modelId, thinkingLevel: level });
    } catch (error) { report("Unable to change thinking level", error); }
  }
</script>

<div class="shell" class:chrome-open={chromeOpen} data-testid="shell">
  <button type="button" class="chrome-toggle" data-testid="chrome-toggle"
    aria-label={chromeOpen ? "Hide sidebar" : "Show sidebar"} aria-expanded={chromeOpen}
    aria-controls="app-sidebar" onclick={() => (chromeOpen = !chromeOpen)}>☰</button>
  <aside class="sidebar" id="app-sidebar" data-testid="sidebar" aria-hidden={!chromeOpen}
    inert={chromeOpen ? undefined : true}>
    <div class="sidebar-inner">
      <SessionList {sessions} disabled={busy || state.runState === "running"}
        onnew={() => transition("Unable to create session", () => api.newSession())}
        onopen={(path) => transition("Unable to open session", () => api.switchSession(path))} />
      <ModelPicker {models} modelId={state.modelId} thinking={state.thinkingLevel}
        disabled={busy} onmodel={changeModel} onthinking={changeThinking} />
      <Settings permissionMode={state.permissionMode} disabled={busy || state.runState === "running"}
        onmode={(mode) => transition("Unable to change permissions", () => api.setPermissionMode(mode))}
        onsecret={async (provider, key) => { await transition("Unable to save key", () => api.saveSecret(provider, key)); }}
        onworkspace={() => transition("Unable to change workspace", () => api.pickWorkspace())}
        onroot={() => transition("Unable to add workspace root", () => api.addRoot())} />
    </div>
  </aside>
  <section class="main" class:empty={state.messages.length === 0}>
    <div class="main-top" role="status" aria-live="polite">{status}</div>
    {#if failure}
      <div class="banner error-banner" role="alert" data-testid="error-banner">
        <span class="grow">{failure.message}</span>
        {#if failure.retry}<button type="button" disabled={busy} onclick={() => failure?.retry?.()}>Retry</button>{/if}
        <button type="button" aria-label="Dismiss error" onclick={() => (failure = null)}>Dismiss</button>
      </div>
    {/if}
    {#if state.crashBanner || state.silenceBanner}
      <div class="banner health" data-testid="health-banner">
        <span class="grow">{state.crashBanner ?? state.silenceBanner}</span>
        <button type="button" class="primary" disabled={busy} data-testid="restart-sidecar" onclick={restartSidecar}>Restart agent</button>
      </div>
    {/if}
    <Chat session={history ?? state} historical={history !== null} {hasEarlier}
      loading={loadingHistory} onearlier={loadEarlier} onlatest={latest} />
    {#if state.queue.steering.length || state.queue.followUp.length}
      <div class="chips">
        {#each state.queue.steering as text}<span class="chip">steer: {text}</span>{/each}
        {#each state.queue.followUp as text}<span class="chip">follow: {text}</span>{/each}
      </div>
    {/if}
    <Composer bind:value={state.composer} running={state.runState === "running"} {sending}
      disabled={busy || !state.activeRoot || !!state.crashBanner} onsubmit={submit} onabort={abort} />
  </section>
  <StatusBar expanded={chromeOpen} modelId={state.modelId} tokensPercent={state.tokensPercent}
    sessionCost={state.sessionCost} permissionMode={state.permissionMode} activeRoot={state.activeRoot} extras={state.statusEntries} />
</div>

{#if state.pendingUi[0]}
  {#key state.pendingUi[0].id}
    <ApprovalModal request={state.pendingUi[0]} busy={responding}
      onrespond={(payload) => respond(state.pendingUi[0].id, payload)} />
  {/key}
{/if}

{#each state.toasts as toast}
  <div class="toast" role="status">
    {toast.message}
    <button type="button" aria-label="Dismiss notification"
      onclick={() => (state = { ...state, toasts: state.toasts.filter((item) => item.id !== toast.id) })}>×</button>
  </div>
{/each}
