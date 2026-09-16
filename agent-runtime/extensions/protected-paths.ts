/**
 * Unconditional protected-path blocks (even in full).
 */
import {
  extractPaths,
  loadWorkspaceFromEnv,
  resolveToolPath,
  type WorkspaceConfig,
} from "../policy/paths";
import { isProtectedPath, protectedReason } from "../policy/protected";

type ToolCallEvent = { toolName: string; input: Record<string, unknown> };
type ExtCtx = {
  cwd: string;
  hasUI?: boolean;
  ui?: { notify?: (message: string, kind?: string) => void };
};

let workspace: WorkspaceConfig | null = null;

export function refreshWorkspace(env: NodeJS.Dict<string> = process.env) {
  workspace = loadWorkspaceFromEnv(env);
  return workspace;
}

export function onToolCall(
  event: ToolCallEvent,
  ctx: ExtCtx,
): { block: true; reason: string } | undefined {
  const ws = workspace ?? refreshWorkspace();
  const tools = new Set(["read", "write", "edit", "ls", "grep", "find"]);
  if (!tools.has(event.toolName)) return undefined;
  const paths = extractPaths(event.toolName, event.input ?? {});
  for (const p of paths) {
    const real = resolveToolPath(p, ctx.cwd);
    if (isProtectedPath(real, event.toolName, ws)) {
      ctx.ui?.notify?.(`Blocked access to protected path: ${p}`, "warning");
      return { block: true, reason: protectedReason(real) };
    }
  }
  return undefined;
}

export default function protectedPathsExtension(pi: {
  on: (event: string, handler: (...args: any[]) => unknown) => void;
}) {
  pi.on("session_start", () => {
    refreshWorkspace();
  });
  pi.on("tool_call", async (event: ToolCallEvent, ctx: ExtCtx) => onToolCall(event, ctx));
}
