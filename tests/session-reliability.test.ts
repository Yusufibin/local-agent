import { describe, expect, it } from "vitest";
import { initialState, reduce, MAX_TOOL_BODY, TRANSCRIPT_WINDOW } from "../src/lib/store/session";
import type { HostEvent } from "../src/lib/protocol/events";
const rpc = (event: Record<string, unknown>): HostEvent => ({ kind: "rpc", event: event as { type: string } });
describe("session reliability", () => {
  it("ignores messages from an older sidecar generation", () => {
    let state = reduce(initialState(), { type: "host", event: rpc({ type: "agent_start", _generation: 4 }) });
    state = reduce(state, { type: "host", event: rpc({ type: "agent_settled", _generation: 3 }) });
    expect(state.runState).toBe("running");
    expect(state.generation).toBe(4);
  });
  it("keeps tool results attached to their original message", () => {
    let state = reduce(initialState(), { type: "host", event: rpc({ type: "message_start",
      message: { id: "owner", role: "assistant", content: [{ type: "toolCall", id: "call", name: "read", arguments: {} }] } }) });
    state = reduce(state, { type: "host", event: rpc({ type: "message_end",
      message: { role: "assistant", content: [{ type: "toolCall", id: "call", name: "read", arguments: {} }] } }) });
    state = reduce(state, { type: "host", event: rpc({ type: "tool_execution_start", toolCallId: "call", toolName: "read" }) });
    state = reduce(state, { type: "host", event: rpc({ type: "message_start", message: { id: "next", role: "assistant", content: [] } }) });
    expect(state.toolCards.call.messageId).toBe("owner");
  });
  it("clears old tools and rebuilds completed cards when switching sessions", () => {
    let state = reduce(initialState(), { type: "host", event: rpc({ type: "tool_execution_start", toolCallId: "old" }) });
    state = reduce(state, { type: "hydrate", messages: [
      { id: "new", role: "assistant", content: [{ type: "toolCall", id: "new-call", name: "read", arguments: {} }] },
      { role: "toolResult", toolCallId: "new-call", toolName: "read", content: [{ type: "text", text: "result" }] },
    ] });
    expect(state.toolCards.old).toBeUndefined();
    expect(state.toolCards["new-call"].messageId).toBe("new");
    expect(state.toolCards["new-call"].body).toBe("result");
  });
  it("bounds retained messages and tool bodies rather than only DOM output", () => {
    let state = initialState();
    for (let i = 0; i < 500; i++) {
      state = reduce(state, { type: "host", event: rpc({ type: "message_start", message: { id: String(i), role: "user", content: "hello" } }) });
    }
    expect(state.messages).toHaveLength(TRANSCRIPT_WINDOW);
    state = reduce(state, { type: "host", event: rpc({ type: "tool_execution_end", toolCallId: "large", result: { content: "x".repeat(2 * MAX_TOOL_BODY) } }) });
    expect(state.toolCards.large.body.length).toBeLessThan(MAX_TOOL_BODY + 50);
  });
  it("shows generic host errors", () => {
    const state = reduce(initialState(), { type: "host", event: { kind: "log", level: "error", message: "Provider rejected the key" } });
    expect(state.toasts[0].message).toContain("Provider rejected");
  });
});
