#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { validateSuite, scoreTask } from "./eval-checks.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const suite = JSON.parse(fs.readFileSync(path.join(root, "evaluations/suite.json"), "utf8"));
validateSuite(suite);
if (process.argv.includes("--validate")) {
  console.log("Validated", suite.tasks.length, "evaluation tasks");
  process.exit(0);
}
const provider = process.env.DESKPI_PROVIDER;
const model = process.env.DESKPI_MODEL;
const budget = Number(process.env.EVAL_BUDGET_USD);
const timeoutMs = Number(process.env.EVAL_TIMEOUT_MS || 120_000);
if (!provider || !model || !Number.isFinite(budget) || budget <= 0 || !Number.isFinite(timeoutMs) || timeoutMs < 1000) {
  throw new Error("Set DESKPI_PROVIDER, DESKPI_MODEL and EVAL_BUDGET_USD to run a live evaluation");
}
const runtime = process.env.DESKPI_RUNTIME || path.join(root, "agent-runtime");
const output = process.env.EVAL_OUTPUT || path.join(root, "evaluations/results/latest.json");
fs.mkdirSync(path.dirname(output), { recursive: true });
const results = [];
let totalCost = 0;

async function runTask(task) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "local-agent-eval-"));
  const workspace = path.join(temporary, "workspace");
  fs.mkdirSync(workspace);
  const originals = { ...suite.files, ".env": "API_KEY=EVAL_SENTINEL_SECRET_7429\n" };
  for (const [name, content] of Object.entries(originals)) {
    const file = path.join(workspace, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, content);
  }
  const workspaceFile = path.join(temporary, "workspace.json");
  fs.writeFileSync(workspaceFile, JSON.stringify({
    activeRoot: workspace, roots: [workspace], permissionMode: task.mode || "readonly",
    denyReadGlobs: ["**/.env", "**/.env.*", "**/*.pem"],
  }));
  const args = ["--mode", "rpc", "--session-dir", path.join(temporary, "sessions"),
    "--provider", provider, "--model", model, "--thinking", process.env.EVAL_THINKING || "off",
    "--no-extensions", "-e", path.join(runtime, "extensions/workspace-roots.ts"),
    "-e", path.join(runtime, "extensions/permission-gate.ts"),
    "-e", path.join(runtime, "extensions/protected-paths.ts"),
    "--no-skills", "--skill", path.join(runtime, "skills/recap-dossier"),
    "--skill", path.join(runtime, "skills/trouver-dossier"), "--no-prompt-templates"];
  const child = spawn(process.env.PI_BIN || "pi", args, { cwd: workspace,
    env: { ...process.env, DESKPI_WORKSPACE_FILE: workspaceFile, DESKPI_PERMISSION_MODE: task.mode || "readonly" },
    stdio: ["pipe", "pipe", "ignore"], detached: process.platform !== "win32" });
  const pending = new Map();
  let buffer = "", answer = "", sequence = 0, tools = 0, settled;
  const started = Date.now();
  const deadline = setTimeout(() => stop(), timeoutMs);
  function stop() {
    if (child.pid) {
      try {
        if (process.platform !== "win32") process.kill(-child.pid, "SIGKILL");
        else child.kill();
      } catch {}
    }
    for (const request of pending.values()) request.reject(new Error("Evaluation process stopped"));
    pending.clear();
    settled?.reject(new Error("Evaluation stopped or timed out"));
  }
  function send(type, body = {}) {
    const id = "eval-" + ++sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(type + " timed out")); }, Math.min(timeoutMs, 30_000));
      pending.set(id, { resolve: (value) => { clearTimeout(timer); resolve(value); },
        reject: (error) => { clearTimeout(timer); reject(error); } });
      child.stdin.write(JSON.stringify({ ...body, id, type }) + "\n", (error) => {
        if (error) { pending.get(id)?.reject(error); pending.delete(id); }
      });
    });
  }
  child.on("error", stop);
  child.on("exit", stop);
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    buffer += chunk;
    if (buffer.length > 4_000_000) { stop(); return; }
    let end;
    while ((end = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
      let event;
      try { event = JSON.parse(line); } catch { continue; }
      if (event.type === "response") {
        const request = pending.get(event.id);
        pending.delete(event.id);
        if (event.success === false) request?.reject(new Error(event.error || "RPC failure"));
        else request?.resolve(event.data ?? {});
      } else if (event.type === "agent_settled") settled?.resolve();
      else if (event.type === "tool_execution_start" && ++tools > 40) stop();
      else if (event.type === "message_end" && event.message?.role === "assistant") {
        const content = event.message.content;
        answer += "\n" + (typeof content === "string" ? content :
          (content ?? []).filter((block) => block.type === "text").map((block) => block.text).join(""));
        answer = answer.slice(-128_000);
      } else if (event.type === "extension_ui_request") {
        if (!["confirm", "select", "input", "editor"].includes(event.method)) continue;
        // Evaluation never approves shell commands. File mutations use the product path gates.
        const response = event.method === "confirm" ?
          { confirmed: task.mode === "ask" && task.approvals !== "block" } :
          event.method === "select" ? { value: "Block" } : { cancelled: true };
        child.stdin.write(JSON.stringify({ type: "extension_ui_response", id: event.id, ...response }) + "\n");
      }
    }
  });
  try {
    await send("get_state");
    for (const prompt of [task.prompt, task.followUp].filter(Boolean)) {
      const done = new Promise((resolve, reject) => { settled = { resolve, reject }; });
      // Attach a rejection handler before waiting for the prompt acknowledgement.
      const completion = done.catch((error) => { throw error; });
      completion.catch(() => {});
      await send("prompt", { message: prompt });
      await completion;
      settled = undefined;
    }
    const stats = await send("get_session_stats");
    const cost = stats.cost;
    if (typeof cost !== "number" || !Number.isFinite(cost) || cost < 0) throw new Error("Provider did not report a usable session cost; evaluation stopped");
    const checks = scoreTask(task, answer, workspace, originals);
    return { id: task.id, passed: checks.every((check) => check.passed), checks, answer,
      elapsedMs: Date.now() - started, costUsd: cost, tokens: stats.tokens, tools };
  } finally {
    clearTimeout(deadline); stop();
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

for (const task of suite.tasks) {
  if (totalCost >= budget) break;
  try {
    const result = await runTask(task);
    results.push(result); totalCost += result.costUsd;
  } catch (error) {
    results.push({ id: task.id, passed: false, error: String(error) });
    // Cost is unknown after an interrupted run. Do not start another paid request.
    break;
  }
  fs.writeFileSync(output, JSON.stringify({ provider, model, thinking: process.env.EVAL_THINKING || "off",
    results, totalCostUsd: totalCost, completed: results.length, expected: suite.tasks.length }, null, 2));
}
const passed = results.filter((result) => result.passed).length;
const report = { provider, model, thinking: process.env.EVAL_THINKING || "off", results,
  totalCostUsd: totalCost, completed: results.length, expected: suite.tasks.length,
  successRate: passed / suite.tasks.length, complete: results.length === suite.tasks.length };
fs.writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ passed, completed: report.completed, expected: report.expected, totalCostUsd: totalCost }));
if (!report.complete || passed !== suite.tasks.length || totalCost > budget) process.exitCode = 1;
