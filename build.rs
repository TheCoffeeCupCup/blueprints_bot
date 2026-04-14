use std::process::Command;

fn main() {
    const ENV_VAR_NAME: &'static str = "GIT_TAG";

    // Gives current HEAD git commit hash.
    // If there are are uncommitted changes, adds "-dirty" suffix.
    let git_output = Command::new("git")
        .args(["describe", "--always", "--dirty", "--exclude=\"*\""])
        .output()
        .ok();

    let git_tag = match git_output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    };

    println!("cargo:rustc-env={ENV_VAR_NAME}={git_tag}");

    // Triggers on commits
    println!("cargo:rerun-if-changed=.git/index");
    // Triggers on branch switches
    println!("cargo:rerun-if-changed=.git/HEAD");
}
