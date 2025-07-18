use std::process::Command;

fn main() {
    // Get git commit hash
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git command");

    let git_hash = String::from_utf8(output.stdout).unwrap();
    let git_hash = git_hash.trim();

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);

    tauri_build::build()
}
