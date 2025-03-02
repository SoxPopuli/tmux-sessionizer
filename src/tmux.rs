use std::process::{Command, Output as ProcessOutput};

fn cmd(args: &[&str]) -> Option<ProcessOutput> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .expect("Failed to run tmux command");

    if output.status.success() {
        Some(output)
    } else {
        None
    }
}

pub fn has_session(name: &str) -> bool {
    cmd(&["has-session", "-t", name]).is_some()
}

pub fn new_session(name: &str, path: &str) {
    cmd(&["new-session", "-c", path, "-s", name, "-d"]);
}

pub fn attach(name: &str) {
    cmd(&["attach", "-t", name]);
}

pub fn switch(name: &str) {
    cmd(&["switch", "-t", name]);
}
