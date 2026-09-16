import fs from "node:fs";
import path from "node:path";
import { normalizePath } from "./globs";

export const PATH_TOOLS = new Set(["read", "write", "edit", "ls", "grep", "find"]);

export const OUTSIDE_ROOTS_REASON = "path outside workspace roots";

export type WorkspaceConfig = {
  activeRoot: string;
  roots: string[];
  permissionMode: "readonly" | "ask" | "full" | string;
  denyReadGlobs: string[];
  settingsFile?: string;
  secretsFile?: string;
};

export function loadWorkspaceFromFile(file: string): WorkspaceConfig {
  const raw = fs.readFileSync(file, "utf8");
  const parsed = JSON.parse(raw) as Record<string, unknown>;
  const activeRoot = String(parsed.activeRoot ?? parsed.active_root ?? "");
  const roots = Array.isArray(parsed.roots) ? parsed.roots.map(String) : [];
  if (activeRoot && !roots.includes(activeRoot)) roots.unshift(activeRoot);
  return {
    activeRoot,
    roots,
    permissionMode: String(parsed.permissionMode ?? parsed.permission_mode ?? "ask"),
    denyReadGlobs: Array.isArray(parsed.denyReadGlobs)
      ? parsed.denyReadGlobs.map(String)
      : Array.isArray(parsed.deny_read_globs)
        ? parsed.deny_read_globs.map(String)
        : [],
    settingsFile: parsed.settingsFile ? String(parsed.settingsFile) : undefined,
    secretsFile: parsed.secretsFile ? String(parsed.secretsFile) : undefined,
  };
}

export function loadWorkspaceFromEnv(
  env: NodeJS.Dict<string> = process.env,
): WorkspaceConfig | null {
  const file = env.DESKPI_WORKSPACE_FILE;
  if (!file) return null;
  if (!fs.existsSync(file)) return null;
  const ws = loadWorkspaceFromFile(file);
  if (env.DESKPI_PERMISSION_MODE) ws.permissionMode = env.DESKPI_PERMISSION_MODE;
  return ws;
}

/** Realpath an existing prefix so writes to new files still resolve .. escapes. */
export function resolveExisting(abs: string): string {
  let current = abs;
  const suffix: string[] = [];
  while (current && current !== path.parse(current).root) {
    try {
      const real = fs.realpathSync(current);
      return path.join(real, ...suffix.reverse());
    } catch {
      suffix.push(path.basename(current));
      current = path.dirname(current);
    }
  }
  try {
    return fs.realpathSync(abs);
  } catch {
    return path.resolve(abs);
  }
}

export function resolveToolPath(p: string, cwd: string): string {
  const abs = path.resolve(cwd, p);
  return resolveExisting(abs);
}

export function isUnderRoot(realPath: string, root: string): boolean {
  let realRoot: string;
  try {
    realRoot = fs.realpathSync(root);
  } catch {
    realRoot = path.resolve(root);
  }
  const a = normalizePath(realPath);
  const b = normalizePath(realRoot);
  if (a === b) return true;
  const prefix = b.endsWith("/") ? b : b + "/";
  return a.startsWith(prefix);
}

export function isUnderAnyRoot(realPath: string, roots: string[]): boolean {
  return roots.some((r) => isUnderRoot(realPath, r));
}

export function extractPaths(toolName: string, input: Record<string, unknown>): string[] {
  const out: string[] = [];
  const push = (v: unknown) => {
    if (typeof v === "string" && v.length > 0) out.push(v);
  };
  push(input.path);
  push(input.file);
  push(input.target);
  push(input.dir);
  push(input.directory);
  push(input.searchPath);
  if (Array.isArray(input.paths)) input.paths.forEach(push);
  if (
    (toolName === "ls" || toolName === "find" || toolName === "grep") &&
    out.length === 0
  ) {
    out.push(".");
  }
  return out;
}

export type BlockResult = { block: true; reason: string };

export function gateWorkspacePaths(
  toolName: string,
  input: Record<string, unknown>,
  cwd: string,
  workspace: WorkspaceConfig,
): BlockResult | undefined {
  if (!PATH_TOOLS.has(toolName)) return undefined;
  const paths = extractPaths(toolName, input);
  for (const p of paths) {
    const real = resolveToolPath(p, cwd);
    if (!isUnderAnyRoot(real, workspace.roots)) {
      return { block: true, reason: OUTSIDE_ROOTS_REASON };
    }
  }
  return undefined;
}
