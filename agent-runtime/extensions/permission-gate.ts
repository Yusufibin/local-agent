/**
 * Permission gate: readonly / ask / full, plus /permission command.
 * Never YOLO when ctx.hasUI === false.
 */
import path from "node:path";
import {
  extractPaths,
  loadWorkspaceFromEnv,
  resolveToolPath,
  type WorkspaceConfig,
} from "../policy/paths";
import { isProtectedPath, protectedReason } from "../policy/protected";
import {
  applyPermissionDecision,
  normalizeMode,
  permissionDecision,
  type PermissionMode,
} from "../policy/permissions";

type ToolCallEvent = { toolName: string; input: Record<string, unknown> };
type ExtCtx = {
  cwd: string;
  hasUI: boolean;
  ui: {
    confirm: (title: string, message: string) => Promise<boolean | undefined>;
    select: (title: string, options: string[]) => Promise<string | undefined>;
    notify: (message: string, kind?: string) => void;
    setStatus: (key: string, text?: string) => void;
    setWidget: (key: string, lines?: string[]) => void;
  };
};

let workspace: WorkspaceConfig | null = null;
let modeOverride: PermissionMode | null = null;

export function refreshWorkspace(env: NodeJS.Dict<string> = process.env) {
  workspace = loadWorkspaceFromEnv(env);
  return workspace;
}

export function currentMode(): PermissionMode {
  if (modeOverride) return modeOverride;
  return normalizeMode(workspace?.permissionMode ?? process.env.DESKPI_PERMISSION_MODE);
}

export function setMode(mode: PermissionMode) {
  modeOverride = mode;
}

export async function onToolCall(
  event: ToolCallEvent,
  ctx: ExtCtx,
): Promise<{ block: true; reason: string } | undefined> {
  const ws = workspace ?? refreshWorkspace();
  const tool = event.toolName;
  const input = (event.input ?? {}) as Record<string, unknown>;

  if (["read", "write", "edit", "ls", "grep", "find"].includes(tool)) {
    const paths = extractPaths(tool, input);
    for (const p of paths) {
      const real = resolveToolPath(p, ctx.cwd);
      if (isProtectedPath(real, tool, ws)) {
        return { block: true, reason: protectedReason(real) };
      }
    }
  }

  const rel =
    typeof input.path === "string"
      ? path.relative(ws?.activeRoot || ctx.cwd, resolveToolPath(String(input.path), ctx.cwd))
      : undefined;
  const content = typeof input.content === "string" ? input.content : undefined;
  const diff =
    typeof input.diff === "string"
      ? input.diff
      : typeof input.newText === "string" && typeof input.oldText === "string"
        ? `--- old\n${input.oldText}\n+++ new\n${input.newText}`
        : undefined;

  const decision = permissionDecision({
    mode: currentMode(),
    toolName: tool,
    hasUI: ctx.hasUI === true,
    command: typeof input.command === "string" ? input.command : undefined,
    relativePath: rel,
    diff,
    writeBytes: content ? Buffer.byteLength(content) : undefined,
    cwd: ctx.cwd,
  });

  return applyPermissionDecision(decision, ctx.ui);
}

export default function permissionGateExtension(pi: {
  on: (event: string, handler: (...args: any[]) => unknown) => void;
  registerCommand: (
    name: string,
    opts: { description?: string; handler: (args: string, ctx: ExtCtx) => unknown },
  ) => void;
}) {
  pi.on("session_start", (_e: unknown, ctx: ExtCtx) => {
    refreshWorkspace();
    ctx.ui?.setStatus?.("permission", `permissionMode: ${currentMode()}`);
  });

  pi.on("tool_call", async (event: ToolCallEvent, ctx: ExtCtx) => onToolCall(event, ctx));

  pi.registerCommand("permission", {
    description: "Set permission mode: ask | full | readonly",
    handler: async (args, ctx) => {
      const next = args.trim().split(/\s+/)[0];
      if (!["ask", "full", "readonly"].includes(next)) {
        ctx.ui.notify("Usage: /permission ask|full|readonly", "warning");
        return;
      }
      setMode(next as PermissionMode);
      ctx.ui.setStatus("permission", `permissionMode: ${next}`);
      ctx.ui.notify(`permission mode: ${next}`, "info");
    },
  });
}
