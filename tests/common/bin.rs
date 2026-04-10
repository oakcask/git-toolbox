use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

static GIT_WHOSE_BIN: OnceLock<Result<PathBuf, String>> = OnceLock::new();

pub fn git_whose_exe() -> PathBuf {
    GIT_WHOSE_BIN
        .get_or_init(resolve_git_whose_exe)
        .as_ref()
        .unwrap_or_else(|err| panic!("{err}"))
        .clone()
}

fn resolve_git_whose_exe() -> Result<PathBuf, String> {
    if let Some(exe) = option_env!("CARGO_BIN_EXE_git-whose") {
        return Ok(PathBuf::from(exe));
    }

    // `required-features` on the bin target means Cargo may skip setting
    // `CARGO_BIN_EXE_git-whose` for integration tests, so build it explicitly.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("cargo")
        .current_dir(&manifest_dir)
        .args(["build", "--bin", "git-whose", "--features", "git-whose"])
        .status()
        .map_err(|err| format!("failed to spawn cargo build for git-whose: {err}"))?;

    if !status.success() {
        return Err(format!(
            "cargo build --bin git-whose --features git-whose failed with status {status}"
        ));
    }

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"));

    Ok(target_dir
        .join("debug")
        .join(format!("git-whose{}", std::env::consts::EXE_SUFFIX)))
}
