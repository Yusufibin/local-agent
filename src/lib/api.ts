/** Typed invoke wrappers. The webview never talks to Pi JSONL directly. */

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export const api = {
  agentStart: () => invoke("agent_start"),
  agentStop: () => invoke("agent_stop"),
  agentRestart: () => invoke("agent_restart"),
  prompt: (text: string, behavior?: "steer" | "followUp") =>
    invoke("prompt", { text, behavior }),
  steer: (text: string) => invoke("steer", { text }),
  followUp: (text: string) => invoke("follow_up", { text }),
  abort: () => invoke<{ steering?: string[]; followUp?: string[] }>("abort"),
  newSession: () => invoke("new_session"),
  switchSession: (path: string) => invoke("switch_session", { path }),
  listSessions: () => invoke<unknown[]>("list_sessions"),
  getState: () => invoke<Record<string, unknown>>("get_state"),
  getMessages: () => invoke<{ messages?: unknown[] }>("get_messages"),
  setModel: (provider: string, modelId: string) =>
    invoke("set_model", { provider, modelId }),
  setThinkingLevel: (level: string) => invoke("set_thinking_level", { level }),
  getAvailableModels: () => invoke<{ models?: unknown[] }>("get_available_models"),
  getSessionStats: () => invoke<Record<string, unknown>>("get_session_stats"),
  compact: () => invoke("compact"),
  getCommands: () => invoke("get_commands"),
  uiRespond: (id: string, payload: Record<string, unknown>) =>
    invoke("ui_respond", { id, payload }),
  pickWorkspace: () => invoke("pick_workspace"),
  saveSecret: (provider: string, key: string) => invoke("save_secret", { provider, key }),
  getSettings: () => invoke<Record<string, unknown>>("get_settings"),
  setPermissionMode: (mode: string) => invoke("set_permission_mode", { mode }),
  addRoot: () => invoke("add_root"),
  installHint: () => invoke<string>("install_hint"),
};