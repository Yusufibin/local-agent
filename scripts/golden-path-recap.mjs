#!/usr/bin/env node
/**
 * Phase 5 golden path: spawn real `pi --mode rpc` with the product runtime
 * against a copy of tests/fixtures/Fred-Projet and prompt
 * « fais-moi un récap du dossier de fred ».
 *
 * Requires `pi` on PATH and a working provider (Pi reads ~/.pi/agent/auth.json
 * via HOME). Does not call a live LLM from unit tests — run via `pnpm golden`.
 */
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runtime = process.env.DESKPI_RUNTIME || path.join(root, "agent-runtime");
const fixture = path.join(root, "tests/fixtures/Fred-Projet");
const outFile = path.join(root, "tests/golden-path-output.md");
const timeoutMs = Number(process.env.DESKPI_GOLDEN_TIMEOUT_MS || 300_000);

function fail(msg, extra) {
  console.error(msg);
  if (extra) console.error(extra);
  process.exit(1);
}

if (!fs.existsSync(path.join(runtime, "extensions/workspace-roots.ts"))) {
  fail(`agent-runtime missing at ${runtime} (set DESKPI_RUNTIME)`);
}
if (!fs.existsSync(fixture)) fail(`missing fixture ${fixture}`);

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "deskpi-golden-"));
const ws = tmp;
const fred = path.join(ws, "Fred-Projet");
fs.cpSync(fixture, fred, { recursive: true });
const sessionDir = path.join(tmp, "sessions");
fs.mkdirSync(sessionDir, { recursive: true });
const workspaceFile = path.join(tmp, "workspace.json");
fs.writeFileSync(
  workspaceFile,
  JSON.stringify(
    {
      activeRoot: ws,
      roots: [ws],
      permissionMode: "ask",
      denyReadGlobs: ["**/.env", "**/.env.*", "**/*id_rsa*", "**/*.pem"],
    },
    null,
    2,
  ),
);

const pi = process.env.PI_BIN || "pi";
const args = [
  "--mode",
  "rpc",
  "--session-dir",
  sessionDir,
  "--no-extensions",
  "-e",
  path.join(runtime, "extensions/workspace-roots.ts"),
  "-e",
  path.join(runtime, "extensions/permission-gate.ts"),
  "-e",
  path.join(runtime, "extensions/protected-paths.ts"),
  "--no-skills",
  "--skill",
  path.join(runtime, "skills/recap-dossier"),
  "--skill",
  path.join(runtime, "skills/trouver-dossier"),
  "--no-prompt-templates",
  "--prompt-template",
  path.join(runtime, "prompts/recap.md"),
  "--thinking",
  "off",
];
if (process.env.DESKPI_PROVIDER) {
  args.push("--provider", process.env.DESKPI_PROVIDER);
}
if (process.env.DESKPI_MODEL) {
  args.push("--model", process.env.DESKPI_MODEL);
}

const env = {
  HOME: process.env.HOME,
  USER: process.env.USER,
  PATH: process.env.PATH,
  LANG: process.env.LANG || "C",
  TERM: "dumb",
  NO_COLOR: "1",
  DESKPI_WORKSPACE_FILE: workspaceFile,
  DESKPI_PERMISSION_MODE: "ask",
};

const stderrLog = path.join(tmp, "pi.stderr.log");
const stderrFd = fs.openSync(stderrLog, "w");
const child = spawn(pi, args, {
  cwd: ws,
  env,
  stdio: ["pipe", "pipe", stderrFd],
  detached: true,
});
if (!child.pid) fail("failed to spawn pi");

function cleanup() {
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    /* already dead */
  }
  setTimeout(() => {
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch {
      /* ignore */
    }
  }, 1500).unref();
}
process.on("exit", cleanup);
process.on("SIGINT", () => process.exit(130));
process.on("SIGTERM", () => process.exit(143));

let buf = "";
const events = [];
const assistantChunks = [];
const toolNames = [];
const uiRequests = [];
const apiErrors = [];
const pending = new Map();
let settled = false;
let seq = 0;

function send(obj) {
  const id = obj.id || `g-${++seq}`;
  const payload = { ...obj, id };
  child.stdin.write(JSON.stringify(payload) + "\n");
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`rpc timeout for ${payload.type}`));
    }, 15_000);
    pending.set(id, (resp) => {
      clearTimeout(t);
      resolve(resp);
    });
  });
}

function dangerousBash(cmd) {
  return /\brm\s+(-rf?|--recursive)|\bsudo\b|\bmkfs\b|\bdd\s+if=/i.test(cmd);
}

function handleUi(msg) {
  const method = msg.method;
  uiRequests.push(msg);
  if (method === "notify" || method === "setStatus" || method === "setWidget" || method === "setTitle") {
    return;
  }
  const id = msg.id;
  if (method === "confirm") {
    child.stdin.write(JSON.stringify({ type: "extension_ui_response", id, confirmed: true }) + "\n");
    return;
  }
  if (method === "select") {
    const cmd = String(msg.message || msg.title || "");
    const value = dangerousBash(cmd) ? "Block" : "Allow";
    child.stdin.write(JSON.stringify({ type: "extension_ui_response", id, value }) + "\n");
    return;
  }
  child.stdin.write(JSON.stringify({ type: "extension_ui_response", id, cancelled: true }) + "\n");
}

function onLine(line) {
  if (!line) return;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch {
    return;
  }
  events.push(msg.type || "?");
  if (msg.type === "response" && msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
    return;
  }
  if (msg.type === "extension_ui_request") {
    handleUi(msg);
    return;
  }
  if (msg.type === "agent_settled") settled = true;
  if (msg.type === "tool_execution_start") toolNames.push(String(msg.toolName || ""));
  if (msg.type === "message_update" && msg.assistantMessageEvent?.type === "text_delta") {
    assistantChunks.push(String(msg.assistantMessageEvent.delta || ""));
  }
  const message = msg.message || msg;
  const err =
    (msg.message && (msg.message.errorMessage || msg.message.error)) ||
    msg.errorMessage ||
    msg.error;
  if (err && (msg.type === "message_end" || msg.type === "auto_retry_end" || msg.type === "agent_end")) {
    apiErrors.push(String(err));
  }
  if (msg.type === "message_end" && msg.message) {
    const role = msg.message.role;
    const content = msg.message.content;
    if (role === "assistant") {
      let text = "";
      if (typeof content === "string") text = content;
      else if (Array.isArray(content)) {
        text = content
          .map((c) => (typeof c?.text === "string" ? c.text : ""))
          .join("");
      }
      if (text) assistantChunks.push(text);
    }
  }
}

child.stdout.setEncoding("utf8");
child.stdout.on("data", (chunk) => {
  buf += chunk;
  let idx;
  while ((idx = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, idx);
    buf = buf.slice(idx + 1);
    onLine(line.endsWith("\r") ? line.slice(0, -1) : line);
  }
});

const started = Date.now();
try {
  const state = await send({ type: "get_state" });
  if (!state.success) fail("get_state failed", state);
  console.log("model", state.data?.model?.provider, state.data?.model?.id);

  const cmds = await send({ type: "get_commands" }).catch(() => null);
  const names = JSON.stringify(cmds?.data || cmds || {});
  if (!/recap/i.test(names)) {
    console.warn("warning: get_commands did not list recap (continuing)");
  } else {
    console.log("commands include recap");
  }

  settled = false;
  const ack = await send({
    type: "prompt",
    message: "fais-moi un récap du dossier de fred",
  });
  if (!ack.success) fail("prompt rejected", ack);
  console.log("prompt accepted");

  const deadline = Date.now() + timeoutMs;
  while (!settled && Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 250));
    if (child.exitCode != null) {
      fail(`pi exited ${child.exitCode}`, fs.readFileSync(stderrLog, "utf8").slice(-2000));
    }
  }
  if (!settled) fail(`timed out after ${timeoutMs}ms waiting for agent_settled`);
  if (apiErrors.length && assistantChunks.join("").trim().length === 0) {
    fail(`provider error (empty recap): ${apiErrors[0]}`);
  }
} catch (e) {
  fail(String(e), fs.existsSync(stderrLog) ? fs.readFileSync(stderrLog, "utf8").slice(-2000) : "");
}

const recapOnDisk = path.join(fred, "RECAP.md");
const disk = fs.existsSync(recapOnDisk) ? fs.readFileSync(recapOnDisk, "utf8") : "";
const chat = assistantChunks.join("");
const blob = `${disk}\n${chat}`;
const norm = blob.replace(/'/g, "’");

const sections = ["Vue d’ensemble", "Contenu", "État", "Manques", "Actions"];
const missing = sections.filter((s) => !norm.includes(s));
const mentionsTodo = /argparse/i.test(blob) || /tests/i.test(blob);
const mentionsRecap = /RECAP\.md/i.test(blob) || disk.length > 0;
const wrote = disk.length > 0;
const uiConfirm = uiRequests.some((u) => u.method === "confirm");

const report = [
  "# Golden path — récap du dossier de Fred",
  "",
  `Elapsed: ${((Date.now() - started) / 1000).toFixed(1)}s`,
  `Workspace: ${ws}`,
  `Tools: ${toolNames.join(", ") || "(none)"}`,
  `UI requests: ${uiRequests.map((u) => u.method).join(", ") || "(none)"}`,
  `RECAP.md written: ${wrote}`,
  `Confirm modal: ${uiConfirm}`,
  "",
  wrote ? disk : chat,
  "",
].join("\n");
fs.writeFileSync(outFile, report);
console.log(report);
console.log("wrote", outFile);

const errors = [];
if (missing.length) errors.push(`missing sections: ${missing.join(", ")}`);
if (!mentionsTodo) errors.push("recap does not mention open TODOs (argparse / tests)");
if (!mentionsRecap) errors.push("did not write or propose RECAP.md");
if (errors.length) {
  fail(errors.join("\n"), `stderr tail:\n${fs.readFileSync(stderrLog, "utf8").slice(-1500)}`);
}

console.log("GOLDEN PATH PASS");
cleanup();
setTimeout(() => process.exit(0), 400);