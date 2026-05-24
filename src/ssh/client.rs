use std::collections::HashMap;
use std::io::{Read, Write, stdout};
use std::process::{Command, Stdio};
use std::thread;
use crossterm::{terminal::disable_raw_mode, cursor::Show, execute};
use crate::models::Host;
use crate::ssh::proxy::resolve_proxy_jump;

/// Build the connection command for `h` as an argv vector — `ssh …` normally,
/// or `mosh --ssh="ssh …" …` when `h.mosh` is set. `argv[0]` is the program.
///
/// `all_hosts` resolves multi-hop `proxy_jump` entries that name saved hosts.
pub fn build_ssh_argv(h: &Host, all_hosts: &HashMap<String, Host>) -> Vec<String> {
    // SSH option flags shared by the `ssh` invocation and mosh's `--ssh`.
    let mut ssh_opts: Vec<String> = vec!["-p".to_string(), h.port.to_string()];
    if let Some(id) = &h.identity_file {
        if !id.is_empty() {
            ssh_opts.push("-i".to_string());
            ssh_opts.push(id.clone());
        }
    }
    if let Some(j) = &h.proxy_jump {
        if let Some(resolved) = resolve_proxy_jump(j, all_hosts) {
            ssh_opts.push("-J".to_string());
            ssh_opts.push(resolved);
        }
    }
    if h.forward_agent {
        ssh_opts.push("-A".to_string());
    }

    let target = format!("{}@{}", h.username, h.host);

    if h.mosh {
        // mosh drives ssh internally for the handshake; pass our flags via --ssh.
        let inner = std::iter::once("ssh".to_string())
            .chain(ssh_opts.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        vec!["mosh".to_string(), format!("--ssh={}", inner), target]
    } else {
        let mut argv = vec!["ssh".to_string(), target];
        argv.extend(ssh_opts);
        argv
    }
}

/// Construit et exécute la commande de connexion en combinant Host + overrides CLI.
///
/// Utilise `ssh` par défaut, ou `mosh` quand `h.mosh` est activé.
///
/// `all_hosts` est utilisé pour résoudre une chaîne `proxy_jump` multi-hop
/// dont les entrées peuvent être des noms d'hôtes sauvegardés.
///
/// Returns `Some(msg)` with a short diagnostic when the session ended in
/// failure (auth denied, host unreachable, etc.) so callers can surface it
/// to the user — the TUI clears the screen on return and would otherwise
/// hide the only place ssh wrote the error.
pub fn launch_ssh(h: &Host, all_hosts: &HashMap<String, Host>, overrides: Option<&[String]>) -> Option<String> {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), Show);

    let argv = build_ssh_argv(h, all_hosts);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    if let Some(args) = overrides {
        cmd.args(args);
    }
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Some(if h.mosh {
                format!("failed to launch `mosh` — is it installed and on PATH? ({e})")
            } else {
                format!("failed to launch `ssh`: {e}")
            });
        }
    };

    // Tee child stderr: pass bytes through to our terminal so the user still
    // sees host-key warnings and interactive ssh messages live, while keeping
    // a copy in memory to surface a toast after the session ends.
    let mut child_stderr = child.stderr.take().expect("stderr piped");
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            match child_stderr.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = std::io::stderr().write_all(&chunk[..n]);
                    let _ = std::io::stderr().flush();
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    });

    let status = child.wait();
    let captured = stderr_handle.join().unwrap_or_default();

    let status = match status {
        Ok(s) => s,
        Err(e) => return Some(format!("ssh wait failed: {e}")),
    };

    if status.success() {
        return None;
    }

    Some(extract_ssh_error(&captured).unwrap_or_else(|| match status.code() {
        Some(c) => format!("ssh exited with code {c}"),
        None => "ssh terminated by signal".to_string(),
    }))
}

/// Pull the most informative line out of ssh's stderr.
///
/// SSH typically emits the relevant failure as the last non-trivial line
/// ("Permission denied …", "Could not resolve hostname …", "Connection
/// refused", "Host key verification failed."). We walk from the bottom and
/// skip benign noise like the "Permanently added … to known_hosts" warning.
fn extract_ssh_error(stderr: &str) -> Option<String> {
    for line in stderr.lines().rev() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.starts_with("Warning: Permanently added") {
            continue;
        }
        return Some(l.to_string());
    }
    None
}
