import type { ExtensionUiRequest, HostEvent, PiEvent } from "../protocol/events";

export type RunState = "idle" | "running";

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "toolCall"; id: string; name: string; arguments: unknown; argumentsText: string };

export type TranscriptMessage = {
  id: string;
  role: string;
  content: ContentBlock[];
};

export type ToolCard = {
  messageId?: string;
  toolCallId: string;
  toolName: string;
  args: unknown;
  body: string;
  diff?: string;
  patch?: string;
  isError: boolean;
  status: "running" | "done";
};

export type QueueState = { steering: string[]; followUp: string[] };

export type SessionState = {
  runState: RunState;
  messages: TranscriptMessage[];
  partialByIndex: Record<number, ContentBlock>;
  currentMessageId: string | null;
  toolCards: Record<string, ToolCard>;
  queue: QueueState;
  composer: string;
  composerRestore: string | null;
  banners: string[];
  toasts: { id: string; message: string }[];
  widgets: Record<string, string[]>;
  statusEntries: Record<string, string>;
  windowTitle: string | null;
  crashBanner: string | null;
  silenceBanner: string | null;
  missingPi: string | null;
  modelId: string | null;
  thinkingLevel: string | null;
  permissionMode: string;
  activeRoot: string | null;
  tokensPercent: number | null;
  sessionCost: number | null;
  pendingUi: ExtensionUiRequest[];
  generation: number;
};

export const PI_INSTALL_HINT = "npm i -g --ignore-scripts @earendil-works/pi-coding-agent";
export const TRANSCRIPT_WINDOW = 200;
export const SILENCE_MS = 10 * 60 * 1000;
export const MAX_TOOL_BODY = 32_000;

export function visibleMessages(
  messages: TranscriptMessage[],
  extra = 0,
  windowSize = TRANSCRIPT_WINDOW,
): { hidden: number; slice: TranscriptMessage[] } {
  const keep = windowSize + extra;
  if (messages.length <= keep) return { hidden: 0, slice: messages };
  const hidden = messages.length - keep;
  return { hidden, slice: messages.slice(hidden) };
}

export function silenceBannerText(silenceMs: number): string {
  const mins = Math.max(1, Math.round(silenceMs / 60000));
  return `Agent silent for ${mins} min. Restart sidecar. Long bash is not auto-killed; the last prompt is not replayed.`;
}

export function initialState(): SessionState {
  return {
    runState: "idle",
    messages: [],
    partialByIndex: {},
    currentMessageId: null,
    toolCards: {},
    queue: { steering: [], followUp: [] },
    composer: "",
    composerRestore: null,
    banners: [],
    toasts: [],
    widgets: {},
    statusEntries: {},
    windowTitle: null,
    crashBanner: null,
    silenceBanner: null,
    missingPi: null,
    modelId: null,
    thinkingLevel: null,
    permissionMode: "ask",
    activeRoot: null,
    tokensPercent: null,
    sessionCost: null,
    pendingUi: [],
    generation: 0,
  };
}

export type Action =
  | { type: "host"; event: HostEvent }
  | { type: "hydrate"; messages: unknown[] }
  | { type: "abort_result"; steering: string[]; followUp: string[] }
  | { type: "set_composer"; text: string }
  | { type: "clear_restore" }
  | { type: "dismiss_ui"; id: string }
  | { type: "set_stats"; stats: Record<string, unknown> }
  | { type: "set_model"; modelId: string | null; thinkingLevel?: string | null }
  | { type: "set_workspace"; activeRoot: string | null; permissionMode?: string }
  | { type: "silence"; silenceMs: number }
  | { type: "restarting" }
  | { type: "restarted" };

function textOf(block: unknown): string {
  if (!block || typeof block !== "object") return "";
  const b = block as Record<string, unknown>;
  if (typeof b.text === "string") return b.text;
  if (typeof b.thinking === "string") return b.thinking;
  return "";
}

function blocksFromMessage(message: Record<string, unknown>): ContentBlock[] {
  if (message.role === "toolResult") return [];
  const content = message.content;
  if (typeof content === "string") return [{ type: "text", text: content }];
  if (!Array.isArray(content)) return [];
  return content.map((c) => {
    const x = c as Record<string, unknown>;
    if (x.type === "thinking") return { type: "thinking", thinking: String(x.thinking ?? "") };
    if (x.type === "toolCall" || x.type === "tool_call") {
      return {
        type: "toolCall",
        id: String(x.id ?? ""),
        name: String(x.name ?? x.toolName ?? ""),
        arguments: x.arguments ?? x.args,
        argumentsText: JSON.stringify(x.arguments ?? x.args ?? {}),
      };
    }
    return { type: "text", text: String(x.text ?? "") };
  });
}

function upsertPartial(state: SessionState, index: number, block: ContentBlock): SessionState {
  return { ...state, partialByIndex: { ...state.partialByIndex, [index]: block } };
}

function applyDelta(state: SessionState, ev: Record<string, unknown>): SessionState {
  const index = Number(ev.contentIndex ?? 0);
  const t = String(ev.type ?? "");
  const current = state.partialByIndex[index];
  if (t === "text_start") return upsertPartial(state, index, { type: "text", text: "" });
  if (t === "text_delta") {
    const prev = current && current.type === "text" ? current.text : "";
    return upsertPartial(state, index, { type: "text", text: prev + String(ev.delta ?? "") });
  }
  if (t === "thinking_start") return upsertPartial(state, index, { type: "thinking", thinking: "" });
  if (t === "thinking_delta") {
    const prev = current && current.type === "thinking" ? current.thinking : "";
    return upsertPartial(state, index, {
      type: "thinking",
      thinking: prev + String(ev.delta ?? ""),
    });
  }
  if (t === "toolcall_start") {
    return upsertPartial(state, index, {
      type: "toolCall",
      id: String(ev.id ?? ""),
      name: String(ev.toolName ?? ""),
      arguments: {},
      argumentsText: "",
    });
  }
  if (t === "toolcall_delta") {
    const prev = current && current.type === "toolCall" ? current : {
      type: "toolCall" as const,
      id: "",
      name: "",
      arguments: {},
      argumentsText: "",
    };
    const argumentsText = prev.argumentsText + String(ev.delta ?? "");
    return upsertPartial(state, index, { ...prev, argumentsText });
  }
  if (t === "toolcall_end") {
    const toolCall = (ev.toolCall as Record<string, unknown> | undefined) ?? {};
    return upsertPartial(state, index, {
      type: "toolCall",
      id: String(toolCall.id ?? ev.id ?? ""),
      name: String(toolCall.name ?? ev.toolName ?? ""),
      arguments: toolCall.arguments,
      argumentsText: JSON.stringify(toolCall.arguments ?? {}),
    });
  }
  return state;
}

function toolBody(result: unknown): { body: string; diff?: string; patch?: string } {
  if (!result || typeof result !== "object") return { body: "" };
  const r = result as Record<string, unknown>;
  const details = (r.details as Record<string, unknown> | undefined) ?? {};
  const content = r.content;
  let body = "";
  if (Array.isArray(content)) body = content.map(textOf).join("");
  else if (typeof content === "string") body = content;
  const diff = typeof details.diff === "string" ? details.diff : undefined;
  const patch = typeof details.patch === "string" ? details.patch : undefined;
  return { body, diff, patch };
}

function applyRpc(state: SessionState, event: PiEvent): SessionState {
  switch (event.type) {
    case "agent_start":
      return { ...state, runState: "running", crashBanner: null };
    case "agent_end":
      return state; // stay running; idle only on agent_settled
    case "agent_settled":
      return { ...state, runState: "idle", currentMessageId: null, partialByIndex: {} };
    case "message_start": {
      const message = (event.message as Record<string, unknown>) ?? {};
      const id = String(message.id ?? `m-${state.messages.length}`);
      const msg: TranscriptMessage = {
        id,
        role: String(message.role ?? "assistant"),
        content: blocksFromMessage(message),
      };
      return {
        ...state,
        currentMessageId: id,
        partialByIndex: {},
        messages: [...state.messages, msg],
      };
    }
    case "message_update": {
      const delta = event.assistantMessageEvent as Record<string, unknown> | undefined;
      if (!delta) return state;
      return applyDelta(state, delta);
    }
    case "message_end": {
      const message = (event.message as Record<string, unknown>) ?? {};
      const id = state.currentMessageId;
      const next = blocksFromMessage(message);
      const messages = state.messages.map((m) =>
        m.id === id || (id === null && m === state.messages[state.messages.length - 1])
          ? { ...m, content: next, }
          : m,
      );
      return { ...state, messages, partialByIndex: {}, currentMessageId: null };
    }
    case "tool_execution_start": {
      const id = String(event.toolCallId ?? "");
      const card: ToolCard = {
        toolCallId: id,
        messageId: ownerOfTool(state.messages, id) ?? state.currentMessageId ?? undefined,
        toolName: String(event.toolName ?? ""),
        args: event.args,
        body: "",
        isError: false,
        status: "running",
      };
      return { ...state, toolCards: { ...state.toolCards, [id]: card } };
    }
    case "tool_execution_update": {
      const id = String(event.toolCallId ?? "");
      const prev = state.toolCards[id];
      const parsed = toolBody(event.partialResult);
      const card: ToolCard = {
        toolCallId: id,
        messageId: prev?.messageId ?? ownerOfTool(state.messages, id),
        toolName: String(event.toolName ?? prev?.toolName ?? ""),
        args: event.args ?? prev?.args,
        body: parsed.body,
        diff: parsed.diff,
        patch: parsed.patch,
        isError: prev?.isError ?? false,
        status: "running",
      };
      return { ...state, toolCards: { ...state.toolCards, [id]: card } };
    }
    case "tool_execution_end": {
      const id = String(event.toolCallId ?? "");
      const prev = state.toolCards[id];
      const parsed = toolBody(event.result);
      const card: ToolCard = {
        toolCallId: id,
        toolName: String(event.toolName ?? prev?.toolName ?? ""),
        args: prev?.args,
        body: parsed.body,
        diff: parsed.diff,
        patch: parsed.patch,
        isError: Boolean(event.isError),
        status: "done",
      };
      return { ...state, toolCards: { ...state.toolCards, [id]: card } };
    }
    case "queue_update":
      return {
        ...state,
        queue: {
          steering: Array.isArray(event.steering) ? (event.steering as string[]) : [],
          followUp: Array.isArray(event.followUp) ? (event.followUp as string[]) : [],
        },
      };
    case "compaction_start":
      return { ...state, banners: withBanner(state.banners, "compaction") };
    case "compaction_end":
      return { ...state, banners: state.banners.filter((b) => b !== "compaction") };
    case "auto_retry_start":
      return { ...state, banners: withBanner(state.banners, "retry") };
    case "auto_retry_end":
      return { ...state, banners: state.banners.filter((b) => b !== "retry") };
    case "extension_error":
      return {
        ...state,
        toasts: [
          ...state.toasts,
          { id: `t-${state.toasts.length}`, message: String(event.error ?? "extension error") },
        ],
      };
    default:
      return state;
  }
}

function withBanner(banners: string[], name: string): string[] {
  return banners.includes(name) ? banners : [...banners, name];
}

function applyUiRequest(state: SessionState, request: ExtensionUiRequest): SessionState {
  switch (request.method) {
    case "notify":
      return {
        ...state,
        toasts: [...state.toasts, { id: request.id, message: request.message ?? "" }],
      };
    case "setStatus": {
      const next = { ...state.statusEntries };
      if (!request.statusText) delete next[request.statusKey ?? "default"];
      else next[request.statusKey ?? "default"] = request.statusText;
      return { ...state, statusEntries: next };
    }
    case "setWidget": {
      const next = { ...state.widgets };
      if (!request.widgetLines) delete next[request.widgetKey ?? "default"];
      else next[request.widgetKey ?? "default"] = request.widgetLines;
      return { ...state, widgets: next };
    }
    case "setTitle":
      return { ...state, windowTitle: request.title ?? request.text ?? null };
    case "set_editor_text":
      return { ...state, composer: request.text ?? "", composerRestore: request.text ?? null };
    case "select":
    case "confirm":
    case "input":
    case "editor":
      return { ...state, pendingUi: [...state.pendingUi, request] };
    default:
      return state;
  }
}

function reduceAction(state: SessionState, action: Action): SessionState {
  switch (action.type) {
    case "host": {
      const ev = action.event;
      const tagged = ev.kind === "rpc" ? ev.event : ev.kind === "ui_request" ? ev.request : null;
      const generation = tagged && typeof tagged._generation === "number" ? tagged._generation : state.generation;
      if (generation < state.generation) return state;
      if (generation > state.generation) state = { ...state, generation, pendingUi: [] };
      if (ev.kind === "rpc") return applyRpc(state, ev.event);
      if (ev.kind === "ui_request") return applyUiRequest(state, ev.request);
      if (ev.kind === "process") {
        if (ev.status === "crashed") {
          const extra = ev.message ? `: ${ev.message.slice(0, 400)}` : "";
          return {
            ...state,
            runState: "idle",
            pendingUi: [],
            silenceBanner: null,
            crashBanner: `sidecar crashed${ev.code != null ? ` (code ${ev.code})` : ""}${extra}`,
          };
        }
        if (ev.status === "spawned") {
          return {
            ...state,
            crashBanner: null,
            silenceBanner: null,
            runState: "idle",
          };
        }
        if (ev.status === "exited") return { ...state, runState: "idle", pendingUi: [], crashBanner: "Agent stopped. Restart to continue." };
        return state;
      }
      if (ev.kind === "watchdog") {
        return { ...state, silenceBanner: silenceBannerText(ev.silenceMs) };
      }
      if (ev.kind === "log") {
        const msg = ev.message;
        if (msg.includes("npm i -g --ignore-scripts @earendil-works/pi-coding-agent")) {
          return { ...state, missingPi: PI_INSTALL_HINT };
        }
        if (ev.level === "error" || ev.level === "warn") {
          return { ...state, toasts: [...state.toasts, { id: `log-${Date.now()}`, message: msg }] };
        }
        return state;
      }
      return state;
    }
    case "hydrate": {
      const messages: TranscriptMessage[] = action.messages.map((m, i) => {
        const rec = (m ?? {}) as Record<string, unknown>;
        return {
          id: String(rec.id ?? `h-${i}`),
          role: String(rec.role ?? "assistant"),
          content: blocksFromMessage(rec),
        };
      });
      const toolCards: Record<string, ToolCard> = {};
      for (const [i, raw] of action.messages.entries()) {
        const rec = (raw ?? {}) as Record<string, unknown>;
        if (rec.role !== "toolResult") continue;
        const id = String(rec.toolCallId ?? "");
        const result = toolBody(rec);
        toolCards[id] = { toolCallId: id, messageId: ownerOfTool(messages, id) ?? messages[i]?.id,
          toolName: String(rec.toolName ?? "tool"), args: undefined, ...result,
          isError: Boolean(rec.isError), status: "done" };
      }
      return { ...state, messages, toolCards, queue: { steering: [], followUp: [] }, pendingUi: [], partialByIndex: {}, currentMessageId: null };
    }
    case "abort_result": {
      const bits = [...action.steering, ...action.followUp].filter(Boolean);
      const text = bits.join("\n");
      return { ...state, composerRestore: text, composer: text || state.composer };
    }
    case "set_composer":
      return { ...state, composer: action.text };
    case "clear_restore":
      return { ...state, composerRestore: null };
    case "dismiss_ui":
      return { ...state, pendingUi: state.pendingUi.filter((p) => p.id !== action.id) };
    case "set_stats": {
      const ctx = action.stats.contextUsage as Record<string, unknown> | undefined;
      const percent = ctx && typeof ctx.percent === "number" ? ctx.percent : null;
      const cost = typeof action.stats.cost === "number" ? action.stats.cost : null;
      return { ...state, tokensPercent: percent, sessionCost: cost };
    }
    case "set_model":
      return {
        ...state,
        modelId: action.modelId,
        thinkingLevel: action.thinkingLevel ?? state.thinkingLevel,
      };
    case "set_workspace":
      return {
        ...state,
        activeRoot: action.activeRoot,
        permissionMode: action.permissionMode ?? state.permissionMode,
      };
    case "silence":
      return { ...state, silenceBanner: silenceBannerText(action.silenceMs) };
    case "restarting":
      return { ...state, banners: withBanner(state.banners, "restarting") };
    case "restarted":
      return {
        ...state,
        runState: "idle",
        crashBanner: null,
        silenceBanner: null,
        pendingUi: [],
        banners: state.banners.filter((b) => b !== "restarting"),
      };
    default:
      return state;
  }
}

export type SendOp =
  | { op: "prompt"; text: string }
  | { op: "steer"; text: string }
  | { op: "follow_up"; text: string }
  | { op: "abort" }
  | { op: "none"; reason: string };

export function canSendBarePrompt(state: SessionState): boolean {
  return state.runState !== "running";
}

/** Keyboard mapping: Enter idle→prompt, running→steer; Alt+Enter running→follow_up; Esc→abort. */
export function planComposerSubmit(
  state: SessionState,
  text: string,
  input: { kind: "enter" | "alt-enter" | "esc" },
): SendOp {
  if (input.kind === "esc") return { op: "abort" };
  const trimmed = text.trim();
  if (!trimmed) return { op: "none", reason: "empty" };
  if (state.runState === "idle") {
    if (input.kind === "enter") return { op: "prompt", text: trimmed };
    return { op: "none", reason: "idle-alt" };
  }
  if (input.kind === "alt-enter") return { op: "follow_up", text: trimmed };
  if (input.kind === "enter") return { op: "steer", text: trimmed };
  return { op: "none", reason: "streaming" };
}

export function liveAssistantText(state: SessionState): string {
  return Object.values(state.partialByIndex)
    .filter((b): b is Extract<ContentBlock, { type: "text" }> => b.type === "text")
    .map((b) => b.text)
    .join("");
}
function ownerOfTool(messages: TranscriptMessage[], id: string): string | undefined {
  return [...messages].reverse().find((message) =>
    message.content.some((block) => block.type === "toolCall" && block.id === id),
  )?.id;
}

function bounded(text: string, limit: number): string {
  return text.length > limit ? text.slice(0, limit) + "\n… (truncated)" : text;
}

/** Bound retained state as well as rendered output, including restored sessions. */
export function reduce(state: SessionState, action: Action): SessionState {
  const next = reduceAction(state, action);
  if (next === state) return state;
  const messages = next.messages === state.messages ? state.messages : next.messages.slice(-TRANSCRIPT_WINDOW).map((message) => ({
    ...message,
    content: message.content.map((block) => {
      if (block.type === "text") return { ...block, text: bounded(block.text, 128_000) };
      if (block.type === "thinking") return { ...block, thinking: bounded(block.thinking, MAX_TOOL_BODY) };
      return { ...block, argumentsText: bounded(block.argumentsText, MAX_TOOL_BODY) };
    }),
  }));
  const owners = new Set(messages.map((message) => message.id));
  const toolCards = next.toolCards === state.toolCards && messages === state.messages ? state.toolCards : Object.fromEntries(Object.entries(next.toolCards)
    .filter(([, card]) => !card.messageId || owners.has(card.messageId))
    .slice(-TRANSCRIPT_WINDOW)
    .map(([id, card]) => [id, { ...card, body: bounded(card.body, MAX_TOOL_BODY),
      diff: card.diff ? bounded(card.diff, MAX_TOOL_BODY) : undefined,
      patch: card.patch ? bounded(card.patch, MAX_TOOL_BODY) : undefined,
    }]));
  const partialByIndex = Object.fromEntries(Object.entries(next.partialByIndex).map(([index, block]) => {
    if (block.type === "text") return [index, { ...block, text: bounded(block.text, 128_000) }];
    if (block.type === "thinking") return [index, { ...block, thinking: bounded(block.thinking, MAX_TOOL_BODY) }];
    return [index, { ...block, argumentsText: bounded(block.argumentsText, MAX_TOOL_BODY) }];
  }));
  return { ...next, messages, toolCards, partialByIndex, toasts: next.toasts.slice(-5) };
}
