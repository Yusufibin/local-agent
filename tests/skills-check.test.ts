import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

describe("skills overlay fixture", () => {
  it("trouver-dossier has bounded search procedure", () => {
    const text = fs.readFileSync(
      path.join(root, "agent-runtime/skills/trouver-dossier/SKILL.md"),
      "utf8",
    );
    expect(text).toMatch(/maxdepth 5/);
    expect(text).toMatch(/workspace roots/);
    expect(text).toMatch(/Ne jamais scanner \/ ou \$HOME/);
  });

  it("recap-dossier has required section titles", () => {
    const text = fs.readFileSync(
      path.join(root, "agent-runtime/skills/recap-dossier/SKILL.md"),
      "utf8",
    );
    for (const title of ["Vue d’ensemble", "Contenu", "État", "Manques", "Actions"]) {
      expect(text).toContain(title);
    }
    expect(text).toMatch(/find -maxdepth 3/);
  });

  it("SYSTEM overlay states roots, no full-tree dump, no .env", () => {
    const text = fs.readFileSync(path.join(root, "agent-runtime/SYSTEM.md"), "utf8");
    expect(text.toLowerCase()).toMatch(/roots/);
    expect(text).toMatch(/Never dump a whole tree/i);
    expect(text).toMatch(/\.env/);
  });

  it("Fred-Projet fixture has the four files", () => {
    const base = path.join(root, "tests/fixtures/Fred-Projet");
    for (const f of ["README.md", "notes.txt", "todo.md", "src/main.py"]) {
      expect(fs.existsSync(path.join(base, f))).toBe(true);
    }
  });

  it("Fred-Projet open TODOs are argparse and tests", () => {
    const todo = fs.readFileSync(path.join(root, "tests/fixtures/Fred-Projet/todo.md"), "utf8");
    expect(todo).toMatch(/argparse/);
    expect(todo).toMatch(/Écrire des tests|tests/);
    expect(todo).toMatch(/- \[ \]/);
    const py = fs.readFileSync(path.join(root, "tests/fixtures/Fred-Projet/src/main.py"), "utf8");
    expect(py).toMatch(/TODO: argparse/);
  });

  it("/recap prompt template has frontmatter and section titles", () => {
    const text = fs.readFileSync(path.join(root, "agent-runtime/prompts/recap.md"), "utf8");
    expect(text).toMatch(/^---/m);
    expect(text).toMatch(/description:/);
    for (const title of ["Vue d’ensemble", "Contenu", "État", "Manques", "Actions"]) {
      expect(text).toContain(title);
    }
    expect(text).toMatch(/RECAP\.md/);
  });
});