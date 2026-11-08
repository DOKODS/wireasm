//! Host toolchain helpers: find clang, assemble a generated .s, run it with a
//! timeout. Kept dependency-free on purpose.

use crate::model::Target;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn host_target() -> Option<Target> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Some(Target::Aarch64Macos),
        ("aarch64", "linux") => Some(Target::Aarch64Linux),
        ("x86_64", "linux") => Some(Target::X86_64Linux),
        ("x86_64", "macos") => Some(Target::X86_64Macos),
        _ => None,
    }
}

pub fn can_build(t: Target) -> bool {
    host_target() == Some(t)
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    if cmd.contains('/') {
        let p = PathBuf::from(cmd);
        return p.exists().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(cmd);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn assembler() -> Option<PathBuf> {
    which("clang").or_else(|| which("cc")).or_else(|| which("gcc"))
}

/// Assemble `asm` on disk into `exe`. Only host-native targets are supported.
pub fn assemble(asm_path: &Path, exe_path: &Path, t: Target) -> Result<(), String> {
    let cc = assembler().ok_or_else(|| "clang/cc not found in PATH".to_string())?;
    let mut cmd = Command::new(cc);
    if t.is_start() {
        cmd.arg("-nostdlib").arg("-static");
    }
    cmd.arg("-o").arg(exe_path).arg(asm_path);
    let out = cmd.output().map_err(|e| format!("failed to start linker: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "assembler failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// Syntax-check only (does not link).
pub fn syntax_check(asm_path: &Path, t: Target) -> Result<(), String> {
    let cc = assembler().ok_or_else(|| "clang/cc not found in PATH".to_string())?;
    let mut cmd = Command::new(cc);
    if t.is_aarch64() {
        cmd.arg("--target=aarch64-linux-gnu");
    } else {
        cmd.arg("--target=x86_64-linux-gnu");
    }
    cmd.arg("-c").arg("-o").arg("/dev/null").arg(asm_path);
    let out = cmd.output().map_err(|e| format!("failed to start assembler: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct RunResult {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn run(exe: &Path, timeout_secs: u64) -> Result<RunResult, String> {
    let mut child = Command::new(exe)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start program: {e}"))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(p) = stdout_pipe.as_mut() {
            let _ = p.read_to_string(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(p) = stderr_pipe.as_mut() {
            let _ = p.read_to_string(&mut buf);
        }
        buf
    });

    let start = Instant::now();
    let mut timed_out = false;
    let status;
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(timeout_secs) {
                    let _ = child.kill();
                    timed_out = true;
                    status = child.wait().ok();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("wait failed: {e}")),
        }
    }
    let out = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();
    Ok(RunResult {
        status: status.and_then(|s| s.code()),
        stdout: out,
        stderr: err,
        timed_out,
    })
}
