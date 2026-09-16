# Plan d’intégration — agent local Pi + GUI Tauri

Document d’exécution. Pas un brainstorm. Les décisions ci-dessous sont verrouillées sauf contradiction technique découverte en implémentation.

**Stack :** Tauri 2 · Svelte 5 + TypeScript · Vite · Pi Coding Agent en sidecar RPC  
**Objectif produit :** un agent de type coding agent, collé à ta machine, avec **ta** fenêtre. Exemple : « fais-moi un récap du dossier de Fred » → chercher dans des roots autorisés, lire, analyser, écrire ce qui manque.

Pi n’est pas Hermes. Pi est le moteur (read / write / edit / bash). La GUI et la politique de sécurité sont le produit.

---

## 1. Décisions verrouillées

| # | Décision | Pourquoi |
|---|---|---|
| D1 | **Tauri 2**, pas Electron | Binaire léger, process Rust, capabilities, pas de Chromium embarqué entier |
| D2 | **Pi en sidecar RPC** (`pi --mode rpc`), pas le SDK in-process | Le host est Rust. Pi documente RPC pour l’intégration hors Node **et** l’isolation process. Un crash Node ne tue pas la fenêtre |
| D3 | **Rust = multiplexeur mince**, zéro logique agent | Pas de second cerveau. Rust parse le JSONL, route commandes / events / dialogs UI |
| D4 | **Frontend = Svelte 5 + TS + Vite** | Compile away, bundle petit, streaming UI simple. React est interchangeable au niveau UI seulement |
| D5 | **Jamais le disque entier comme cwd** | Workspace roots explicites. « Trouver Fred » = search bornée, pas `find /` |
| D6 | **Write / edit / bash derrière approval GUI** (défaut) | Pi est YOLO. Le produit ne l’est pas |
| D7 | **Extensions via `-e`**, pas via `~/.pi` du user | Isolé de l’install CLI Pi de l’utilisateur. RPC ne montre pas le prompt de project trust |
| D8 | **V1 : Node + `pi` sur le PATH** | Bundler Node dans le sidecar = phase packaging. Dev et premier usage restent légers |
| D9 | **Changement de workspace = restart du sidecar** | Le RPC Pi n’expose pas `set_cwd`. Le cwd est celui du process |
| D10 | **La webview n’a pas les API keys, pas d’accès FS, pas d’accès réseau providers** | Les appels LLM se font dans le sidecar. Le frontend ne voit que des events |

Non-négociable pour v1 :

- JSONL strict : split sur `\n` uniquement (jamais `readline` / split Unicode).
- Le reader stdout **ne bloque jamais** (dialogs UI asynchrones).
- Chaque commande RPC a un `id` ; les responses sont corrélées par `id`.
- `agent_settled` = fin réelle d’un run (pas `agent_end`, qui peut précéder retry / compaction / follow-up).

---

## 2. Ce que v1 fait / ne fait pas

### Fait (v1)

- Fenêtre desktop unique, chat streaming.
- Un workspace root actif + N roots secondaires en lecture (voir §7).
- Prompt, abort, steer, follow-up.
- Cartes d’outils (bash, read, write, edit, grep, find, ls) en live.
- Approval write / edit / bash (et read hors root).
- Sessions persistées (JSONL Pi), liste, new, resume.
- Sélecteur de modèle / thinking level.
- Coût + contexte (footer).
- Skill `recap-dossier` + skill `trouver-dossier`.
- Settings : provider keys dans le keychain OS, roots, mode permission.

### Ne fait pas (v1)

- Scan du PC entier.
- Sub-agents, plan mode, MCP.
- Mémoire type Hermes / learning loop.
- Sandbox OS (Docker / Gondolin) — documenté, pas livré.
- Bundling Node dans l’installeur.
- Multi-fenêtres, systray always-on, cron.
- Éditeur de code embarqué (les diffs s’affichent dans le chat).

---

## 3. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Webview  (Svelte 5)                                        │
│  Chat · tool cards · approval modals · settings · sessions  │
│  invoke()  ─────────────►  listen / Channel                 │
└──────────────┬──────────────────────────────▲───────────────┘
               │ Tauri IPC                    │ events / Channel
               ▼                              │
┌─────────────────────────────────────────────────────────────┐
│  Host Rust  (src-tauri)                                     │
│  SidecarManager · JsonlCodec · RpcClient · UiBridge         │
│  capabilities : shell.sidecar, dialog, notification,        │
│                 path app-data only                          │
└──────────────┬──────────────────────────────▲───────────────┘
               │ stdin JSONL                  │ stdout JSONL
               ▼                              │
┌─────────────────────────────────────────────────────────────┐
│  Sidecar : pi --mode rpc                                    │
│  cwd = workspace actif                                      │
│  -e workspace-roots.ts                                      │
│  -e permission-gate.ts                                      │
│  -e protected-paths.ts                                      │
│  --session-dir {app_data}/sessions                          │
│  env : API keys (injectées au spawn, jamais dans la webview)│
└─────────────────────────────────────────────────────────────┘
```

Trois flux, pas un :

| Flux | Sens | Mécanisme Tauri | Exemples |
|---|---|---|---|
| Commande | UI → Rust → Pi stdin | `invoke` + oneshot | `prompt`, `abort`, `set_model` |
| Event | Pi stdout → Rust → UI | `Channel` (streaming) + event `agent://event` | `text_delta`, `tool_execution_*` |
| Dialog extension | Pi → UI → Pi | event `agent://ui_request` + `invoke("ui_respond")` | confirm write, select allow/block |

Ne pas fusionner ces trois flux dans un seul WebSocket maison. Tauri IPC existe déjà.

---

## 4. Pourquoi RPC et pas le SDK

Pi dit explicitement :

- **SDK** si tu es dans le même process Node, tu veux les types, tu customises tools/extensions en code.
- **RPC** si tu intègres depuis une autre langue, tu veux l’isolation process, tu construis un client language-agnostic.

On est le cas RPC. Conséquences :

1. Le host Rust parle le protocole de [rpc.md](https://pi.dev/docs/latest/rpc), pas l’API `createAgentSession`.
2. Les extensions restent du TypeScript **chargé par Pi**, pas par Rust.
3. `ctx.ui.*` en RPC devient `extension_ui_request` / `extension_ui_response`. C’est **le** pont approval GUI. On ne réimplémente pas un permission system dans Rust.
4. On peut tuer / relancer Pi sans relancer l’app (workspace change, hang, crash).

Le SDK reste une option **future** uniquement si on abandonne Tauri pour un host Node (Electron, ou sidecar Node qui wrap le SDK et expose un proto plus simple). Pas v1.

---

## 5. Layout du repo

Monorepo plat, un seul package app. Pas de cargo workspace tant qu’on n’a pas un deuxième crate utile.

```
agent/
├── plan.md                          ← ce fichier
├── README.md
├── package.json                     ← scripts : tauri dev / build
├── pnpm-workspace.yaml              ← optionnel, un seul package suffit
├── src/                             ← frontend Svelte
│   ├── app.css
│   ├── main.ts
│   ├── App.svelte
│   ├── lib/
│   │   ├── api.ts                   ← invoke wrappers typés
│   │   ├── events.ts                ← listen agent://*
│   │   ├── store/
│   │   │   ├── session.svelte.ts    ← run state, messages, queue
│   │   │   ├── settings.svelte.ts
│   │   │   └── workspace.svelte.ts
│   │   ├── protocol/
│   │   │   ├── events.ts            ← types mirror du JSONL Pi
│   │   │   ├── commands.ts
│   │   │   └── ui-requests.ts
│   │   └── components/
│   │       ├── Chat.svelte
│   │       ├── Composer.svelte
│   │       ├── Message.svelte
│   │       ├── ToolCard.svelte
│   │       ├── ApprovalModal.svelte
│   │       ├── SessionList.svelte
│   │       ├── ModelPicker.svelte
│   │       ├── StatusBar.svelte
│   │       └── Settings.svelte
│   └── assets/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── binaries/                    ← vide en v1 (sidecar bundlé = phase 5)
│   └── src/
│       ├── lib.rs
│       ├── main.rs
│       ├── error.rs
│       ├── commands.rs              ← #[tauri::command]
│       ├── sidecar.rs               ← spawn / kill / restart
│       ├── jsonl.rs                 ← framer stdout
│       ├── rpc.rs                   ← id, pending map, send
│       ├── bridge.rs                ← events + extension UI
│       └── config.rs                ← app_data, spawn args, env
├── agent-runtime/                   ← artefacts chargés par Pi, pas par la GUI
│   ├── extensions/
│   │   ├── workspace-roots.ts
│   │   ├── permission-gate.ts
│   │   └── protected-paths.ts
│   ├── skills/
│   │   ├── recap-dossier/SKILL.md
│   │   └── trouver-dossier/SKILL.md
│   ├── prompts/
│   │   └── recap.md
│   └── SYSTEM.md                    ← overlay prompt produit
└── tests/
    ├── jsonl_framing.rs             ← (ou tests dans src-tauri)
    ├── rpc_golden/
    └── e2e/                         ← plus tard
```

`agent-runtime/` est copié dans les resources Tauri (`bundle.resources`) pour que le sidecar reçoive des chemins `-e` / `--skill` stables en prod.

Frontend **ne** lit pas ces fichiers. Seul Pi les charge.

---

## 6. Couche protocole (contrat host)

### 6.1 Framing

- 1 objet JSON par ligne, délimiteur **LF uniquement**.
- Accepter `\r\n` en stripant le `\r` final.
- Buffer bytes → UTF-8 incremental (`StringDecoder` côté Node ; côté Rust : accumuler, split sur `b'\n'`).
- Une ligne trop longue (ex. > 16 MiB) : log + drop + restart sidecar. Ne pas OOM la GUI.
- stderr Pi : logger fichier `{app_data}/logs/pi.stderr.log`, ne pas parser comme JSONL.

### 6.2 Identifiants

Toute commande stdin :

```json
{"id":"ui-7f3a…","type":"prompt","message":"…"}
```

`id` = UUID v4 généré par Rust. La response stdout reprend le même `id`.

Map Rust :

```text
pending: HashMap<String, oneshot::Sender<RpcResponse>>
```

Timeout commande : 30s pour les commandes **hors** `prompt` / `compact` / `bash`.  
`prompt` n’attend que l’ack (`success: true` = accepté/queue). Le run lui-même se termine sur `agent_settled`.

### 6.3 Commandes host exposées à la GUI

Surface **volontairement petite**. La GUI n’envoie pas de JSON Pi brut. Rust traduit.

| Commande Tauri | RPC Pi | Notes |
|---|---|---|
| `agent_start` | spawn process | args + env + cwd |
| `agent_stop` | kill process group | SIGTERM puis SIGKILL après 2s |
| `agent_restart` | stop + start | workspace change, hang |
| `prompt { text, images?, behavior? }` | `prompt` | `behavior` = `steer` \| `followUp` si streaming |
| `steer { text }` | `steer` | |
| `follow_up { text }` | `follow_up` | |
| `abort` | `clear_queue` puis `abort` | comportement Esc Pi : vider la queue, remettre le texte dans le composer |
| `new_session` | `new_session` | |
| `switch_session { path }` | `switch_session` | |
| `list_sessions` | `SessionManager` n’est pas RPC — **lister le dossier session-dir en Rust** | |
| `get_state` | `get_state` | |
| `get_messages` | `get_messages` | hydrate UI au resume |
| `set_model { provider, modelId }` | `set_model` | |
| `set_thinking_level { level }` | `set_thinking_level` | |
| `get_available_models` | `get_available_models` | |
| `get_session_stats` | `get_session_stats` | footer |
| `compact` | `compact` | |
| `get_commands` | `get_commands` | palette `/` |
| `ui_respond { id, payload }` | `extension_ui_response` | |
| `pick_workspace` | dialog Tauri, puis `agent_restart` | pas une cmd Pi |
| `save_secret { provider, key }` | keychain, puis restart pour injecter l’env | |

Interdit côté GUI : envoyer `bash` RPC direct (le bash utilisateur). Le shell passe par l’agent (`prompt`) ou plus tard un composer `!cmd` si on l’ajoute.

### 6.4 Events poussés vers la GUI

Channel unique `agent-events` (Tauri Channel, ordonné) **ou** event `agent://event` si Channel est trop contraignant au boot. Préférer Channel pour `text_delta` (volume).

Payload wrapper :

```ts
type HostEvent =
  | { kind: "rpc"; event: PiEvent }          // JSON Pi tel quel + kind
  | { kind: "process"; status: "spawned" | "exited" | "crashed"; code?: number }
  | { kind: "ui_request"; request: ExtensionUiRequest }
  | { kind: "log"; level: "info" | "warn" | "error"; message: string };
```

Events Pi à gérer **explicitement** dans le store UI :

| Event | UI |
|---|---|
| `agent_start` | état `running` |
| `message_start` | créer bulle (user / assistant / toolResult) |
| `message_update` / `text_delta` | append texte ; assembler par `contentIndex` |
| `message_update` / `thinking_delta` | bloc thinking repliable |
| `message_update` / `toolcall_start\|delta\|end` | carte outil en construction |
| `message_end` | snapshot autoritatif (remplace le partiel) |
| `tool_execution_start` | carte « running » |
| `tool_execution_update` | remplacer le body (Pi envoie l’accumulé, pas un delta) |
| `tool_execution_end` | success / error |
| `queue_update` | chips steer / follow-up |
| `compaction_start/end` | bandeau contexte |
| `auto_retry_start/end` | bandeau retry |
| `agent_end` | **ne pas** déverrouiller le composer |
| `agent_settled` | idle, composer libre |
| `extension_error` | toast erreur extension |
| `extension_ui_request` | modal (voir §8) |

Règle streaming Pi : `message_update` **n’a plus** de snapshot cumulatif. Le client assemble. `message_end.message` écrase.

### 6.5 Cycle de vie d’un prompt

```
UI prompt()
  → Rust write {"id","type":"prompt",...}
  → response success=true          // ack, pas la fin
  → agent_start
  → [turns: message_* + tool_execution_*]*
  → agent_end                      // peut retry
  → agent_settled                  // FIN — UI idle
```

Si `prompt` arrive pendant `isStreaming` sans `streamingBehavior` : Pi error. La GUI doit envoyer `steer` / `follow_up`, ou désactiver l’envoi.

Esc :

1. `clear_queue` → récupérer `steering` / `followUp` → remettre dans le composer.
2. `abort`.
3. Attendre `agent_settled` ou timeout 5s puis considérer idle.

---

## 7. Modèle workspace & sécurité

Pi n’a **pas** de sandbox built-in. Project trust ≠ sandbox. Donc la sécurité est **notre** produit, en couches.

### 7.1 Couches

```
L0  OS user = l’utilisateur de la machine          (inévitable)
L1  Workspace roots (allowlist chemins)            (extension workspace-roots)
L2  Protected paths (.env, .ssh, keys)             (extension protected-paths)
L3  Permission gate write/edit/bash                (extension permission-gate + GUI)
L4  Modes : ReadOnly | Ask | FullAccess            (setting, défaut Ask)
L5  (plus tard) container / Gondolin               (hors v1)
```

L1–L4 ne rendent pas bash sûr. Un `bash` approuvé peut `cat ~/.ssh/id_rsa`. Le dire dans l’UI (« cette commande a un accès shell réel ») et ne pas prétendre le contraire.

### 7.2 Workspace roots

Config app (`{app_data}/settings.json`) :

```json
{
  "activeRoot": "/home/me/projets",
  "roots": [
    "/home/me/projets",
    "/home/me/Documents"
  ],
  "permissionMode": "ask",
  "denyReadGlobs": ["**/.env", "**/.env.*", "**/*id_rsa*", "**/*.pem"]
}
```

- `activeRoot` = `cwd` du sidecar. C’est le répertoire de travail (writes par défaut).
- `roots` = allowlist. Toute path de `read` / `write` / `edit` / `ls` / `grep` / `find` est résolue (`realpath`) puis doit être **sous un root** (prefix match, pas de `..` qui sort).
- Hors roots : **block** + reason claire pour le modèle (« path outside workspace roots »).
- Changement d’`activeRoot` : `agent_restart` avec nouveau cwd.
- Ajouter un root : dialog Tauri `folder picker` → append settings → restart (l’extension relit un fichier de config au `session_start`).

Passage de la config à l’extension :

- Écrire `{app_data}/runtime/workspace.json` **avant** le spawn.
- L’extension lit ce fichier (chemin passé par env `DESKPI_WORKSPACE_FILE`).
- Ne pas parser les settings UI depuis `~/.pi`.

### 7.3 Permission modes

| Mode | read in-root | write/edit in-root | bash | hors-root |
|---|---|---|---|---|
| `readonly` | auto | block | block | block |
| `ask` (défaut) | auto | confirm | confirm | block |
| `full` | auto | auto | confirm si pattern dangereux | block |

Patterns dangereux (même en `full`) : `rm -rf`, `sudo`, `chmod 777`, `mkfs`, `dd if=`, curl|sh, overwrite `~/.ssh`. Reprendre et durcir `permission-gate.ts` officiel.

`readonly` est le bon défaut pour « récap du dossier de Fred » : l’agent lit, propose un `RECAP.md`, et **ask** au moment du write.

### 7.4 Protected paths (toujours)

Block write (et read en `ask`/`readonly`) :

- `**/.env`, `**/.env.*`
- `**/secrets.*`, `**/*credential*`
- `**/.git/objects/**` (write)
- `**/.ssh/**`, `**/.gnupg/**`, `**/.aws/**`
- le fichier settings / keychain de l’app

Inspiré de l’exemple officiel `protected-paths.ts` + `tool-override.ts`.

### 7.5 Project trust

RPC + `defaultProjectTrust: "ask"` **ignore** `.pi/` du projet. On veut ça : un repo malveillant ne doit pas charger ses propres extensions.

Nos extensions passent par `-e` (CLI) : elles se chargent **avant** le trust. Ne pas copier nos extensions dans le `.pi` du workspace utilisateur.

Flag spawn : **ne pas** passer `--approve` sauf setting expert explicite.

---

## 8. Pont Extension UI (approvals)

C’est le mécanisme officiel. Ne pas le contourner avec un permission system Rust parallèle.

### Dialog (bloquant côté Pi, async côté host)

| `method` | GUI | Response stdin |
|---|---|---|
| `select` | modal liste | `{ type, id, value }` ou `cancelled` |
| `confirm` | modal oui/non | `{ type, id, confirmed }` |
| `input` | champ texte | `{ type, id, value }` |
| `editor` | textarea | `{ type, id, value }` |

### Fire-and-forget

`notify` → toast  
`setStatus` → status bar (map `statusKey`)  
`setWidget` → bandeau au-dessus du composer  
`setTitle` → titre fenêtre Tauri  
`set_editor_text` → préfill composer

Règles host :

1. Le reader stdout **émet** `ui_request` et continue. Il n’attend pas la GUI.
2. Un `HashSet` des `id` dialogs ouverts. Timeout Pi (champ `timeout`) : Pi auto-resolve ; la GUI ferme le modal si un event ultérieur l’indique, ou on ignore une response tardive.
3. Si la GUI est en background : `tauri-plugin-notification` + badge.
4. `ctx.mode === "rpc"` et `ctx.hasUI === true` : les extensions officielles de permission marchent **si** on implémente ce pont. C’est **le** test d’acceptance bloquant.

Prompt d’approval write : montrer path relatif au root, diff si `edit`, taille si `write`. Pour bash : commande brute + cwd, wording « accès shell réel, hors sandbox ».

---

## 9. Spawn Pi — contrat process

### 9.1 Commande

```
pi --mode rpc
   --session-dir {app_data}/sessions
   -e {resource}/agent-runtime/extensions/workspace-roots.ts
   -e {resource}/agent-runtime/extensions/permission-gate.ts
   -e {resource}/agent-runtime/extensions/protected-paths.ts
   --skill {resource}/agent-runtime/skills/recap-dossier
   --skill {resource}/agent-runtime/skills/trouver-dossier
```

cwd = `activeRoot`.

Env (whitelist, pas tout l’env user) :

```
HOME, USER, PATH, LANG, TERM=dumb
ANTHROPIC_API_KEY / OPENAI_API_KEY / …  (depuis keychain)
DESKPI_WORKSPACE_FILE={app_data}/runtime/workspace.json
DESKPI_PERMISSION_MODE=ask
NO_COLOR=1
```

Ne **pas** passer `PI_OFFLINE` sauf mode air-gap.

Résolution binaire v1 :

1. `PI_BIN` env si set
2. `which pi`
3. sinon erreur GUI : « Installe Pi : `npm i -g --ignore-scripts @earendil-works/pi-coding-agent` »

Process group : `setsid` / `killpg` pour que les enfants bash meurent avec le sidecar.

### 9.2 Health

Après spawn :

1. `get_state` avec timeout 5s = ready.
2. Sinon kill + event `crashed` + message stderr tail.

Watchdog : si `isStreaming` depuis > 10 min sans event → bandeau « agent silencieux » + bouton restart. Ne pas kill automatique (un bash long est légitime).

Crash (exit ≠ 0) : UI idle, bannière, bouton relancer. Ne pas rejouer le dernier prompt tout seul.

### 9.3 App data

```
{app_data}/                     # Tauri path::app_data_dir
  settings.json
  runtime/workspace.json        # dump pour extensions
  sessions/                     # JSONL Pi
  logs/pi.stderr.log
  logs/host.log
```

Pas de secrets dans `settings.json`. Keys → keychain (`tauri-plugin-stronghold` ou plugin keyring). V1 acceptable : fichier `secrets.json` chmod 600 **hors git**, à remplacer dès que le keychain est branché.

---

## 10. Frontend — UI v1

Une fenêtre, trois zones. Pas de chrome lourd.

```
┌──────────┬────────────────────────────────────────────┐
│ Sessions │  Transcript (virtualiser si > 200 msgs)    │
│ + New    │  user / assistant / thinking / tool cards  │
│          │                                            │
│ Roots    │                                            │
│ Model    │────────────────────────────────────────────│
│          │  widgets extension · queue chips           │
│          │  composer (Enter = send, Alt+Enter =       │
│          │            follow-up si running)           │
├──────────┴────────────────────────────────────────────┤
│ model · tokens% · $session · permissionMode · root    │
└───────────────────────────────────────────────────────┘
```

Comportement clavier (aligné Pi, adapté GUI) :

- `Enter` idle → prompt ; running → steer
- `Alt+Enter` running → follow-up
- `Esc` → clear_queue + abort
- `Ctrl+L` → palette modèle (plus tard)

Tool cards : nom, args repliés, stdout scroll, badge error. `write`/`edit` : extraire `details.diff` / `details.patch` si présent.

Virtualisation du transcript (svelte-virtuoso ou équivalent) dès P1 : un récap de dossier génère beaucoup de tool events.

Thème : dark par défaut, système. Pas de design system lourd. CSS variables, ~200 lignes.

Accessibilité : focus trap sur les modals d’approval (obligatoire — c’est le contrôle de sécurité).

---

## 11. Runtime Pi : extensions & skills

### 11.1 `workspace-roots.ts`

- `session_start` : lire `DESKPI_WORKSPACE_FILE`.
- `tool_call` sur `read|write|edit|ls|grep|find` : résoudre path, `block` si hors roots.
- `bash` : ne **pas** parser le shell pour « sandboxer ». En `readonly` → block bash. En `ask`/`full` → laisser le permission-gate.
- Option : préfixer le system prompt (`before_agent_start`) avec la liste des roots et la consigne « ne sors pas de ces chemins ; pour chercher un dossier, utilise find/grep borné ».

### 11.2 `permission-gate.ts`

Fork de l’exemple officiel, paramétré par `DESKPI_PERMISSION_MODE`.

- `write` / `edit` : `ctx.ui.confirm` avec path.
- `bash` : `ctx.ui.select(["Allow","Block"])` + texte commande.
- `ctx.hasUI === false` → block (jamais YOLO headless).
- Commande `/permission ask|full|readonly` pour changer en session (status widget).

### 11.3 `protected-paths.ts`

Block inconditionnel (même `full`) sur les globs §7.4.

### 11.4 `SYSTEM.md` produit (chargé via overlay / `-e` qui append)

Court. Pi veut un prompt petit. Dire :

- Tu es un agent local. Tes roots sont listés.
- Pour un récap de dossier : find → échantillonner → lire → écrire `RECAP.md` **seulement après** avoir compris les trous.
- Ne jamais dumper un arbre entier dans le contexte.
- Ne pas lire `.env` ni clés.

### 11.5 Skill `trouver-dossier`

```yaml
name: trouver-dossier
description: >
  Localise un dossier ou un projet sur la machine à partir d’un nom
  approximatif (ex. "dossier de Fred"). Utiliser quand l’utilisateur
  désigne un dossier sans chemin. Chercher uniquement dans les workspace
  roots via find/fd/grep. Ne jamais scanner / ou $HOME entier.
```

Procédure dans le SKILL.md :

1. Normaliser le nom (fred → `*fred*`, accents).
2. `find <each-root> -maxdepth 5 -iname '*fred*' -type d`.
3. Si 0 : élargir maxdepth, proposer à l’user.
4. Si N : lister chemins + mtimes, demander confirmation via le chat (ou `ctx.ui.select` si on ajoute un petit tool).
5. Une fois choisi : `cd` mental = travailler sous ce path.

### 11.6 Skill `recap-dossier`

```yaml
name: recap-dossier
description: >
  Produit un récapitulatif structuré d’un dossier (contenu, état,
  trous, prochaines actions) et peut écrire RECAP.md. Utiliser pour
  "fais-moi un récap", "où en est le dossier X", "ce qu’il manque".
```

Procédure :

1. Inventaire borné (`ls`, `find -maxdepth 3`).
2. Lire README, docs, TODOs, 5–10 fichiers représentatifs. Pas tout.
3. Structure imposée : Vue d’ensemble · Contenu · État · Manques · Actions.
4. En `readonly` : afficher le markdown dans le chat.
5. En `ask` : proposer d’écrire `RECAP.md` à la racine du dossier (confirm write).

C’est le golden path d’acceptance.

---

## 12. Tauri : capabilities, plugins, CSP

### Plugins v1

- `tauri-plugin-shell` — spawn (binaire **externe** `pi` en v1, sidecar bundlé en phase 5)
- `tauri-plugin-dialog` — folder picker
- `tauri-plugin-notification` — approval en background
- `tauri-plugin-os` — platform
- keychain : `tauri-plugin-stronghold` **ou** fichier 600 en v1

Pas de `fs` plugin exposé à la webview. Le FS passe par Pi.

### `capabilities/default.json` (esprit)

Autoriser :

- `core:event:allow-listen` / `emit`
- `core:window:allow-set-title`
- `dialog:allow-open` (directories only)
- `notification:default`
- `shell:allow-spawn` / `allow-stdin-write` **uniquement** pour le binaire `pi` (allowlist command)

Interdire : FS webview, HTTP webview vers l’extérieur, `shell open` générique.

### CSP webview

Default Tauri strict. `connect-src` = `ipc:` seulement. Les appels LLM ne partent pas du front.

### Fenêtre

- 1100×720 min 800×560, dark.
- `create-tauri-app` : identifier `local.agent.desktop` (renommer avant ship).

---

## 13. Phases d’implémentation

Chaque phase a un critère de sortie testable. Ne pas commencer N+1 tant que N n’est pas vert.

### Phase 0 — Squelette (1 palier)

- `create-tauri-app` : Svelte + TS, pnpm, Tauri 2.
- Fenêtre vide + bouton « ping ».
- Rust spawn `pi --mode rpc --no-session`, `get_state`, afficher model ou erreur.
- Kill propre à la fermeture de fenêtre (`RunEvent::Exit`).

**Done quand :** `pnpm tauri dev` ouvre, ping affiche un model id, fermer l’app ne laisse aucun `pi` zombie (`pgrep -a pi`).

### Phase 1 — Chat streaming

- `prompt` + assemblage `text_delta`.
- `agent_settled` → idle.
- Abort Esc.
- Composer désactivé vs steer selon state.
- Hydratation `get_messages` (reprise).

**Done quand :** on envoie « réponds juste pong », le texte stream, Esc coupe, un second prompt marche.

### Phase 2 — Tool cards

- `tool_execution_start/update/end` rendus.
- `message_end` autoritatif.
- Footer `get_session_stats` (poll 2s pendant running, une fois au settled).
- Queue chips.

**Done quand :** « liste les fichiers ici » montre une carte `ls`/`bash` live puis une réponse.

### Phase 3 — Workspace + restart

- Folder picker → `activeRoot` → restart sidecar cwd.
- Session dir persisté.
- New session / liste sessions (scan JSONL headers côté Rust).
- `SYSTEM.md` + consigne roots.

**Done quand :** on change de dossier, l’agent `ls` le nouveau, l’ancien process est mort.

### Phase 4 — Sécurité (bloquant avant usage réel)

- Les 3 extensions `-e`.
- Pont `extension_ui_*` complet (select/confirm au minimum).
- Modes readonly / ask / full.
- Protected paths.
- Test : write `../` hors root → block. write `.env` → block. `rm -rf` → modal.

**Done quand :** les 3 tests ci-dessus passent à la main, et un approval « Block » se voit dans le transcript comme tool error, l’agent continue sans exécuter.

### Phase 5 — Skills golden path

- `trouver-dossier` + `recap-dossier`.
- Prompt template `/recap`.
- Mode readonly par défaut sur ce flux, ask au write `RECAP.md`.

**Done quand :** avec un dossier fixture `Fred-Projet/` dans un root, la phrase « fais-moi un récap du dossier de fred » produit un recap cohérent et propose d’écrire `RECAP.md`.

### Phase 6 — Settings & modèles

- Liste models, set_model, thinking level.
- Saisie API key (masquée) → storage → restart.
- Permission mode dans la status bar.

**Done quand :** on passe d’un modèle à un autre mid-session via `set_model` (Pi le supporte) et le footer suit.

### Phase 7 — Durcissement

- [x] Virtualisation transcript (fenêtre 200 + Load earlier ; cap `get_messages` / tool body).
- [x] Logs rotation (`host.log` / `pi.stderr.log`, 5 MiB × 3).
- [x] Watchdog silence (10 min, bandeau + Restart, pas de kill auto).
- [x] Crash banner + restart (waiter `health_tick`, pas de replay du prompt).
- [x] Compaction bandeau.
- [x] Tests framing JSONL (ligne avec U+2028 dans une string ne split pas).
- [x] Freeze UI : mutex RPC relâché pendant l’attente ; pending `fail_all` à la mort du reader.
- [x] Fenêtre invisible / process zombie : restore GTK, recreate webview, exit au close utilisateur.

### Phase 8 — Packaging (après usage quotidien)

- Sidecar Node self-contained (guide Tauri « Node.js as a sidecar ») **ou** documenter Node comme dépendance runtime.
- Icones, identifiant, signatures (macOS/Windows plus tard).
- Linux : .deb / AppImage. Cible de dev prioritaire : Linux (environnement actuel).

---

## 14. Tests

| Niveau | Quoi | Où |
|---|---|---|
| Unit Rust | split JSONL, `\r\n`, U+2028 dans string, ligne géante | `src-tauri` |
| Unit Rust | pending map : response id inconnu ignorée ; timeout | `rpc.rs` |
| Golden | fixtures stdout Pi (prompt simple, tool call, ui_request) → store events attendus | `tests/rpc_golden/` |
| Manuel Phase 4 | matrix permissions | checklist ci-dessous |
| Manuel Phase 5 | fixture `Fred-Projet` | golden path |
| Interdit | tests qui tapent une vraie API en CI sans key | skip |

Checklist sécurité manuelle (coller dans le README à Phase 4) :

- [ ] `read /etc/passwd` → block
- [ ] `read <root>/.env` → block
- [ ] `write <root>/../escape.txt` → block
- [ ] `write <root>/ok.md` en ask → modal → Allow → fichier créé
- [ ] même write → Block → pas de fichier
- [ ] `bash: rm -rf /tmp/x` → modal même en full
- [ ] readonly + `write` → block sans modal
- [ ] tuer l’app pendant un bash → pas de process orphelin

Fixture `tests/fixtures/Fred-Projet/` :

```
README.md          (projet incomplet)
notes.txt
todo.md            (3 items, 1 coché)
src/main.py        (TODO: argparse)
```

Le recap doit mentionner les TODOs ouverts. C’est le test produit.

---

## 15. Risques et mitigations

| Risque | Impact | Mitigation |
|---|---|---|
| Bash contourne L1 | Lecture secrets, destruction | Modes, wording honnête, confirm, plus tard sandbox OS |
| Prompt injection via fichiers du dossier | L’agent exécute des instructions cachées | Roots bornés, protected paths, ask sur write/bash, ne jamais `--approve` par défaut |
| JSONL mal parsé (U+2028 / buffer) | Desync protocole, hang | Framer maison, golden tests |
| Reader bloqué sur un dialog | UI freeze, events perdus | Dialog 100% async |
| Zombies bash | CPU/disk | process group kill |
| `~/.pi` user pollué / extensions hostiles d’un repo | RCE au trust | `-e` only, pas `--approve`, cwd sans charger `.pi` projet |
| Changement breaking du proto RPC Pi | App cassée | Pin version `@earendil-works/pi-coding-agent` dans le README ; changelog Pi à relire |
| Coût API d’un récap naïf | Facture | Skill impose échantillonnage ; footer cost visible |
| Node non installé | App morte au launch | Écran d’onboarding « installer Pi » Phase 0, bundling Phase 8 |
| `get_messages` gros | Freeze UI au resume | Truncate affichage, virtualiser ; ne pas JSON.parse 200 MB sur le thread UI |

---

## 16. Pinning & versions

Noter dans le README au moment du scaffold (ne pas inventer un numéro ici) :

- `@tauri-apps/cli` / `tauri` 2.x
- `@tauri-apps/plugin-shell` / `dialog` / `notification`
- `@earendil-works/pi-coding-agent` : version exacte testée + date
- Node : 22.x LTS recommandé (Pi est un package npm)
- Svelte 5, Vite 6/7 selon create-tauri-app

Commande d’install Pi documentée :

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

`--ignore-scripts` est le conseil officiel Pi.

---

## 17. Ordre de travail recommandé (prochaine session)

1. Scaffold Tauri 2 + Svelte 5 (Phase 0).
2. `SidecarManager` + `JsonlCodec` + `get_state` ping.
3. Chat text-only (Phase 1).
4. Tool cards (Phase 2).
5. **Sécurité avant skills** (Phase 3–4). Un agent YOLO sur le disque n’est pas un MVP.
6. Skills Fred (Phase 5).
7. Settings / models (Phase 6).

Ne pas designer un design system, un store Redux, ni un plugin marketplace avant la Phase 5 verte.

---

## 18. Critère de succès global

Une personne lance l’app, choisit `~/Documents` (ou un root de test), tape :

> fais-moi un récap du dossier de fred

L’agent cherche **uniquement dans les roots**, trouve `Fred-Projet`, lit un sous-ensemble, sort un recap structuré, demande avant d’écrire `RECAP.md`. Chaque write/bash a un modal. Fermer la fenêtre ne laisse aucun `pi` ni bash orphelin.

Si ça marche, le socle est le bon. Tout le reste (mémoire Hermes, sub-agents, packaging Node) s’empile après.
