import path from "node:path";
import { pathMatchesAny } from "./globs";
import type { WorkspaceConfig } from "./paths";

/** Always-blocked globs (even in full). `.git/objects` is write-only. */
export const ALWAYS_PROTECTED_GLOBS = [
  "**/.env",
  "**/.env.*",
  "**/secrets.*",
  "**/*credential*",
  "**/.ssh/**",
  "**/.gnupg/**",
  "**/.aws/**",
];

export const WRITE_ONLY_PROTECTED_GLOBS = ["**/.git/objects/**"];

export function isProtectedPath(
  realPath: string,
  toolName: string,
  workspace?: WorkspaceConfig | null,
): boolean {
  const extra = workspace?.denyReadGlobs ?? [];
  if (pathMatchesAny(realPath, ALWAYS_PROTECTED_GLOBS)) return true;
  if (pathMatchesAny(realPath, extra)) return true;
  if (
    (toolName === "write" || toolName === "edit") &&
    pathMatchesAny(realPath, WRITE_ONLY_PROTECTED_GLOBS)
  ) {
    return true;
  }
  if (workspace?.settingsFile && sameFile(realPath, workspace.settingsFile)) return true;
  if (workspace?.secretsFile && sameFile(realPath, workspace.secretsFile)) return true;
  return false;
}

function sameFile(a: string, b: string): boolean {
  return path.resolve(a) === path.resolve(b);
}

export function protectedReason(realPath: string): string {
  return `Path "${realPath}" is protected`;
}
