# Local Agent

Application desktop pour parler à un agent de code sur ton propre dossier de projet.

L’interface est un chat léger. Sous le capot, elle lance [Pi](https://pi.dev) en local : le modèle lit, écrit et exécute dans un workspace que tu choisis, avec ton accord pour les actions sensibles.

## Ce que ça fait

- Choisir un dossier de travail
- Discuter avec l’agent dans une fenêtre chat-first (thème clair)
- Approuver ou refuser les écritures et commandes dangereuses
- Garder les clés API hors du dépôt (stockage local chiffré côté app)

Pi tourne en sidecar ; Local Agent s’occupe de la fenêtre, des permissions et du redémarrage propre du process.

## Lancer en développement

Prérequis : Node 22, pnpm, Rust (Tauri 2).

```bash
pnpm install
bash scripts/vendor-runtime.sh   # Node + Pi embarqués (ignorés par git)
pnpm tauri dev
```

Sans le vendor, `pnpm tauri dev` peut utiliser un `pi` déjà présent dans ton `PATH`.

## Builder l’AppImage (Linux)

```bash
pnpm package:appimage
```

Le binaire sort sous `src-tauri/target/release/bundle/appimage/`.

## Stack

Tauri 2 · Svelte 5 · Rust (multiplexeur JSONL) · [Pi coding agent](https://pi.dev)

## Licence

Voir le dépôt pour les conditions d’usage.
