/** @vitest-environment happy-dom */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, unmount, tick } from "svelte";
import App from "../src/App.svelte";
import type { HostEvent } from "../src/lib/protocol/events";
const mocks = vi.hoisted(() => ({
  api: {
    getSettings: vi.fn(), agentStart: vi.fn(), getMessages: vi.fn(), getState: vi.fn(),
    listSessions: vi.fn(), getAvailableModels: vi.fn(), getSessionStats: vi.fn(),
    prompt: vi.fn(), steer: vi.fn(), followUp: vi.fn(), abort: vi.fn(), uiRespond: vi.fn(),
    newSession: vi.fn(), switchSession: vi.fn(),
  },
  event: undefined as ((event: HostEvent) => void) | undefined,
}));
vi.mock("../src/lib/api", () => ({ api: mocks.api }));
vi.mock("../src/lib/events", () => ({ listenHostEvents: vi.fn(async (handler: (event: HostEvent) => void) => { mocks.event = handler; return () => {}; }) }));
let app: ReturnType<typeof mount> | undefined;
beforeEach(async () => {
  vi.resetAllMocks();
  mocks.api.getSettings.mockResolvedValue({ activeRoot: "/workspace", permissionMode: "ask" });
  mocks.api.agentStart.mockResolvedValue({});
  mocks.api.getMessages.mockResolvedValue({ messages: [], total: 0, before: 0, hasMore: false });
  mocks.api.getState.mockResolvedValue({ model: { id: "test-model" }, isStreaming: false });
  mocks.api.listSessions.mockResolvedValue([]);
  mocks.api.getAvailableModels.mockResolvedValue({ models: [] });
  mocks.api.getSessionStats.mockResolvedValue({});
  document.body.innerHTML = '<div id="app"></div>';
  app = mount(App, { target: document.getElementById("app")! });
  await vi.waitFor(() => expect((document.querySelector('[data-testid="composer-input"]') as HTMLTextAreaElement).disabled).toBe(false));
});
afterEach(async () => { if (app) await unmount(app); document.body.innerHTML = ""; });
function draft(text: string) {
  const textarea = document.querySelector('[data-testid="composer-input"]') as HTMLTextAreaElement;
  textarea.value = text;
  textarea.dispatchEvent(new Event("input", { bubbles: true }));
  return textarea;
}
function send() {
  document.querySelector<HTMLButtonElement>(".composer-actions .primary")!.click();
}
describe("chat interactions", () => {
  it("preserves the draft and displays an IPC failure", async () => {
    mocks.api.prompt.mockRejectedValue(new Error("offline"));
    const textarea = draft("fix the bug");
    await tick(); send();
    await vi.waitFor(() => expect(document.querySelector('[data-testid="error-banner"]')?.textContent).toContain("offline"));
    expect(textarea.value).toBe("fix the bug");
  });
  it("prevents duplicate submissions and preserves edits made while awaiting acknowledgement", async () => {
    let resolve!: () => void;
    mocks.api.prompt.mockReturnValue(new Promise<void>((done) => { resolve = done; }));
    draft("first"); await tick(); send(); send();
    expect(mocks.api.prompt).toHaveBeenCalledTimes(1);
    draft("new draft"); await tick(); resolve();
    await vi.waitFor(() => expect(document.querySelector<HTMLButtonElement>(".composer-actions .primary")!.disabled).toBe(false));
    expect(document.querySelector<HTMLTextAreaElement>("#composer-input")!.value).toBe("new draft");
  });
  it("keeps approvals visible after a transmission failure and permits a retry", async () => {
    mocks.api.uiRespond.mockRejectedValueOnce(new Error("broken pipe")).mockResolvedValueOnce({ accepted: true });
    mocks.event?.({ kind: "ui_request", request: { type: "extension_ui_request", id: "approval", method: "confirm", title: "Allow edit?" } });
    await tick();
    const allow = () => Array.from(document.querySelectorAll<HTMLButtonElement>(".modal button")).find((button) => button.textContent === "Allow")!;
    allow().click();
    await vi.waitFor(() => expect(document.querySelector('[data-testid="error-banner"]')?.textContent).toContain("broken pipe"));
    expect(document.querySelector('[data-testid="approval-modal"]')).toBeTruthy();
    allow().click();
    await vi.waitFor(() => expect(document.querySelector('[data-testid="approval-modal"]')).toBeNull());
    expect(mocks.api.uiRespond).toHaveBeenCalledTimes(2);
  });
  it("requests older messages from the host using its absolute cursor", async () => {
    mocks.api.getMessages.mockResolvedValueOnce({ messages: [], total: 450, before: 250, hasMore: true })
      .mockResolvedValueOnce({ messages: [{ id: "earlier", role: "assistant", content: "older reply" }], total: 450, before: 50, hasMore: true });
    mocks.event?.({ kind: "rpc", event: { type: "message_start", message: { id: "live", role: "user", content: "hello" } } });
    await tick();
    document.querySelector<HTMLButtonElement>('[data-testid="load-earlier"]')!.click();
    await vi.waitFor(() => expect(document.querySelector(".transcript")?.textContent).toContain("older reply"));
    expect(mocks.api.getMessages).toHaveBeenLastCalledWith(250);
  });
});
