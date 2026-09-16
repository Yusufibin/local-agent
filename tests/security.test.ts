import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { onToolCall as workspaceGate } from "../agent-runtime/extensions/workspace-roots";
import { setWorkspace } from "../agent-runtime/extensions/workspace-roots";
import { onToolCall as protectedGate, refreshWorkspace as refreshProtected } from "../agent-runtime/extensions/protected-paths";
import {
  onToolCall as permissionGate,
  refreshWorkspace as refreshPerm,
  setMode,
} from "../agent-runtime/extensions/permission-gate";
import type { WorkspaceConfig } from "../agent-runtime/policy/paths";

const tmpDirs: string[] = [];

function tmpRoot(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "deskpi-sec-"));
  tmpDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const d of tmpDirs) fs.rmSync(d, { recursive: true, force: true });
  tmpDirs.length = 0;
});

function wsFor(root: string, mode = "ask"): WorkspaceConfig {
  return {
    activeRoot: root,
    roots: [root],
    permissionMode: mode,
    denyReadGlobs: [],
  };
}

function writeWorkspace(dir: string, mode = "ask") {
  const file = path.join(dir, "workspace.json");
  fs.writeFileSync(
    file,
    JSON.stringify({
      activeRoot: dir,
      roots: [dir],
      permissionMode: mode,
      denyReadGlobs: [],
    }),
  );
  process.env.DESKPI_WORKSPACE_FILE = file;
  process.env.DESKPI_PERMISSION_MODE = mode;
  setWorkspace(wsFor(dir, mode));
  refreshProtected();
  refreshPerm();
  setMode(mode as "ask" | "full" | "readonly");
}

function fakeUi(log: string[]) {
  return {
    cwd: "",
    hasUI: true,
    ui: {
      confirm: async (title: string, message: string) => {
        log.push(`confirm:${title}:${message}`);
        return true;
      },
      select: async (title: string, options: string[]) => {
        log.push(`select:${title}:${options.join("|")}`);
        return "Allow";
      },
      notify: (m: string) => log.push(`notify:${m}`),
      setStatus: () => {},
      setWidget: () => {},
    },
  };
}

describe("shipped security extensions", () => {
  it("allows in-root read", async () => {
    const root = tmpRoot();
    fs.writeFileSync(path.join(root, "ok.txt"), "hi");
    writeWorkspace(root, "ask");
    const ctx = { ...fakeUi([]), cwd: root };
    const r = workspaceGate({ toolName: "read", input: { path: "ok.txt" } }, ctx);
    expect(r).toBeUndefined();
  });

  it("find without path stays on cwd (in-root); find / is blocked", () => {
    const root = tmpRoot();
    writeWorkspace(root, "ask");
    const ctx = { cwd: root };
    const local = workspaceGate({ toolName: "find", input: { pattern: "*fred*" } }, ctx);
    expect(local).toBeUndefined();
    const outside = workspaceGate(
      { toolName: "find", input: { path: "/", pattern: "*fred*" } },
      ctx,
    );
    expect(outside?.block).toBe(true);
    expect(outside?.reason).toBe("path outside workspace roots");
  });

  it("blocks ../ escape and /etc/passwd with path outside workspace roots", () => {
    const root = tmpRoot();
    writeWorkspace(root, "full");
    const ctx = { cwd: root };
    const escape = workspaceGate(
      { toolName: "write", input: { path: "../escape.txt" } },
      ctx,
    );
    expect(escape?.block).toBe(true);
    expect(escape?.reason).toBe("path outside workspace roots");
    const etc = workspaceGate(
      { toolName: "read", input: { path: "/etc/passwd" } },
      ctx,
    );
    expect(etc?.block).toBe(true);
    expect(etc?.reason).toBe("path outside workspace roots");
  });

  it("blocks .env and .ssh even in full", () => {
    const root = tmpRoot();
    fs.writeFileSync(path.join(root, ".env"), "SECRET=1");
    fs.mkdirSync(path.join(root, ".ssh"));
    fs.writeFileSync(path.join(root, ".ssh", "id_rsa"), "k");
    writeWorkspace(root, "full");
    const ctx = { ...fakeUi([]), cwd: root };
    const envBlock = protectedGate({ toolName: "read", input: { path: ".env" } }, ctx);
    expect(envBlock?.block).toBe(true);
    const sshBlock = protectedGate(
      { toolName: "read", input: { path: ".ssh/id_rsa" } },
      ctx,
    );
    expect(sshBlock?.block).toBe(true);
  });

  it("readonly write/bash block without a UI call", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "readonly");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root, hasUI: true };
    const w = await permissionGate({ toolName: "write", input: { path: "ok.md", content: "x" } }, ctx);
    const b = await permissionGate({ toolName: "bash", input: { command: "echo hi" } }, ctx);
    expect(w).toEqual({ block: true, reason: "write/edit blocked in readonly mode" });
    expect(b).toEqual({ block: true, reason: "bash blocked in readonly mode" });
    expect(log.filter((l) => l.startsWith("confirm:") || l.startsWith("select:")).length).toBe(0);
  });

  it("ask write → confirm, bash → select", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "ask");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root };
    const w = await permissionGate({ toolName: "write", input: { path: "ok.md", content: "hello" } }, ctx);
    expect(w).toBeUndefined();
    expect(log.some((l) => l.startsWith("confirm:"))).toBe(true);
    const b = await permissionGate({ toolName: "bash", input: { command: "echo hi" } }, ctx);
    expect(b).toBeUndefined();
    expect(log.some((l) => l.startsWith("select:") && l.includes("Allow|Block"))).toBe(true);
  });

  it("full rm -rf still asks", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "full");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root };
    await permissionGate({ toolName: "bash", input: { command: "rm -rf /tmp/x" } }, ctx);
    expect(log.some((l) => l.startsWith("select:"))).toBe(true);
  });

  it("hasUI === false blocks", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "ask");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root, hasUI: false };
    const r = await permissionGate({ toolName: "write", input: { path: "ok.md", content: "x" } }, ctx);
    expect(r?.block).toBe(true);
    expect(log.filter((l) => l.startsWith("confirm:") || l.startsWith("select:")).length).toBe(0);
  });

  it("Block response is { block: true } and does not execute a write", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "ask");
    const target = path.join(root, "ok.md");
    const ctx = {
      cwd: root,
      hasUI: true,
      ui: {
        confirm: async () => false,
        select: async () => "Block",
        notify: () => {},
        setStatus: () => {},
        setWidget: () => {},
      },
    };
    const r = await permissionGate(
      { toolName: "write", input: { path: "ok.md", content: "should-not-land" } },
      ctx,
    );
    expect(r).toEqual({ block: true, reason: "Blocked by user" });
    expect(fs.existsSync(target)).toBe(false);
  });
});

/**
 * Phase 4 checklist (README). Chains workspace-roots → protected-paths →
 * permission-gate, then performs the write only if every gate allows — the
 * same order Pi uses for `tool_call` (`{ block: true }` first wins).
 */
async function runWriteChain(
  root: string,
  rel: string,
  content: string,
  opts: { mode: string; confirm: boolean },
): Promise<{ block: true; reason: string } | undefined> {
  writeWorkspace(root, opts.mode);
  const log: string[] = [];
  const ctx = {
    ...fakeUi(log),
    cwd: root,
    hasUI: true,
    ui: {
      ...fakeUi(log).ui,
      confirm: async (title: string, message: string) => {
        log.push(`confirm:${title}:${message}`);
        return opts.confirm;
      },
      select: async (title: string, options: string[]) => {
        log.push(`select:${title}:${options.join("|")}`);
        return opts.confirm ? "Allow" : "Block";
      },
    },
  };
  const input = { path: rel, content };
  const ws = workspaceGate({ toolName: "write", input }, ctx);
  if (ws?.block) return ws;
  const prot = protectedGate({ toolName: "write", input }, ctx);
  if (prot?.block) return prot;
  const perm = await permissionGate({ toolName: "write", input }, ctx);
  if (perm?.block) return perm;
  const dest = path.resolve(root, rel);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.writeFileSync(dest, content);
  return undefined;
}

describe("Phase 4 security checklist", () => {
  it("read /etc/passwd → block", () => {
    const root = tmpRoot();
    writeWorkspace(root, "ask");
    const r = workspaceGate(
      { toolName: "read", input: { path: "/etc/passwd" } },
      { cwd: root },
    );
    expect(r).toEqual({ block: true, reason: "path outside workspace roots" });
  });

  it("read <root>/.env → block", () => {
    const root = tmpRoot();
    fs.writeFileSync(path.join(root, ".env"), "SECRET=1");
    writeWorkspace(root, "full");
    const r = protectedGate(
      { toolName: "read", input: { path: ".env" } },
      { cwd: root },
    );
    expect(r?.block).toBe(true);
  });

  it("write <root>/../escape.txt → block", async () => {
    const root = tmpRoot();
    const outside = path.join(root, "..", `escape-${path.basename(root)}.txt`);
    const r = await runWriteChain(root, "../escape.txt", "nope", {
      mode: "full",
      confirm: true,
    });
    expect(r?.block).toBe(true);
    expect(r?.reason).toBe("path outside workspace roots");
    expect(fs.existsSync(outside)).toBe(false);
  });

  it("write <root>/ok.md en ask → modal → Allow → fichier créé", async () => {
    const root = tmpRoot();
    const r = await runWriteChain(root, "ok.md", "hello", {
      mode: "ask",
      confirm: true,
    });
    expect(r).toBeUndefined();
    expect(fs.readFileSync(path.join(root, "ok.md"), "utf8")).toBe("hello");
  });

  it("même write → Block → pas de fichier", async () => {
    const root = tmpRoot();
    const r = await runWriteChain(root, "ok.md", "hello", {
      mode: "ask",
      confirm: false,
    });
    expect(r).toEqual({ block: true, reason: "Blocked by user" });
    expect(fs.existsSync(path.join(root, "ok.md"))).toBe(false);
  });

  it("bash: rm -rf /tmp/x → modal même en full", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "full");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root };
    await permissionGate({ toolName: "bash", input: { command: "rm -rf /tmp/x" } }, ctx);
    expect(log.some((l) => l.startsWith("select:"))).toBe(true);
  });

  it("readonly + write → block sans modal", async () => {
    const root = tmpRoot();
    writeWorkspace(root, "readonly");
    const log: string[] = [];
    const ctx = { ...fakeUi(log), cwd: root };
    const r = await permissionGate(
      { toolName: "write", input: { path: "ok.md", content: "x" } },
      ctx,
    );
    expect(r).toEqual({ block: true, reason: "write/edit blocked in readonly mode" });
    expect(log.filter((l) => l.startsWith("confirm:") || l.startsWith("select:")).length).toBe(0);
    expect(fs.existsSync(path.join(root, "ok.md"))).toBe(false);
  });
});