import type { HostEvent } from "./protocol/events";

export async function listenHostEvents(onEvent: (event: HostEvent) => void): Promise<() => void> {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<HostEvent>("agent://event", (e) => onEvent(e.payload));
    return () => {
      un();
    };
  } catch {
    return () => {};
  }
}