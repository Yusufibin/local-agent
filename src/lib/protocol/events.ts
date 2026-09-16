export type ProcessStatus = "spawned" | "exited" | "crashed";

export type PiEvent = {
  type: string;
  [key: string]: unknown;
};

export type ExtensionUiRequest = {
  type: "extension_ui_request";
  id: string;
  method: "select" | "confirm" | "input" | "editor" | "notify" | "setStatus" | "setWidget" | "setTitle" | "set_editor_text" | string;
  title?: string;
  message?: string;
  options?: string[];
  placeholder?: string;
  prefill?: string;
  timeout?: number;
  notifyType?: string;
  statusKey?: string;
  statusText?: string;
  widgetKey?: string;
  widgetLines?: string[];
  widgetPlacement?: string;
  text?: string;
};

export type HostEvent =
  | { kind: "rpc"; event: PiEvent }
  | { kind: "process"; status: ProcessStatus; code?: number; message?: string }
  | { kind: "ui_request"; request: ExtensionUiRequest }
  | { kind: "log"; level: "info" | "warn" | "error" | string; message: string }
  | { kind: "watchdog"; silenceMs: number };