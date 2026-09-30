//! Embeds the source branch and commit so a running API can report exactly
//! what it was built from. Container builds have no `.git` directory, so the
//! image build passes both values as environment variables instead.

use std::process::Command;

const BRANCH_ENV: &str = "ATTRICAT_BUILD_BRANCH";
const COMMIT_ENV: &str = "ATTRICAT_BUILD_COMMIT";
const UNKNOWN: &str = "unknown";

fn main() {
    println!("cargo:rerun-if-env-changed={BRANCH_ENV}");
    println!("cargo:rerun-if-env-changed={COMMIT_ENV}");

    let branch = env_value(BRANCH_ENV)
        .or_else(|| git(&["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| UNKNOWN.to_owned());
    let commit = env_value(COMMIT_ENV)
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| UNKNOWN.to_owned());

    // Rebuild when HEAD moves to another branch or commit. `--git-path`
    // resolves correctly inside linked worktrees.
    for path in ["HEAD", "packed-refs"] {
        watch_git_path(path);
    }
    if branch != UNKNOWN && branch != "HEAD" {
        watch_git_path(&format!("refs/heads/{branch}"));
    }

    println!("cargo:rustc-env={BRANCH_ENV}={branch}");
    println!("cargo:rustc-env={COMMIT_ENV}={commit}");
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn watch_git_path(path: &str) {
    if let Some(resolved) = git(&["rev-parse", "--path-format=absolute", "--git-path", path])
        && std::path::Path::new(&resolved).exists()
    {
        println!("cargo:rerun-if-changed={resolved}");
    }
}
