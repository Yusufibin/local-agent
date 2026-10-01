import { describe, expect, it } from "vitest";
import { assertRpcSuccess } from "../src/lib/api";
describe("RPC result validation", () => {
  it("rejects application failures even when IPC succeeded", () => {
    expect(() => assertRpcSuccess({ success: false, error: "Model unavailable" })).toThrow("Model unavailable");
    expect(() => assertRpcSuccess({ success: true })).not.toThrow();
    expect(() => assertRpcSuccess({ messages: [] })).not.toThrow();
  });
});
