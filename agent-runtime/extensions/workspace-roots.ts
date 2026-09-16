/**
 * Workspace roots allowlist. Paths of read/write/edit/ls/grep/find must stay
 * under configured roots after realpath. Bash is not parsed for sandboxing.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  gateWorkspacePaths,
  loadWorkspaceFromEnv,
  type WorkspaceConfig,
} from "../policy/paths";

type ToolCallEvent = { toolName: string; input: Record<string, unknown> };
type ExtCtx = { cwd: string; hasUI?: boolean };

let workspace: WorkspaceConfig | null = null;

export function getWorkspace(): WorkspaceConfig | null {
  return workspace;
}

export function setWorkspace(ws: WorkspaceConfig | null): void {
  workspace = ws;
}

export function refreshWorkspace(
  env: NodeJS.Dict<string> = process.env,
): WorkspaceConfig | null {
  workspace = loadWorkspaceFromEnv(env);
  return workspace;
}

export function onToolCall(
  event: ToolCallEvent,
  ctx: ExtCtx,
): { block: true; reason: string } | undefined {
  const ws = workspace ?? refreshWorkspace();
  if (!ws) {
    return { block: true, reason: "workspace roots not configured" };
  }
  if (event.toolName === "bash") {
    const mode = (ws.permissionMode || "ask").toLowerCase();
    if (mode === "readonly") {
      return { block: true, reason: "bash blocked in readonly mode" };
    }
    return undefined;
  }
  return gateWorkspacePaths(event.toolName, event.input ?? {}, ctx.cwd, ws);
}

function overlayText(ws: WorkspaceConfig): string {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const systemPath = path.join(here, "..", "SYSTEM.md");
  let system = "";
  try {
    system = fs.readFileSync(systemPath, "utf8");
  } catch {
    system = "";
  }
  const roots = ws.roots.map((r) => `- ${r}`).join("\n");
  return `${system}\n\nWorkspace roots (do not leave these paths):\n${roots}\n`;
}

export default function workspaceRootsExtension(pi: {
  on: (event: string, handler: (...args: any[]) => unknown) => void;
}) {
  pi.on("session_start", (_event: unknown, ctx: ExtCtx) => {
    refreshWorkspace();
    const ws = workspace;
    if (ws && (ctx as ExtCtx & { ui?: { setWidget?: Function } }).ui?.setWidget) {
      (ctx as any).ui.setWidget("roots", [
        "roots:",
        ...ws.roots.map((r) => `  ${r}`),
      ]);
    }
  });

  pi.on("tool_call", async (event: ToolCallEvent, ctx: ExtCtx) => {
    return onToolCall(event, ctx);
  });

  pi.on("before_agent_start", (event: { systemPrompt?: string }) => {
    const ws = workspace ?? refreshWorkspace();
    if (!ws) return;
    return {
      systemPrompt: `${event.systemPrompt ?? ""}\n\n${overlayText(ws)}`,
    };
  });
}
