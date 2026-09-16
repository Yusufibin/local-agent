pub fn vendor_roots(runtime: &Path) -> Vec<PathBuf> {
    vec![runtime.to_path_buf()]
}

pub fn vendored_node(roots: &[PathBuf]) -> Option<PathBuf> {
    for root in roots {
        let p = root.join(VENDOR_NODE_REL);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn vendored_pi(roots: &[PathBuf]) -> Option<PathBuf> {
    // Prefer the real ESM entry (chunks resolve next to it). `bin/pi` may be a
    // dereferenced copy after Tauri resource bundling, which breaks `./chunks`.
    const RELS: &[&str] = &[
        "vendor/pi/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js",
        VENDOR_PI_CLI_REL,
        "vendor/pi/node_modules/.bin/pi",
        VENDOR_PI_REL,
    ];
    for root in roots {
        for rel in RELS {
            let p = root.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Priority: `PI_BIN` → vendored `vendor/pi/bin/pi` → PATH.
pub fn resolve_pi_bin(
    pi_bin: Option<&str>,
    path_env: Option<&str>,
    vendor_roots: &[PathBuf],
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
    if let Some(bundled) = vendored_pi(vendor_roots) {
        return Ok(bundled);
    }
    which_in_path("pi", path_env).ok_or_else(|| PI_INSTALL_HINT.to_string())
}

/// How to exec Pi: vendored Node + vendored `pi` script, or the resolved bin.
#[derive(Debug, Clone)]
pub struct PiLaunch {
    pub program: PathBuf,
    pub leading_args: Vec<String>,
}

pub fn resolve_pi_launch(
    pi_bin: Option<&str>,
    path_env: Option<&str>,
    vendor_roots: &[PathBuf],
) -> Result<PiLaunch, String> {
    let pi = resolve_pi_bin(pi_bin, path_env, vendor_roots)?;
    let explicit = pi_bin.map(str::trim).filter(|s| !s.is_empty()).is_some();
    if !explicit {
        if let Some(node) = vendored_node(vendor_roots) {
            if vendored_pi(vendor_roots).as_ref() == Some(&pi) {
                return Ok(PiLaunch {
                    program: node,
                    leading_args: vec![pi.display().to_string()],
                });
            }
        }
    }
    Ok(PiLaunch {
        program: pi,
        leading_args: Vec::new(),
    })
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

fn prepend_path_dir(path: &mut String, dir: &Path) {
    let dir = dir.display().to_string();
    if dir.is_empty() {
        return;
    }
    if path.is_empty() {
        *path = dir;
        return;
    }
    if path.split(':').any(|p| p == dir) {
        return;
    }
    *path = format!("{dir}:{path}");
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

fn seed_auth_candidates(runtime: &Path, host_env: &BTreeMap<String, String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(explicit) = host_env
        .get("DESKPI_SEED_AUTH")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        out.push(PathBuf::from(explicit));
    }
    out.push(runtime.join("vendor/seed/auth.json"));
    out.push(runtime.join("vendor/seed/opencode-auth.json"));
    let mut p = runtime.to_path_buf();
    for _ in 0..8 {
        out.push(p.join("packaging/seed/opencode-auth.json"));
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => break,
        }
    }
    out
}

fn seed_settings_candidates(runtime: &Path, host_env: &BTreeMap<String, String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(explicit) = host_env
        .get("DESKPI_SEED_SETTINGS")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        out.push(PathBuf::from(explicit));
    }
    out.push(runtime.join("vendor/seed/settings.json"));
    let mut p = runtime.to_path_buf();
    for _ in 0..8 {
        out.push(p.join("packaging/seed/settings.json"));
        out.push(p.join("packaging/seed/settings.example.json"));
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => break,
        }
    }
    out
}

fn chmod600(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

const DEFAULT_SEEDED_SETTINGS: &str = r#"{
  "defaultProvider": "opencode",
  "defaultModel": "muse-spark"
}
"#;

/// First launch: copy gitignored OpenCode seed into `{app_data}/pi-home/.pi/agent`.
/// Never overwrites an existing auth.json / settings.json.
pub fn seed_pi_home(paths: &AppPaths, host_env: &BTreeMap<String, String>) -> std::io::Result<()> {
    fs::create_dir_all(&paths.pi_agent_dir)?;
    let auth_dest = paths.pi_agent_dir.join("auth.json");
    let mut seeded_auth = auth_dest.is_file();
    if !seeded_auth {
        if let Some(src) = first_existing(&seed_auth_candidates(&paths.agent_runtime, host_env)) {
            fs::copy(&src, &auth_dest)?;
            chmod600(&auth_dest);
            seeded_auth = true;
        }
    }
    let settings_dest = paths.pi_agent_dir.join("settings.json");
    if !settings_dest.is_file() {
        if let Some(src) = first_existing(&seed_settings_candidates(
            &paths.agent_runtime,
            host_env,
        )) {
            fs::copy(&src, &settings_dest)?;
        } else if seeded_auth {
            atomic_write(&settings_dest, DEFAULT_SEEDED_SETTINGS.as_bytes())?;
        }
    }
    Ok(())
}

/// Merge a provider API key into `{pi_agent_dir}/auth.json` (Settings UI).
pub fn upsert_pi_auth_key(agent_dir: &Path, provider: &str, key: &str) -> HostResult<()> {
    fs::create_dir_all(agent_dir)?;
    let path = agent_dir.join("auth.json");
    let mut map: serde_json::Map<String, serde_json::Value> = if path.is_file() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        serde_json::Map::new()
    };
    map.insert(
        provider.to_string(),
        serde_json::json!({ "type": "api_key", "key": key }),
    );
    atomic_write(&path, &serde_json::to_vec_pretty(&map)?)?;
    chmod600(&path);
    Ok(())
}

#[cfg(test)]
mod bundle_tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_runtime_tree(root: &Path) {
        fs::create_dir_all(root.join("extensions")).unwrap();
        fs::create_dir_all(root.join("skills/recap-dossier")).unwrap();
        fs::create_dir_all(root.join("skills/trouver-dossier")).unwrap();
        fs::write(root.join("extensions/workspace-roots.ts"), "//").unwrap();
        fs::write(root.join("extensions/permission-gate.ts"), "//").unwrap();
        fs::write(root.join("extensions/protected-paths.ts"), "//").unwrap();
        fs::write(root.join("skills/recap-dossier/SKILL.md"), "#").unwrap();
        fs::write(root.join("skills/trouver-dossier/SKILL.md"), "#").unwrap();
    }

    #[test]
    fn resolve_pi_bin_prefers_vendored_over_path() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = tmp.path().join("agent-runtime");
        write_runtime_tree(&runtime);
        let bundled = runtime.join("vendor/pi/bin/pi");
        fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        fs::write(&bundled, b"#!/bin/sh\necho bundled\n").unwrap();
        let path_dir = tmp.path().join("pathbin");
        fs::create_dir_all(&path_dir).unwrap();
        let path_pi = path_dir.join("pi");
        fs::write(&path_pi, b"#!/bin/sh\necho path\n").unwrap();

        let found = resolve_pi_bin(None, Some(path_dir.to_str().unwrap()), &[runtime.clone()]).unwrap();
        assert_eq!(found, bundled);

        let explicit = resolve_pi_bin(
            Some(path_pi.to_str().unwrap()),
            Some(path_dir.to_str().unwrap()),
            &[runtime.clone()],
        )
        .unwrap();
        assert_eq!(explicit, path_pi);

        let missing = resolve_pi_bin(None, Some("/no/such/bin"), &[]);
        assert_eq!(missing.unwrap_err(), PI_INSTALL_HINT);
    }

    #[test]
    fn spawn_plan_uses_vendored_node_and_isolated_pi_home() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = tmp.path().join("agent-runtime");
        write_runtime_tree(&runtime);
        let node = runtime.join("vendor/node/bin/node");
        let pi = runtime.join("vendor/pi/bin/pi");
        fs::create_dir_all(node.parent().unwrap()).unwrap();
        fs::create_dir_all(pi.parent().unwrap()).unwrap();
        fs::write(&node, b"#!/bin/sh\n").unwrap();
        fs::write(&pi, b"#!/bin/sh\n").unwrap();
        let seed = runtime.join("vendor/seed/auth.json");
        fs::create_dir_all(seed.parent().unwrap()).unwrap();
        fs::write(&seed, r#"{"opencode":{"type":"api_key","key":"seed-key"}}"#).unwrap();

        let app_data = tmp.path().join("data");
        let cwd = tmp.path().join("ws");
        fs::create_dir_all(&cwd).unwrap();
        let paths = AppPaths::from_app_data(app_data, runtime);
        paths.ensure_dirs().unwrap();
        let mut settings = Settings::default();
        settings.active_root = Some(cwd.clone());
        let mut host = BTreeMap::new();
        host.insert("HOME".into(), "/home/me".into());
        host.insert("USER".into(), "me".into());
        host.insert("PATH".into(), "/usr/bin".into());
        host.insert("LANG".into(), "C".into());
        let plan = build_spawn_plan(&paths, &settings, &BTreeMap::new(), &host, None).unwrap();
        assert_eq!(plan.program, node);
        assert_eq!(plan.args.first().map(String::as_str), Some(pi.to_str().unwrap()));
        assert!(plan.args.windows(2).any(|w| w == ["--mode", "rpc"]));
        assert_eq!(plan.env.get("HOME").unwrap(), "/home/me");
        assert_eq!(
            plan.env.get(PI_AGENT_DIR_ENV).unwrap(),
            &paths.pi_agent_dir.display().to_string()
        );
        let path = plan.env.get("PATH").unwrap();
        let node_dir = node.parent().unwrap().display().to_string();
        let pi_dir = pi.parent().unwrap().display().to_string();
        assert!(
            path.split(':').next() == Some(node_dir.as_str()),
            "PATH should start with vendored node bin: {path}"
        );
        assert!(path.contains(&pi_dir), "PATH should include vendored pi bin: {path}");
        assert!(path.contains("/usr/bin"));
        let auth = paths.pi_agent_dir.join("auth.json");
        assert!(auth.is_file(), "first launch must copy seed auth");
        let raw = fs::read_to_string(&auth).unwrap();
        assert!(raw.contains("seed-key"));
        let settings_pi = paths.pi_agent_dir.join("settings.json");
        assert!(settings_pi.is_file());
        let st = fs::read_to_string(&settings_pi).unwrap();
        assert!(st.contains("opencode"));
        assert!(st.contains("muse-spark"));
        fs::write(&auth, "stay").unwrap();
        let _ = build_spawn_plan(&paths, &settings, &BTreeMap::new(), &host, None).unwrap();
        assert_eq!(fs::read_to_string(&auth).unwrap(), "stay");
    }
}
