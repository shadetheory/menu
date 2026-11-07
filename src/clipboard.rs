//! System clipboard through a clipboard program's argv.
//!
//! The text is written to that program's stdin. It is not passed to a shell.
//! If no known tool is on PATH, the copy fails and the caller must say so.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn copy_text(text: &str, extra_dir: Option<&Path>) -> Result<String, String> {
    let (program, args) = match extra_dir {
        Some(dir) => find_in_dir(dir)?,
        None => find_tool()?,
    };
    let mut child = Command::new(&program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("{program} did not start: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("{program} did not accept the text: {err}"))?;
    }
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(program),
            Ok(Some(status)) => return Err(format!("{program} failed: {status}")),
            Ok(None) if start.elapsed() > std::time::Duration::from_millis(400) => {
                // xclip keeps running to own the selection. That is a completed copy.
                return Ok(program);
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(err) => return Err(format!("{program} failed while waiting: {err}")),
        }
    }
}

fn find_tool() -> Result<(String, Vec<String>), String> {
    let candidates = [
        ("wl-copy", Vec::new()),
        ("xclip", vec!["-selection".into(), "clipboard".into()]),
        ("xsel", vec!["--clipboard".into(), "--input".into()]),
    ];
    for (name, args) in candidates {
        if tool_on_path(name) {
            return Ok((name.to_string(), args));
        }
    }
    Err("no wl-copy, xclip, or xsel on PATH".into())
}

fn find_in_dir(dir: &Path) -> Result<(String, Vec<String>), String> {
    for name in ["wl-copy", "xclip", "xsel"] {
        if dir.join(name).is_file() {
            let args = match name {
                "xclip" => vec!["-selection".into(), "clipboard".into()],
                "xsel" => vec!["--clipboard".into(), "--input".into()],
                _ => Vec::new(),
            };
            return Ok((dir.join(name).to_string_lossy().into_owned(), args));
        }
    }
    Err("no wl-copy, xclip, or xsel in the clipboard directory".into())
}

fn tool_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn xclip_argv_receives_the_text_and_a_missing_tool_fails_honestly() {
        let dir = std::env::temp_dir().join(format!("omr-clip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("clip.txt");
        let tool = dir.join("xclip");
        std::fs::write(&tool, format!("#!/bin/sh\ncat > '{}'\n", marker.display())).unwrap();
        let mut perms = std::fs::metadata(&tool).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tool, perms).unwrap();
        let empty = dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let missing = copy_text("hello", Some(&empty)).unwrap_err();
        assert!(missing.contains("no wl-copy"));
        assert!(!missing.to_lowercase().contains("success"));
        let copied = copy_text("hello clipboard", Some(&dir)).unwrap();
        assert!(copied.ends_with("xclip"));
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "hello clipboard");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
