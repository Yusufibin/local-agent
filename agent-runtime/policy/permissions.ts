export type PermissionMode = "readonly" | "ask" | "full";

export type UiKind = "confirm" | "select" | "none";

export type GateDecision =
  | { action: "allow" }
  | { action: "block"; reason: string; ui: false }
  | {
      action: "ask";
      ui: "confirm" | "select";
      title: string;
      message: string;
      options?: string[];
    };

const DANGEROUS_BASH = [
  /\brm\s+(-rf?|--recursive)/i,
  /\bsudo\b/i,
  /\b(chmod|chown)\b.*777/i,
  /\bmkfs\b/i,
  /\bdd\s+if=/i,
  /curl[\s\S]*\|\s*(ba)?sh/i,
  /wget[\s\S]*\|\s*(ba)?sh/i,
  /~\/\.ssh/,
  /(?:^|[\s'"])\/?(?:home\/[^/]+\/)?\.ssh\//,
];

export function isDangerousBash(command: string): boolean {
  return DANGEROUS_BASH.some((p) => p.test(command));
}

export function normalizeMode(mode: string | undefined): PermissionMode {
  if (mode === "readonly" || mode === "full" || mode === "ask") return mode;
  return "ask";
}

/**
 * Permission-gate decision. Workspace/protected checks run first in those modules.
 * `hasUI === false` never YOLO-allows write/edit/bash.
 * `readonly` write/bash block without a UI call.
 */
export function permissionDecision(opts: {
  mode: string;
  toolName: string;
  hasUI: boolean;
  command?: string;
  relativePath?: string;
  diff?: string;
  writeBytes?: number;
  cwd?: string;
}): GateDecision {
  const mode = normalizeMode(opts.mode);
  const tool = opts.toolName;

  if (tool === "read" || tool === "ls" || tool === "grep" || tool === "find") {
    return { action: "allow" };
  }

  if (tool === "write" || tool === "edit") {
    if (!opts.hasUI) {
      return {
        action: "block",
        reason: "Blocked (no UI for confirmation)",
        ui: false,
      };
    }
    if (mode === "readonly") {
      return { action: "block", reason: "write/edit blocked in readonly mode", ui: false };
    }
    if (mode === "full") {
      return { action: "allow" };
    }
    const rel = opts.relativePath ?? "(file)";
    const extra =
      tool === "edit" && opts.diff
        ? `\n\n${opts.diff}`
        : tool === "write" && opts.writeBytes != null
          ? `\n\nsize: ${opts.writeBytes} bytes`
          : "";
    return {
      action: "ask",
      ui: "confirm",
      title: `Allow ${tool}?`,
      message: `${rel}${extra}`,
    };
  }

  if (tool === "bash") {
    const command = opts.command ?? "";
    const dangerous = isDangerousBash(command);
    if (!opts.hasUI) {
      return {
        action: "block",
        reason: dangerous
          ? "Dangerous command blocked (no UI for confirmation)"
          : "bash blocked (no UI for confirmation)",
        ui: false,
      };
    }
    if (mode === "readonly") {
      return { action: "block", reason: "bash blocked in readonly mode", ui: false };
    }
    const mustAsk = mode === "ask" || dangerous;
    if (!mustAsk) return { action: "allow" };
    const cwd = opts.cwd ? `\ncwd: ${opts.cwd}` : "";
    return {
      action: "ask",
      ui: "select",
      title: "Allow shell command?",
      message: `${command}${cwd}\n\naccès shell réel, hors sandbox`,
      options: ["Allow", "Block"],
    };
  }

  return { action: "allow" };
}

export async function applyPermissionDecision(
  decision: GateDecision,
  ui: {
    confirm: (title: string, message: string) => Promise<boolean | undefined>;
    select: (title: string, options: string[]) => Promise<string | undefined>;
  },
): Promise<{ block: true; reason: string } | undefined> {
  if (decision.action === "allow") return undefined;
  if (decision.action === "block") return { block: true, reason: decision.reason };
  if (decision.ui === "confirm") {
    const ok = await ui.confirm(decision.title, decision.message);
    if (!ok) return { block: true, reason: "Blocked by user" };
    return undefined;
  }
  const choice = await ui.select(
    `${decision.title}\n\n${decision.message}`,
    decision.options ?? ["Allow", "Block"],
  );
  if (choice !== "Allow") return { block: true, reason: "Blocked by user" };
  return undefined;
}
