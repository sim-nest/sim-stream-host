//! Thin repository-tool launcher: defers to shared sim-tooling commands.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(args: Vec<String>) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    let command = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| usage(program))?;
    if !matches!(command, "simdoc" | "index-check" | "check-file-sizes") {
        return Err(usage(program));
    }

    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let manifest = locate_sim_tooling_manifest(&root)?;
    let mut child = Command::new("cargo");
    child.args(["run", "--manifest-path"]);
    child.arg(manifest);
    child.args(["--quiet", "--"]);
    child.arg(command);
    if command == "index-check" {
        if !has_repo_arg(&args) {
            child.arg("--repo");
            child.arg(&root);
        }
    } else {
        child.arg("--repo-root");
        child.arg(&root);
    }
    for arg in args.iter().skip(2) {
        child.arg(arg);
    }

    let status = child
        .status()
        .map_err(|err| format!("run shared sim-tooling command: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "shared sim-tooling command failed with status {status}"
        ))
    }
}

fn usage(program: &str) -> String {
    format!(
        "usage: {program} simdoc [--check] | {program} index-check [--repo PATH] [--strict SPEC] | {program} check-file-sizes"
    )
}

fn has_repo_arg(args: &[String]) -> bool {
    args.iter()
        .skip(2)
        .any(|arg| arg == "--repo" || arg.starts_with("--repo="))
}

fn locate_sim_tooling_manifest(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("SIMDOC_TOOLING_MANIFEST") {
        return Ok(PathBuf::from(path));
    }
    let sibling = repo_root
        .parent()
        .unwrap_or(repo_root)
        .join("sim-tooling")
        .join("Cargo.toml");
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err("set SIMDOC_TOOLING_MANIFEST to the sim-tooling Cargo.toml".to_owned())
}
