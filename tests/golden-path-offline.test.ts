import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

describe("Phase 5 golden path fixture procedure", () => {
  it("bounded find locates Fred-Projet then recap mentions open TODOs and writes RECAP.md", () => {
    const script = path.join(repo, "scripts/golden-path-offline.mjs");
    const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), "deskpi-vitest-golden-"));
    const out = execFileSync(process.execPath, [script], {
      encoding: "utf8",
      env: { ...process.env, HOME: tmpHome },
    });
    expect(out).toMatch(/GOLDEN PATH OFFLINE PASS/);
    expect(out).toContain("Vue d’ensemble");
    expect(out).toContain("Contenu");
    expect(out).toContain("État");
    expect(out).toContain("Manques");
    expect(out).toContain("Actions");
    expect(out).toMatch(/argparse/i);
    expect(out).toMatch(/tests/i);
    const saved = fs.readFileSync(path.join(repo, "tests/golden-path-output.md"), "utf8");
    expect(saved).toContain("## Vue d’ensemble");
    expect(saved).toMatch(/RECAP\.md/);
  });
});