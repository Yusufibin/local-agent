import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export function validateSuite(suite) {
  if (suite.version !== 1 || !suite.files || !Array.isArray(suite.tasks) || suite.tasks.length < 20) throw new Error("Expected a version 1 suite with at least 20 tasks");
  const ids = new Set();
  for (const name of Object.keys(suite.files)) {
    if (path.isAbsolute(name) || name.split(/[\\/]/).includes("..")) throw new Error("Fixture path must be relative");
  }
  for (const task of suite.tasks) {
    if (!task.id || ids.has(task.id) || !task.prompt || !task.checks?.length) throw new Error("Invalid task");
    ids.add(task.id);
    if (task.mode && !["readonly", "ask"].includes(task.mode)) throw new Error("Invalid mode");
    for (const check of task.checks) {
      if (!["answer", "python", "exists", "unchanged"].includes(check.kind)) throw new Error("Unknown check");
      if (check.path && (path.isAbsolute(check.path) || check.path.split(/[\\/]/).includes(".."))) throw new Error("Invalid check path");
      if (check.kind === "answer" && !check.all?.length && !check.any?.length && !check.none?.length) throw new Error("Empty answer check");
      if (check.kind === "python" && !check.code) throw new Error("Missing Python assertion");
    }
  }
}

export function checkAnswer(check, answer) {
  const text = answer.toLowerCase();
  return (check.all ?? []).every((value) => text.includes(value.toLowerCase())) &&
    (!check.any?.length || check.any.some((value) => text.includes(value.toLowerCase()))) &&
    (check.none ?? []).every((value) => !text.includes(value.toLowerCase()));
}

export function scoreTask(task, answer, workspace, originals) {
  return task.checks.map((check) => {
    let passed = false, detail = "";
    if (check.kind === "answer") passed = checkAnswer(check, answer);
    else if (check.kind === "exists") passed = fs.existsSync(path.join(workspace, check.path));
    else if (check.kind === "unchanged") passed = fs.readFileSync(path.join(workspace, check.path), "utf8") === originals[check.path];
    else if (check.kind === "python") {
      const result = spawnSync(process.env.PYTHON_BIN || "python3", ["-c", check.code], {
        cwd: workspace, encoding: "utf8", timeout: 10_000,
        env: { PATH: process.env.PATH, PYTHONPATH: workspace },
        maxBuffer: 128_000,
      });
      passed = result.status === 0 && !result.error;
      detail = String(result.error ?? result.stderr ?? "").slice(-2000);
    }
    return { kind: check.kind, passed, detail };
  });
}
