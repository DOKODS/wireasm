//! Host toolchain helpers: find clang, assemble a generated .s, run it with a
//! timeout. Kept dependency-free on purpose.

use crate::model::Target;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
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
    if t.is_avr() {
        avr_tools().is_some()
    } else {
        host_target() == Some(t)
    }
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
        #[cfg(windows)]
        {
            let p = dir.join(format!("{cmd}.exe"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn assembler() -> Option<PathBuf> {
    which("clang")
        .or_else(|| which("cc"))
        .or_else(|| which("gcc"))
}

/// Assemble `asm` on disk into `exe`. Only host-native targets are supported.
pub fn assemble(asm_path: &Path, exe_path: &Path, t: Target) -> Result<(), String> {
    let cc = assembler().ok_or_else(|| "clang/cc not found in PATH".to_string())?;
    let mut cmd = Command::new(cc);
    if t.is_start() {
        cmd.arg("-nostdlib").arg("-static");
    }
    cmd.arg("-o").arg(exe_path).arg(asm_path);
    let out = cmd
        .output()
        .map_err(|e| format!("failed to start linker: {e}"))?;
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
    let out = cmd
        .output()
        .map_err(|e| format!("failed to start assembler: {e}"))?;
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

// ------------------------------------------------------------------ AVR tools

/// Tools found for the AVR target: avr-gcc/objcopy are required for building,
/// avrdude is required only for flashing.
#[derive(Clone, Debug)]
pub struct AvrTools {
    pub gcc: PathBuf,
    pub objcopy: PathBuf,
    pub size: Option<PathBuf>,
    pub avrdude: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct AvrBuild {
    pub asm: PathBuf,
    pub elf: PathBuf,
    pub hex: PathBuf,
    pub bin: PathBuf,
    pub size: String,
}

fn arduino_tool_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join("Library/Arduino15/packages/arduino/tools"));
        roots.push(home.join(".arduino15/packages/arduino/tools"));
        roots.push(home.join(".platformio/packages"));
        roots.push(home.join("Applications/Arduino.app/Contents/Java/hardware/tools/avr/bin"));
    }
    if let Some(appdata) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(appdata).join("Arduino15/packages/arduino/tools"));
    }
    #[cfg(target_os = "windows")]
    if let Some(programs) = std::env::var_os("PROGRAMFILES") {
        roots.push(PathBuf::from(programs).join("Arduino/hardware/tools/avr/bin"));
    }
    roots
}

fn tool_file(dir: &Path, tool: &str) -> Option<PathBuf> {
    let plain = dir.join(tool);
    if plain.is_file() {
        return Some(plain);
    }
    #[cfg(windows)]
    {
        let exe = dir.join(format!("{tool}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

/// Look for `tool` directly in `root`, in `root/bin`, and under a couple of
/// versioned subdirectories (Arduino15 keeps several versions side by side,
/// e.g. `avr-gcc/7.3.0.../bin/avr-gcc`).
fn scan_tool(root: &Path, tool: &str) -> Option<PathBuf> {
    fn scan_dir(dir: &Path, tool: &str, depth: usize) -> Option<PathBuf> {
        if let Some(p) = tool_file(dir, tool) {
            return Some(p);
        }
        if let Some(p) = tool_file(&dir.join("bin"), tool) {
            return Some(p);
        }
        if depth == 0 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let sub = entry.path();
            if sub.is_dir() {
                if let Some(p) = scan_dir(&sub, tool, depth - 1) {
                    return Some(p);
                }
            }
        }
        None
    }
    scan_dir(root, tool, 3)
}

fn detect_avr_tools() -> Option<AvrTools> {
    let roots = arduino_tool_roots();
    let gcc = which("avr-gcc").or_else(|| roots.iter().find_map(|r| scan_tool(r, "avr-gcc")))?;
    let bin_dir = gcc.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let objcopy = which("avr-objcopy").or_else(|| tool_file(&bin_dir, "avr-objcopy"))?;
    let size = which("avr-size").or_else(|| tool_file(&bin_dir, "avr-size"));
    let avrdude = which("avrdude").or_else(|| roots.iter().find_map(|r| scan_tool(r, "avrdude")));
    Some(AvrTools {
        gcc,
        objcopy,
        size,
        avrdude,
    })
}

/// Cached AVR toolchain lookup; the GUI calls this on every frame.
pub fn avr_tools() -> Option<AvrTools> {
    static CACHE: OnceLock<Option<AvrTools>> = OnceLock::new();
    CACHE.get_or_init(detect_avr_tools).clone()
}

/// Serial ports that look like an Arduino: usbmodem/usbserial on macOS,
/// ttyACM/ttyUSB on Linux. Windows ports have to be typed in by hand.
pub fn avr_ports() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        Vec::new()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/dev") else {
            return out;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let wanted = if cfg!(target_os = "macos") {
                name.starts_with("cu.")
                    && (name.contains("usb")
                        || name.contains("wchusb")
                        || name.contains("SLAB")
                        || name.contains("serial"))
            } else {
                name.starts_with("ttyACM") || name.starts_with("ttyUSB")
            };
            if wanted {
                out.push(format!("/dev/{name}"));
            }
        }
        out.sort();
        out
    }
}

/// Compile the generated assembler into `.elf`, `.hex` and `.bin` files.
pub fn build_avr(asm: &str, out_dir: &Path, stem: &str) -> Result<AvrBuild, String> {
    let tools = avr_tools().ok_or_else(|| {
        "AVR toolchain not found: install the Arduino IDE or avr-gcc and try again".to_string()
    })?;
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let asm_path = out_dir.join(format!("{stem}.s"));
    let elf_path = out_dir.join(format!("{stem}.elf"));
    let hex_path = out_dir.join(format!("{stem}.hex"));
    let bin_path = out_dir.join(format!("{stem}.bin"));
    std::fs::write(&asm_path, asm).map_err(|e| format!("write {}: {e}", asm_path.display()))?;

    let out = Command::new(&tools.gcc)
        .arg("-mmcu=atmega328p")
        .arg("-DF_CPU=16000000UL")
        .arg("-Os")
        .arg("-o")
        .arg(&elf_path)
        .arg(&asm_path)
        .output()
        .map_err(|e| format!("failed to start avr-gcc: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "avr-gcc failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    for (format, path) in [("ihex", &hex_path), ("binary", &bin_path)] {
        let out = Command::new(&tools.objcopy)
            .arg("-O")
            .arg(format)
            .arg(&elf_path)
            .arg(path)
            .output()
            .map_err(|e| format!("failed to start avr-objcopy: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "avr-objcopy failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }

    let mut size = String::new();
    if let Some(size_tool) = &tools.size {
        if let Ok(out) = Command::new(size_tool).arg(&elf_path).output() {
            size = String::from_utf8_lossy(&out.stdout).to_string();
        }
    }

    Ok(AvrBuild {
        asm: asm_path,
        elf: elf_path,
        hex: hex_path,
        bin: bin_path,
        size,
    })
}

/// Upload a `.hex` with avrdude over the Arduino bootloader.
pub fn flash_avr(hex: &Path, port: &str, board: &str) -> Result<String, String> {
    let tools = avr_tools().ok_or_else(|| "AVR toolchain not found".to_string())?;
    let avrdude = tools
        .avrdude
        .ok_or_else(|| "avrdude not found: install the Arduino IDE or avrdude".to_string())?;
    let baud = if board.eq_ignore_ascii_case("nano") {
        "57600"
    } else {
        "115200"
    };
    let out = Command::new(avrdude)
        .arg("-p")
        .arg("m328p")
        .arg("-c")
        .arg("arduino")
        .arg("-P")
        .arg(port)
        .arg("-b")
        .arg(baud)
        .arg("-D")
        .arg("-U")
        .arg(format!("flash:w:{}:i", hex.display()))
        .output()
        .map_err(|e| format!("failed to start avrdude: {e}"))?;
    let mut log = String::from_utf8_lossy(&out.stdout).to_string();
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(format!("avrdude failed:\n{log}"));
    }
    Ok(log)
}
