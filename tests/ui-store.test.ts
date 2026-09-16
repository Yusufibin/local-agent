import { describe, expect, it } from "vitest";
import {
  canSendBarePrompt,
  initialState,
  liveAssistantText,
  planComposerSubmit,
  reduce,
  visibleMessages,
  type SessionState,
} from "../src/lib/store/session";
import type { HostEvent, PiEvent } from "../src/lib/protocol/events";

function rpc(event: Record<string, unknown>): HostEvent {
  return { kind: "rpc", event: event as PiEvent };
}

describe("session reducer", () => {
  it("assembles two text_deltas by contentIndex", () => {
    let s = initialState();
    s = reduce(s, { type: "host", event: rpc({ type: "agent_start" }) });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_start",
        message: { role: "assistant", id: "m1", content: [] },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_update",
        assistantMessageEvent: { type: "text_delta", contentIndex: 0, delta: "Hello" },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_update",
        assistantMessageEvent: { type: "text_delta", contentIndex: 0, delta: " world" },
      }),
    });
    expect(liveAssistantText(s)).toBe("Hello world");
  });

  it("message_end replaces the partial", () => {
    let s = initialState();
    s = reduce(s, { type: "host", event: rpc({ type: "agent_start" }) });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_start",
        message: { role: "assistant", id: "m1", content: [] },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_update",
        assistantMessageEvent: { type: "text_delta", contentIndex: 0, delta: "partial" },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "message_end",
        message: { role: "assistant", id: "m1", content: [{ type: "text", text: "final authoritative" }] },
      }),
    });
    expect(s.messages[0].content).toEqual([{ type: "text", text: "final authoritative" }]);
    expect(Object.keys(s.partialByIndex)).toHaveLength(0);
  });

  it("agent_end leaves running; agent_settled sets idle", () => {
    let s = initialState();
    s = reduce(s, { type: "host", event: rpc({ type: "agent_start" }) });
    expect(s.runState).toBe("running");
    s = reduce(s, { type: "host", event: rpc({ type: "agent_end", messages: [], willRetry: false }) });
    expect(s.runState).toBe("running");
    s = reduce(s, { type: "host", event: rpc({ type: "agent_settled" }) });
    expect(s.runState).toBe("idle");
  });

  it("tool_execution_update replaces card body (not a delta)", () => {
    let s = initialState();
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "tool_execution_start",
        toolCallId: "c1",
        toolName: "bash",
        args: { command: "ls" },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "tool_execution_update",
        toolCallId: "c1",
        toolName: "bash",
        partialResult: { content: [{ type: "text", text: "a\nb\n" }] },
      }),
    });
    s = reduce(s, {
      type: "host",
      event: rpc({
        type: "tool_execution_update",
        toolCallId: "c1",
        toolName: "bash",
        partialResult: { content: [{ type: "text", text: "REPLACED" }] },
      }),
    });
    expect(s.toolCards.c1.body).toBe("REPLACED");
  });

  it("Esc path restores clear_queue text", () => {
    let s = initialState();
    s = reduce(s, {
      type: "abort_result",
      steering: ["queued steer"],
      followUp: ["queued follow"],
    });
    expect(s.composerRestore).toBe("queued steer\nqueued follow");
    expect(s.composer).toContain("queued steer");
  });

  it("crash banner idles UI and restart clears without replaying", () => {
    let s = initialState();
    s = reduce(s, { type: "host", event: rpc({ type: "agent_start" }) });
    s = reduce(s, {
      type: "host",
      event: { kind: "process", status: "crashed", code: 137, message: "killed" },
    });
    expect(s.runState).toBe("idle");
    expect(s.crashBanner).toContain("137");
    expect(s.crashBanner).toContain("killed");
    s = reduce(s, { type: "restarting" });
    s = reduce(s, { type: "restarted" });
    expect(s.crashBanner).toBeNull();
    expect(s.silenceBanner).toBeNull();
    expect(s.runState).toBe("idle");
    expect(s.composer).toBe("");
  });

  it("watchdog sets silence banner; spawned clears it", () => {
    let s = initialState();
    s = reduce(s, { type: "host", event: rpc({ type: "agent_start" }) });
    s = reduce(s, { type: "host", event: { kind: "watchdog", silenceMs: 10 * 60 * 1000 } });
    expect(s.silenceBanner).toMatch(/silent/i);
    s = reduce(s, { type: "host", event: { kind: "process", status: "spawned" } });
    expect(s.silenceBanner).toBeNull();
    expect(s.crashBanner).toBeNull();
  });

  it("transcript window keeps the tail", () => {
    const messages = Array.from({ length: 250 }, (_, i) => ({
      id: `m${i}`,
      role: "assistant",
      content: [{ type: "text" as const, text: String(i) }],
    }));
    const { hidden, slice } = visibleMessages(messages, 0, 200);
    expect(hidden).toBe(50);
    expect(slice).toHaveLength(200);
    expect(slice[0].id).toBe("m50");
  });

  it("streaming without steer/followUp does not send a bare prompt", () => {
    const running: SessionState = { ...initialState(), runState: "running" };
    expect(canSendBarePrompt(running)).toBe(false);
    const enter = planComposerSubmit(running, "hello", { kind: "enter" });
    expect(enter.op).toBe("steer");
    expect(enter.op).not.toBe("prompt");
    const alt = planComposerSubmit(running, "later", { kind: "alt-enter" });
    expect(alt.op).toBe("follow_up");
    const idle = planComposerSubmit(initialState(), "hello", { kind: "enter" });
    expect(idle.op).toBe("prompt");
  });
});