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
  import {
    initialState,
    planComposerSubmit,
    reduce,
    SILENCE_MS,
    type SessionState,
  } from "./lib/store/session";
  import type { HostEvent } from "./lib/protocol/events";

  let state: SessionState = $state(initialState());
  let sessions: { path: string; id?: string; name?: string; timestamp?: string }[] = $state([]);
  let models: { id?: string; provider?: string; name?: string }[] = $state([]);
  let chromeOpen = $state(false);
  let unlisten: (() => void) | undefined;
  let statsTimer: ReturnType<typeof setInterval> | undefined;
  let watchdogTimer: ReturnType<typeof setInterval> | undefined;
  let lastEventAt = Date.now();

  function toggleChrome() {
    chromeOpen = !chromeOpen;
  }

  function apply(event: HostEvent) {
    lastEventAt = Date.now();
    state = reduce(state, { type: "host", event });
  }

  async function refreshSessions() {
    try {
      sessions = (await api.listSessions()) as typeof sessions;
    } catch {
      sessions = [];
    }
  }

  async function refreshModels() {
    try {
      const r = await api.getAvailableModels();
      models = (r.models as typeof models) ?? [];
    } catch {
      models = [];
    }
  }

  async function refreshStats() {
    try {
      const stats = await api.getSessionStats();
      state = reduce(state, { type: "set_stats", stats });
    } catch {
      /* sidecar idle */
    }
  }

  async function bootstrap() {
    try {
      const settings = await api.getSettings();
      state = reduce(state, {
        type: "set_workspace",
        activeRoot: (settings.activeRoot as string) ?? null,
        permissionMode: (settings.permissionMode as string) ?? "ask",
      });
    } catch {
      /* browser / no tauri */
    }
    try {
      const st = await api.getState();
      if (st?.missingPi) {
        apply({ kind: "log", level: "error", message: String(st.install ?? st.missingPi) });
      } else if (st?.crashed) {
        apply({
          kind: "process",
          status: "crashed",
          message: String(st.stderr ?? ""),
        });
      } else {
        const model = st?.model as { id?: string } | undefined;
        state = reduce(state, {
          type: "set_model",
          modelId: model?.id ?? null,
          thinkingLevel: (st?.thinkingLevel as string) ?? null,
        });
      }
    } catch {
      /* ignore */
    }
    try {
      await api.agentStart();
      const st = await api.getState();
      const model = st?.model as { id?: string } | undefined;
      if (model?.id) {
        state = reduce(state, { type: "set_model", modelId: model.id });
      }
      const msgs = await api.getMessages();
      if (msgs?.messages) state = reduce(state, { type: "hydrate", messages: msgs.messages });
    } catch (e) {
      const msg = String(e);
      if (msg.includes("npm i -g")) {
        apply({ kind: "log", level: "error", message: msg });
      }
    }
    await refreshSessions();
    await refreshModels();
    await refreshStats();
  }

  onMount(() => {
    listenHostEvents((ev) => {
      apply(ev);
      if (ev.kind === "rpc" && ev.event.type === "agent_settled") {
        refreshStats();
        if (statsTimer) {
          clearInterval(statsTimer);
          statsTimer = undefined;
        }
      }
      if (ev.kind === "rpc" && ev.event.type === "agent_start") {
        if (!statsTimer) statsTimer = setInterval(refreshStats, 2000);
      }
    }).then((u) => {
      unlisten = u;
    });
    watchdogTimer = setInterval(() => {
      if (state.runState !== "running" || state.silenceBanner) return;
      const dt = Date.now() - lastEventAt;
      if (dt >= SILENCE_MS) {
        state = reduce(state, { type: "silence", silenceMs: dt });
      }
    }, 5000);
    bootstrap();
  });

  onDestroy(() => {
    unlisten?.();
    if (statsTimer) clearInterval(statsTimer);
    if (watchdogTimer) clearInterval(watchdogTimer);
  });

  async function restartSidecar() {
    state = reduce(state, { type: "restarting" });
    try {
      await api.agentRestart();
      state = reduce(state, { type: "restarted" });
      const st = await api.getState();
      const model = st?.model as { id?: string } | undefined;
      if (model?.id) {
        state = reduce(state, { type: "set_model", modelId: model.id });
      }
      await hydrateFromSession();
      await refreshModels();
      await refreshStats();
    } catch (e) {
      apply({ kind: "process", status: "crashed", message: String(e) });
    }
  }

  async function submit(kind: "enter" | "alt-enter") {
    const intent = planComposerSubmit(state, state.composer, { kind });
    if (intent.op === "none") return;
    const text = "text" in intent ? intent.text : "";
    state = reduce(state, { type: "set_composer", text: "" });
    try {
      if (intent.op === "prompt") await api.prompt(text);
      if (intent.op === "steer") await api.steer(text);
      if (intent.op === "follow_up") await api.followUp(text);
    } catch (e) {
      state = reduce(state, {
        type: "host",
        event: { kind: "log", level: "error", message: String(e) },
      });
    }
  }

  async function abort() {
    try {
      const data = await api.abort();
      state = reduce(state, {
        type: "abort_result",
        steering: data.steering ?? [],
        followUp: data.followUp ?? [],
      });
    } catch {
      /* ignore */
    }
  }

  async function hydrateFromSession() {
    try {
      const msgs = await api.getMessages();
      if (msgs?.messages) state = reduce(state, { type: "hydrate", messages: msgs.messages });
    } catch {
      /* ignore */
    }
  }
</script>

<div class="shell" class:chrome-open={chromeOpen} data-testid="shell">
  <button
    type="button"
    class="chrome-toggle"
    data-testid="chrome-toggle"
    aria-label={chromeOpen ? "Hide sidebar" : "Show sidebar"}
    aria-expanded={chromeOpen}
    aria-controls="app-sidebar"
    onclick={toggleChrome}
  >
    ☰
  </button>
  <aside
    class="sidebar"
    id="app-sidebar"
    data-testid="sidebar"
    aria-hidden={!chromeOpen}
    inert={chromeOpen ? undefined : true}
  >
    <div class="sidebar-inner">
      <SessionList
        {sessions}
        onnew={async () => {
          await api.newSession();
          await hydrateFromSession();
          await refreshSessions();
        }}
        onopen={async (path) => {
          await api.switchSession(path);
          await hydrateFromSession();
        }}
      />
      <ModelPicker
        {models}
        modelId={state.modelId}
        thinking={state.thinkingLevel}
        onmodel={(provider, id) => {
          api.setModel(provider, id).then(() => {
            state = reduce(state, { type: "set_model", modelId: id });
          });
        }}
        onthinking={(level) => {
          api.setThinkingLevel(level).then(() => {
            state = reduce(state, { type: "set_model", modelId: state.modelId, thinkingLevel: level });
          });
        }}
      />
      <Settings
        permissionMode={state.permissionMode}
        onmode={async (mode) => {
          await api.setPermissionMode(mode);
          state = reduce(state, { type: "set_workspace", activeRoot: state.activeRoot, permissionMode: mode });
        }}
        onsecret={(provider, key) => api.saveSecret(provider, key)}
        onworkspace={async () => {
          const r = (await api.pickWorkspace()) as { activeRoot?: string };
          if (r?.activeRoot) {
            state = reduce(state, { type: "set_workspace", activeRoot: r.activeRoot });
          }
        }}
        onroot={() => api.addRoot()}
      />
    </div>
  </aside>
  <section class="main" class:empty={state.messages.length === 0}>
    <div class="main-top"></div>
    {#if state.crashBanner || state.silenceBanner}
      <div
        class="banner health"
        class:silence={!!state.silenceBanner && !state.crashBanner}
        data-testid="health-banner"
      >
        <span class="grow">{state.crashBanner ?? state.silenceBanner}</span>
        <button type="button" class="primary" data-testid="restart-sidecar" onclick={restartSidecar}>
          Restart sidecar
        </button>
      </div>
    {/if}
    <Chat session={state} />
    {#if state.queue.steering.length || state.queue.followUp.length}
      <div class="chips">
        {#each state.queue.steering as s}<span class="chip">steer: {s}</span>{/each}
        {#each state.queue.followUp as s}<span class="chip">follow: {s}</span>{/each}
      </div>
    {/if}
    <Composer bind:value={state.composer} running={state.runState === "running"} onsubmit={submit} onabort={abort} />
  </section>
  <StatusBar
    expanded={chromeOpen}
    modelId={state.modelId}
    tokensPercent={state.tokensPercent}
    sessionCost={state.sessionCost}
    permissionMode={state.permissionMode}
    activeRoot={state.activeRoot}
    extras={state.statusEntries}
  />
</div>

{#if state.pendingUi[0]}
  <ApprovalModal
    request={state.pendingUi[0]}
    onrespond={(payload) => {
      const id = state.pendingUi[0].id;
      api.uiRespond(id, payload);
      state = reduce(state, { type: "dismiss_ui", id });
    }}
  />
{/if}

{#each state.toasts as t}
  <div class="toast">{t.message}</div>
{/each}