//! Guard: shipped host sources must not bake a checkout path into the binary.

use deskpi_lib::config::{is_agent_runtime_dir, resolve_agent_runtime};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn read_src(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    fs::read_to_string(p).unwrap()
}

#[test]
fn shipped_host_sources_have_no_checkout_literal() {
    let needle = format!("{}{}", "/root", "/agent");
    for name in ["config.rs", "lib.rs", "sidecar.rs", "commands.rs"] {
        let src = read_src(name);
        assert!(
            !src.contains(&needle),
            "{name} contains a hardcoded checkout path"
        );
        assert!(
            !src.contains("env!(\"CARGO_MANIFEST_DIR\")"),
            "{name} must not use CARGO_MANIFEST_DIR (baked at compile time)"
        );
    }
}

#[test]
fn walk_up_from_exe_dir_finds_runtime() {
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

    let exe = tmp.path().join("nested/target/debug/deskpi");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, b"").unwrap();

    let found = resolve_agent_runtime(None, &BTreeMap::new(), Some(&exe), None).unwrap();
    assert!(is_agent_runtime_dir(&found));
    assert_eq!(found, runtime.canonicalize().unwrap());
}
