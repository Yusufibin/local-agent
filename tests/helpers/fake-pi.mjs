#!/usr/bin/env node
/**
 * Fake Pi RPC sidecar for host tests. Speaks JSONL on stdin/stdout.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { StringDecoder } from "node:string_decoder";

const scenario = process.env.FAKE_PI_SCENARIO || "default";
const stateFile = process.env.DESKPI_FAKE_STATE;
const uiLog = process.env.DESKPI_FAKE_UI_LOG;
const goldenDir =
  process.env.FAKE_PI_GOLDEN_DIR ||
  path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "rpc_golden");

if (stateFile) {
  fs.writeFileSync(
    stateFile,
    JSON.stringify(
      {
        pid: process.pid,
        cwd: process.cwd(),
        argv: process.argv.slice(1),
        env: {
          DESKPI_WORKSPACE_FILE: process.env.DESKPI_WORKSPACE_FILE ?? null,
          DESKPI_PERMISSION_MODE: process.env.DESKPI_PERMISSION_MODE ?? null,
          TERM: process.env.TERM ?? null,
          NO_COLOR: process.env.NO_COLOR ?? null,
          HOME: process.env.HOME ?? null,
        },
        envKeys: Object.keys(process.env).sort(),
      },
      null,
      2,
    ),
  );
}

function send(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function emitGolden(name) {
  const p = path.join(goldenDir, name);
  if (!fs.existsSync(p)) {
    send({ type: "agent_start" });
    send({ type: "agent_end", messages: [], willRetry: false });
    send({ type: "agent_settled" });
    return;
  }
  const lines = fs.readFileSync(p, "utf8").split("\n").filter(Boolean);
  for (const line of lines) process.stdout.write(line + "\n");
}

function handle(line) {
  let cmd;
  try {
    cmd = JSON.parse(line);
  } catch {
    return;
  }
  const id = cmd.id;
  const type = cmd.type;
  if (uiLog && type === "extension_ui_response") {
    fs.appendFileSync(uiLog, line + "\n");
    return;
  }
  if (type === "get_state") {
    send({
      id,
      type: "response",
      command: "get_state",
      success: true,
      data: {
        model: { id: "fake-model", name: "Fake", provider: "fake" },
        thinkingLevel: "off",
        isStreaming: false,
      },
    });
    return;
  }
  if (type === "prompt") {
    send({ id, type: "response", command: "prompt", success: true });
    if (scenario === "tool") emitGolden("tool_call.jsonl");
    else if (scenario === "prompt") emitGolden("prompt.jsonl");
    else if (scenario === "slow-settled") {
      send({ type: "agent_start" });
      setTimeout(() => {
        send({ type: "agent_end", messages: [], willRetry: false });
        send({ type: "agent_settled" });
      }, 400);
    } else {
      send({ type: "agent_start" });
      send({ type: "agent_end", messages: [], willRetry: false });
      send({ type: "agent_settled" });
    }
    return;
  }
  if (type === "get_available_models") {
    if (process.env.FAKE_PI_HANG === "1") return;
    send({
      id,
      type: "response",
      command: "get_available_models",
      success: true,
      data: { models: [{ id: "fake-model", provider: "fake" }] },
    });
    return;
  }
  if (type === "clear_queue") {
    send({
      id,
      type: "response",
      command: "clear_queue",
      success: true,
      data: { steering: ["queued steer"], followUp: ["queued follow"] },
    });
    return;
  }
  if (type === "abort") {
    send({ id, type: "response", command: "abort", success: true });
    send({ type: "agent_settled" });
    return;
  }
  send({ id, type: "response", command: type, success: true, data: {} });
}

let buffer = "";
const decoder = new StringDecoder("utf8");
process.stdin.on("data", (chunk) => {
  buffer += typeof chunk === "string" ? chunk : decoder.write(chunk);
  for (;;) {
    const i = buffer.indexOf("\n");
    if (i < 0) break;
    let line = buffer.slice(0, i);
    buffer = buffer.slice(i + 1);
    if (line.endsWith("\r")) line = line.slice(0, -1);
    handle(line);
  }
});

if (scenario === "ui_select") {
  setTimeout(() => {
    send({
      type: "extension_ui_request",
      id: "ui-select-1",
      method: "select",
      title: "Allow?",
      options: ["Allow", "Block"],
    });
    send({ type: "agent_start" });
  }, 30);
}
if (scenario === "ui_confirm") {
  setTimeout(() => {
    send({
      type: "extension_ui_request",
      id: "ui-confirm-1",
      method: "confirm",
      title: "Write file?",
      message: "ok.md",
      timeout: 80,
    });
  }, 30);
}
if (scenario === "ui_both") {
  setTimeout(() => {
    emitGolden("extension_ui_request.jsonl");
  }, 30);
}

setInterval(() => {}, 1 << 30);