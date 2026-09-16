//! Settings, secrets, spawn argv/env. No provider keys in settings.json.

use crate::error::{HostError, HostResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const PI_INSTALL_HINT: &str = "npm i -g --ignore-scripts @earendil-works/pi-coding-agent";

pub const DEFAULT_DENY_READ_GLOBS: &[&str] = &[
    "**/.env",
    "**/.env.*",
    "**/*id_rsa*",
    "**/*.pem",
];

const ENV_PASSTHROUGH: &[&str] = &["HOME", "USER", "PATH", "LANG"];

const SECRET_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    "GROQ_API_KEY",
    "OPENROUTER_API_KEY",
    "MISTRAL_API_KEY",
    "XAI_API_KEY",
    "DEEPSEEK_API_KEY",
    "TOGETHER_API_KEY",
    "FIREWORKS_API_KEY",
    "COHERE_API_KEY",
    "MOONSHOT_API_KEY",
    "ZAI_API_KEY",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub active_root: Option<PathBuf>,
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
    #[serde(default = "default_deny_globs")]
    pub deny_read_globs: Vec<String>,
    /// Expert-only. Never pass `--approve` unless this is true.
    #[serde(default)]
    pub expert_approve: bool,
}

fn default_permission_mode() -> String {
    "ask".into()
}

fn default_deny_globs() -> Vec<String> {
    DEFAULT_DENY_READ_GLOBS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_root: None,
            roots: Vec::new(),
            permission_mode: default_permission_mode(),
            deny_read_globs: default_deny_globs(),
            expert_approve: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub app_data: PathBuf,
    pub settings_file: PathBuf,
    pub secrets_file: PathBuf,
    pub runtime_dir: PathBuf,
    pub workspace_file: PathBuf,
    pub sessions_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub stderr_log: PathBuf,
    pub host_log: PathBuf,
    pub agent_runtime: PathBuf,
}

impl AppPaths {
    pub fn from_app_data(app_data: PathBuf, agent_runtime: PathBuf) -> Self {
        let runtime_dir = app_data.join("runtime");
        let logs_dir = app_data.join("logs");
        Self {
            settings_file: app_data.join("settings.json"),
            secrets_file: app_data.join("secrets.json"),
            workspace_file: runtime_dir.join("workspace.json"),
            sessions_dir: app_data.join("sessions"),
            stderr_log: logs_dir.join("pi.stderr.log"),
            host_log: logs_dir.join("host.log"),
            runtime_dir,
            logs_dir,
            agent_runtime,
            app_data,
        }
    }

    pub fn ensure_dirs(&self) -> HostResult<()> {
        fs::create_dir_all(&self.runtime_dir)?;
        fs::create_dir_all(&self.sessions_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }
}

/// Fingerprint of a product `agent-runtime` tree (extensions + skills).
pub fn is_agent_runtime_dir(dir: &Path) -> bool {
    dir.join("extensions/workspace-roots.ts").is_file()
        && dir.join("extensions/permission-gate.ts").is_file()
        && dir.join("extensions/protected-paths.ts").is_file()
        && dir.join("skills/recap-dossier/SKILL.md").is_file()
        && dir.join("skills/trouver-dossier/SKILL.md").is_file()
}

fn push_runtime_candidates(out: &mut Vec<PathBuf>, base: &Path) {
    out.push(base.join("agent-runtime"));
    out.push(base.join("_up_/agent-runtime"));
    out.push(base.join("resources/agent-runtime"));
}

fn push_lib_runtime_candidates(out: &mut Vec<PathBuf>, lib_dir: &Path) {
    let Ok(entries) = fs::read_dir(lib_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.push(p.join("agent-runtime"));
        }
    }
}

/// Resolve `agent-runtime` at process start. Never bakes a compile-time
/// workspace checkout path into the binary.
///
/// Order: `DESKPI_RUNTIME` (strict)  Tauri resource dir  next to the
/// executable / `APPDIR`  walk-up from exe and cwd.
pub fn resolve_agent_runtime(
    resource_dir: Option<&Path>,
    host_env: &BTreeMap<String, String>,
    current_exe: Option<&Path>,
    cwd: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(explicit) = host_env
        .get("DESKPI_RUNTIME")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let p = PathBuf::from(explicit);
        if is_agent_runtime_dir(&p) {
            return Ok(p.canonicalize().unwrap_or(p));
        }
        return Err(format!(
            "DESKPI_RUNTIME is not a valid agent-runtime directory: {}",
            p.display()
        ));
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(res) = resource_dir {
        push_runtime_candidates(&mut candidates, res);
        // `bundle.resources: ["../agent-runtime"]` lands as `_up_/agent-runtime`.
        candidates.push(res.join("_up_/agent-runtime"));
        candidates.push(res.join("agent-runtime"));
    }
    if let Some(appdir) = host_env.get("APPDIR").filter(|s| !s.is_empty()) {
        let appdir = Path::new(appdir);
        push_runtime_candidates(&mut candidates, appdir);
        candidates.push(appdir.join("usr/lib/local-agent-desktop/agent-runtime"));
        candidates.push(appdir.join("usr/lib/local.agent.desktop/agent-runtime"));
        candidates.push(appdir.join("usr/lib/deskpi/agent-runtime"));
        // Tauri AppImage: `$APPDIR/usr/lib/<productName>/agent-runtime`
        push_lib_runtime_candidates(&mut candidates, &appdir.join("usr/lib"));
    }
    if let Some(exe) = current_exe {
        if let Some(dir) = exe.parent() {
            let mut p = dir.to_path_buf();
            for _ in 0..8 {
                push_runtime_candidates(&mut candidates, &p);
                push_lib_runtime_candidates(&mut candidates, &p.join("lib"));
                match p.parent() {
                    Some(parent) => p = parent.to_path_buf(),
                    None => break,
                }
            }
        }
    }
    if let Some(cwd) = cwd {
        let mut p = cwd.to_path_buf();
        for _ in 0..8 {
            push_runtime_candidates(&mut candidates, &p);
            match p.parent() {
                Some(parent) => p = parent.to_path_buf(),
                None => break,
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    for c in candidates {
        if !seen.insert(c.clone()) {
            continue;
        }
        if is_agent_runtime_dir(&c) {
            return Ok(c.canonicalize().unwrap_or(c));
        }
    }
    Err(
        "agent-runtime not found (set DESKPI_RUNTIME to the product runtime directory)"
            .into(),
    )
}

/// Dev convenience: if the Fred-Projet fixture exists under a checkout,
/// default the workspace root to its parent (`tests/fixtures`) so
/// `trouver-dossier` can discover `Fred-Projet`. Production builds without
/// the fixture return `None` (the user picks a folder).
pub fn default_workspace_root(search_from: &[PathBuf]) -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("DESKPI_WORKSPACE") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                return Some(p.canonicalize().unwrap_or(p));
            }
        }
    }
    for start in search_from {
        let mut p = start.clone();
        for _ in 0..10 {
            let fred = p.join("tests/fixtures/Fred-Projet");
            if fred.is_dir() {
                let fixtures = p.join("tests/fixtures");
                return Some(fixtures.canonicalize().unwrap_or(fixtures));
            }
            match p.parent() {
                Some(parent) => p = parent.to_path_buf(),
                None => break,
            }
        }
    }
    None
}

fn workspace_search_hints(runtime: &Path) -> Vec<PathBuf> {
    let mut hints = vec![runtime.to_path_buf()];
    if let Ok(cwd) = std::env::current_dir() {
        hints.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            hints.push(dir.to_path_buf());
        }
    }
    hints
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFile {
    pub active_root: PathBuf,
    pub roots: Vec<PathBuf>,
    pub permission_mode: String,
    pub deny_read_globs: Vec<String>,
    #[serde(default)]
    pub settings_file: PathBuf,
    #[serde(default)]
    pub secrets_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SpawnPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub session_dir: PathBuf,
    pub stderr_log: PathBuf,
}

pub fn resolve_pi_bin(
    pi_bin: Option<&str>,
    path_env: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(explicit) = pi_bin.map(str::trim).filter(|s| !s.is_empty()) {
        let p = PathBuf::from(explicit);
        if p.exists() {
            return Ok(p);
        }
        if let Some(found) = which_in_path(explicit, path_env) {
            return Ok(found);
        }
        return Ok(p);
    }
    which_in_path("pi", path_env).ok_or_else(|| PI_INSTALL_HINT.to_string())
}

pub fn which_in_path(name: &str, path_env: Option<&str>) -> Option<PathBuf> {
    let path = path_env?;
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn build_pi_args(session_dir: &Path, runtime: &Path, expert_approve: bool) -> Vec<String> {
    let ext = runtime.join("extensions");
    let skills = runtime.join("skills");
    let mut args = vec![
        "--mode".into(),
        "rpc".into(),
        "--session-dir".into(),
        session_dir.display().to_string(),
        "--no-extensions".into(),
        "-e".into(),
        ext.join("workspace-roots.ts").display().to_string(),
        "-e".into(),
        ext.join("permission-gate.ts").display().to_string(),
        "-e".into(),
        ext.join("protected-paths.ts").display().to_string(),
        "--no-skills".into(),
        "--skill".into(),
        skills.join("recap-dossier").display().to_string(),
        "--skill".into(),
        skills.join("trouver-dossier").display().to_string(),
        "--no-prompt-templates".into(),
        "--prompt-template".into(),
        runtime.join("prompts/recap.md").display().to_string(),
    ];
    if expert_approve {
        args.push("--approve".into());
    }
    args
}

pub fn build_spawn_env(
    host_env: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
    workspace_file: &Path,
    permission_mode: &str,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for key in ENV_PASSTHROUGH {
        if let Some(v) = host_env.get(*key) {
            env.insert((*key).to_string(), v.clone());
        }
    }
    env.insert("TERM".into(), "dumb".into());
    env.insert("NO_COLOR".into(), "1".into());
    env.insert(
        "DESKPI_WORKSPACE_FILE".into(),
        workspace_file.display().to_string(),
    );
    env.insert(
        "DESKPI_PERMISSION_MODE".into(),
        permission_mode.to_string(),
    );
    for (k, v) in secrets {
        if SECRET_ENV_KEYS.contains(&k.as_str()) || k.ends_with("_API_KEY") {
            env.insert(k.clone(), v.clone());
        }
    }
    env
}

pub fn provider_to_env_key(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "anthropic" => "ANTHROPIC_API_KEY".into(),
        "openai" => "OPENAI_API_KEY".into(),
        "google" | "gemini" => "GOOGLE_API_KEY".into(),
        "groq" => "GROQ_API_KEY".into(),
        "openrouter" => "OPENROUTER_API_KEY".into(),
        "mistral" => "MISTRAL_API_KEY".into(),
        "xai" | "grok" => "XAI_API_KEY".into(),
        "deepseek" => "DEEPSEEK_API_KEY".into(),
        other => format!("{}_API_KEY", other.to_ascii_uppercase()),
    }
}

pub fn load_settings(path: &Path, runtime: &Path) -> HostResult<Settings> {
    let mut settings = if !path.exists() {
        Settings::default()
    } else {
        let raw = fs::read_to_string(path)?;
        serde_json::from_str(&raw)?
    };
    if settings.active_root.is_none() {
        if let Some(root) = default_workspace_root(&workspace_search_hints(runtime)) {
            settings.active_root = Some(root.clone());
            if !settings.roots.contains(&root) {
                settings.roots.insert(0, root);
            }
        }
    }
    Ok(settings)
}

pub fn save_settings(path: &Path, settings: &Settings) -> HostResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(path, &serde_json::to_vec_pretty(settings)?)?;
    Ok(())
}

pub fn load_secrets(path: &Path) -> HostResult<BTreeMap<String, String>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_secrets(path: &Path, secrets: &BTreeMap<String, String>) -> HostResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(path, &serde_json::to_vec_pretty(secrets)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn canonicalize_dir(p: PathBuf) -> PathBuf {
    p.canonicalize().unwrap_or(p)
}

pub fn write_workspace_file(paths: &AppPaths, settings: &Settings) -> HostResult<()> {
    let active = canonicalize_dir(
        settings
            .active_root
            .clone()
            .ok_or_else(|| HostError::from("no active workspace root"))?,
    );
    let mut roots: Vec<PathBuf> = settings.roots.iter().cloned().map(canonicalize_dir).collect();
    if !roots.iter().any(|r| r == &active) {
        roots.insert(0, active.clone());
    }
    let payload = WorkspaceFile {
        active_root: active,
        roots,
        permission_mode: settings.permission_mode.clone(),
        deny_read_globs: settings.deny_read_globs.clone(),
        settings_file: paths.settings_file.clone(),
        secrets_file: paths.secrets_file.clone(),
    };
    fs::create_dir_all(&paths.runtime_dir)?;
    atomic_write(&paths.workspace_file, &serde_json::to_vec_pretty(&payload)?)?;
    Ok(())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn current_host_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

pub fn build_spawn_plan(
    paths: &AppPaths,
    settings: &Settings,
    secrets: &BTreeMap<String, String>,
    host_env: &BTreeMap<String, String>,
    pi_bin_override: Option<&str>,
) -> Result<SpawnPlan, String> {
    let program = resolve_pi_bin(
        pi_bin_override.or_else(|| host_env.get("PI_BIN").map(String::as_str)),
        host_env.get("PATH").map(String::as_str),
    )?;
    let cwd = settings
        .active_root
        .clone()
        .ok_or_else(|| "no active workspace root".to_string())?;
    let args = build_pi_args(&paths.sessions_dir, &paths.agent_runtime, settings.expert_approve);
    let env = build_spawn_env(
        host_env,
        secrets,
        &paths.workspace_file,
        &settings.permission_mode,
    );
    Ok(SpawnPlan {
        program,
        args,
        cwd,
        env,
        session_dir: paths.sessions_dir.clone(),
        stderr_log: paths.stderr_log.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_args_have_mode_rpc_three_extensions_two_skills_no_approve() {
        let args = build_pi_args(
            Path::new("/tmp/sessions"),
            Path::new("/tmp/agent-runtime"),
            false,
        );
        assert!(args.windows(2).any(|w| w == ["--mode", "rpc"]));
        assert!(args.windows(2).any(|w| w == ["--session-dir", "/tmp/sessions"]));
        let e_count = args.iter().filter(|a| *a == "-e").count();
        let skill_count = args.iter().filter(|a| *a == "--skill").count();
        assert_eq!(e_count, 3);
        assert_eq!(skill_count, 2);
        assert!(args.iter().any(|a| a.as_str() == "--no-extensions"));
        assert!(args.iter().any(|a| a.as_str() == "--no-skills"));
        assert!(args.iter().any(|a| a.as_str() == "--no-prompt-templates"));
        assert!(args.iter().any(|a| a.ends_with("workspace-roots.ts")));
        assert!(args.iter().any(|a| a.ends_with("permission-gate.ts")));
        assert!(args.iter().any(|a| a.ends_with("protected-paths.ts")));
        assert!(args.iter().any(|a| a.ends_with("recap-dossier")));
        assert!(args.iter().any(|a| a.ends_with("trouver-dossier")));
        assert!(args.iter().any(|a| a.ends_with("recap.md")));
        assert!(!args.iter().any(|a| a == "--approve"));
    }

    #[test]
    fn resolve_runtime_uses_env_override_and_never_needs_compile_time_root() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = tmp.path().join("agent-runtime");
        fs::create_dir_all(runtime.join("extensions")).unwrap();
        fs::create_dir_all(runtime.join("skills/recap-dossier")).unwrap();
        fs::create_dir_all(runtime.join("skills/trouver-dossier")).unwrap();
        fs::write(runtime.join("extensions/workspace-roots.ts"), "//").unwrap();
        fs::write(runtime.join("extensions/permission-gate.ts"), "//").unwrap();
        fs::write(runtime.join("extensions/protected-paths.ts"), "//").unwrap();
        fs::write(runtime.join("skills/recap-dossier/SKILL.md"), "#").unwrap();
        fs::write(runtime.join("skills/trouver-dossier/SKILL.md"), "#").unwrap();

        let mut env = BTreeMap::new();
        env.insert(
            "DESKPI_RUNTIME".into(),
            runtime.display().to_string(),
        );
        let found = resolve_agent_runtime(None, &env, None, None).unwrap();
        assert_eq!(found, runtime.canonicalize().unwrap());

        let mut bad = BTreeMap::new();
        bad.insert("DESKPI_RUNTIME".into(), tmp.path().join("nope").display().to_string());
        assert!(resolve_agent_runtime(None, &bad, None, None).is_err());
    }

    #[test]
    fn resolve_runtime_from_appdir_product_lib() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = tmp.path().join("usr/lib/Local Agent/agent-runtime");
        fs::create_dir_all(runtime.join("extensions")).unwrap();
        fs::create_dir_all(runtime.join("skills/recap-dossier")).unwrap();
        fs::create_dir_all(runtime.join("skills/trouver-dossier")).unwrap();
        fs::write(runtime.join("extensions/workspace-roots.ts"), "//").unwrap();
        fs::write(runtime.join("extensions/permission-gate.ts"), "//").unwrap();
        fs::write(runtime.join("extensions/protected-paths.ts"), "//").unwrap();
        fs::write(runtime.join("skills/recap-dossier/SKILL.md"), "#").unwrap();
        fs::write(runtime.join("skills/trouver-dossier/SKILL.md"), "#").unwrap();
        let mut env = BTreeMap::new();
        env.insert("APPDIR".into(), tmp.path().display().to_string());
        let found = resolve_agent_runtime(None, &env, None, None).unwrap();
        assert_eq!(found, runtime.canonicalize().unwrap());
    }

    #[test]
    fn default_workspace_is_fixtures_parent_when_fred_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("checkout");
        let fred = repo.join("tests/fixtures/Fred-Projet");
        fs::create_dir_all(&fred).unwrap();
        let runtime = repo.join("agent-runtime");
        fs::create_dir_all(&runtime).unwrap();
        let found = default_workspace_root(&[runtime]).unwrap();
        assert_eq!(found, repo.join("tests/fixtures").canonicalize().unwrap());
        assert!(found.join("Fred-Projet").is_dir());
    }

    #[test]
    fn env_is_whitelisted() {
        let mut host = BTreeMap::new();
        host.insert("HOME".into(), "/home/me".into());
        host.insert("USER".into(), "me".into());
        host.insert("PATH".into(), "/usr/bin".into());
        host.insert("LANG".into(), "C".into());
        host.insert("SECRET_STUFF".into(), "nope".into());
        host.insert("ANTHROPIC_API_KEY".into(), "from-host-should-not-pass".into());
        let mut secrets = BTreeMap::new();
        secrets.insert("ANTHROPIC_API_KEY".into(), "sk-test".into());
        let env = build_spawn_env(
            &host,
            &secrets,
            Path::new("/tmp/workspace.json"),
            "ask",
        );
        assert_eq!(env.get("TERM").unwrap(), "dumb");
        assert_eq!(env.get("NO_COLOR").unwrap(), "1");
        assert_eq!(env.get("DESKPI_PERMISSION_MODE").unwrap(), "ask");
        assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "sk-test");
        assert!(!env.contains_key("SECRET_STUFF"));
        assert_eq!(env.len(), 9); // HOME USER PATH LANG TERM NO_COLOR DESKPI_* x2 + key
    }
}
