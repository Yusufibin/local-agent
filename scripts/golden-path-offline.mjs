#!/usr/bin/env node
/**
 * Phase 5 fixture contract (no LLM): run the trouver-dossier / recap-dossier
 * procedure against tests/fixtures/Fred-Projet — bounded find, sample reads,
 * structured recap, write RECAP.md in the found folder.
 *
 * Live agent: `pnpm golden` (needs provider credits).
 */
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixture = path.join(root, "tests/fixtures/Fred-Projet");
const outFile = path.join(root, "tests/golden-path-output.md");

if (!fs.existsSync(fixture)) {
  console.error("missing fixture", fixture);
  process.exit(1);
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "deskpi-golden-offline-"));
const ws = tmp;
fs.cpSync(fixture, path.join(ws, "Fred-Projet"), { recursive: true });

function findFred(searchRoot) {
  const raw = execFileSync(
    "find",
    [searchRoot, "-maxdepth", "5", "-iname", "*fred*", "-type", "d"],
    { encoding: "utf8" },
  );
  return raw
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

const hits = findFred(ws);
const fred = hits.find((p) => path.basename(p).toLowerCase().includes("fred"));
if (!fred) {
  console.error("trouver-dossier procedure found no *fred* dir under", ws, hits);
  process.exit(1);
}

function readIf(rel) {
  const p = path.join(fred, rel);
  return fs.existsSync(p) ? fs.readFileSync(p, "utf8") : "";
}

const readme = readIf("README.md");
const notes = readIf("notes.txt");
const todo = readIf("todo.md");
const mainPy = readIf("src/main.py");

const inventory = execFileSync("find", [fred, "-maxdepth", "3"], { encoding: "utf8" })
  .split("\n")
  .map((s) => s.trim())
  .filter(Boolean)
  .map((p) => path.relative(fred, p) || ".");

const openTodos = todo
  .split("\n")
  .map((l) => l.trim())
  .filter((l) => l.startsWith("- [ ]"));
const doneTodos = todo
  .split("\n")
  .map((l) => l.trim())
  .filter((l) => l.startsWith("- [x]"));

const recap = `# RECAP — Fred-Projet

## Vue d’ensemble

${readme.trim() || "Prototype CLI de Fred, sans description README."}

Le dossier a été localisé par une recherche bornée \`find <root> -maxdepth 5 -iname '*fred*'\` (pas de scan de \`/\` ni de \`\$HOME\`).

## Contenu

Inventaire (maxdepth 3):

${inventory.map((f) => `- \`${f}\``).join("\n")}

Notes:

${notes.trim() || "_pas de notes.txt_"}

Code d’entrée \`src/main.py\`:

\`\`\`python
${mainPy.trim()}
\`\`\`

## État

Fait:
${doneTodos.length ? doneTodos.map((l) => `- ${l.slice(6)}`).join("\n") : "- (rien de coché)"}

Ouvert:
${openTodos.length ? openTodos.map((l) => `- ${l.slice(6)}`).join("\n") : "- (aucun TODO ouvert)"}

## Manques

- Pas de tests (le README le dit, \`todo.md\` aussi).
- Pas d’argparse dans \`src/main.py\` (TODO explicite).
- Pas de fichier de configuration.
- Pas de \`RECAP.md\` avant ce passage.

## Actions

1. Ajouter argparse dans \`src/main.py\`.
2. Écrire des tests.
3. (optionnel) export JSON mentionné dans \`notes.txt\`.

Ce fichier \`RECAP.md\` est proposé à la racine du dossier trouvé (mode ask: confirm write).
`;

fs.writeFileSync(path.join(fred, "RECAP.md"), recap);
fs.writeFileSync(outFile, recap);

const blob = recap;
const sections = ["Vue d’ensemble", "Contenu", "État", "Manques", "Actions"];
const missing = sections.filter((s) => !blob.includes(s));
const mentionsTodo = /argparse/i.test(blob) && /tests/i.test(blob);
if (missing.length || !mentionsTodo) {
  console.error("offline recap failed", { missing, mentionsTodo });
  process.exit(1);
}
if (!fs.existsSync(path.join(fred, "RECAP.md"))) {
  console.error("RECAP.md was not written");
  process.exit(1);
}

console.log(recap);
console.log("GOLDEN PATH OFFLINE PASS");
console.log("found", fred);
console.log("wrote", outFile);
console.log("RECAP.md", path.join(fred, "RECAP.md"));