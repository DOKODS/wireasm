use std::path::PathBuf;
use std::process::ExitCode;
use wireasm_core::json::Json;
use wireasm_core::model::{Graph, Target};
use wireasm_core::{compile, toolchain, Severity};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("wireasm: {e}");
            ExitCode::from(1)
        }
    }
}

fn usage() -> String {
    format!(
        r#"WireASM {} — visual assembler compiler

Usage:
  wireasm <graph.json> [options]

Options:
  -o <file>          write the generated file (default: stdout)
  --target <t>       host asm | avr-atmega328p | stm32f411 | esp32 | esp8266 | c | cpp | csharp
  --check            validate only, do not emit anything
  --run              build and run a host program (asm, C, C++, C#)
  --build            build firmware/source (.elf/.hex/.bin/.exe) into --out-dir
  --out-dir <dir>    artifact directory for --build (default: out)
  --flash <value>    upload: serial port for AVR/ESP, transport for STM32 (stlink|dfu)
  --board <name>     Arduino board for AVR --flash: uno (default) or nano
  --ports            list serial ports that look like a board
  -h, --help         this text

Examples:
  wireasm examples/sum_loop.json -o out/sum_loop.s
  wireasm examples/hello.json --run
  wireasm examples/hello.json --target c --run
  wireasm examples/avr_blink.json --build
  wireasm examples/stm32_blink.json --build
  wireasm examples/esp32_blink.json --build
  wireasm examples/avr_blink.json --flash /dev/cu.usbmodem14101 --board uno
  wireasm examples/stm32_blink.json --flash stlink
  wireasm examples/esp32_blink.json --flash /dev/cu.usbserial-0001
"#,
        wireasm_core::VERSION
    )
}

fn run() -> Result<u8, String> {
    let mut args = std::env::args().skip(1);
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut target: Option<String> = None;
    let mut check = false;
    let mut do_run = false;
    let mut do_build = false;
    let mut flash_port: Option<String> = None;
    let mut board = "uno".to_string();
    let mut out_dir = PathBuf::from("out");

    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{}", usage());
                return Ok(0);
            }
            "-o" | "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or_else(|| "-o needs a path".to_string())?,
                ));
            }
            "--target" => {
                target = Some(
                    args.next()
                        .ok_or_else(|| "--target needs a value".to_string())?,
                );
            }
            "--check" => check = true,
            "--run" => do_run = true,
            "--build" => do_build = true,
            "--flash" => {
                flash_port = Some(
                    args.next()
                        .ok_or_else(|| "--flash needs a port".to_string())?,
                )
            }
            "--board" => {
                board = args
                    .next()
                    .ok_or_else(|| "--board needs a value".to_string())?
            }
            "--out-dir" => {
                out_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--out-dir needs a path".to_string())?,
                )
            }
            "--ports" => {
                let ports = toolchain::avr_ports();
                if ports.is_empty() {
                    println!("no serial ports found");
                }
                for p in ports {
                    println!("{p}");
                }
                return Ok(0);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{}", usage()));
            }
            other => {
                if input.is_some() {
                    return Err("more than one input file".to_string());
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    let input = match input {
        Some(p) => p,
        None => {
            print!("{}", usage());
            return Ok(2);
        }
    };
    let src = std::fs::read_to_string(&input)
        .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
    let value = Json::parse(&src).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut g = Graph::from_json_value(&value)?;
    if let Some(t) = target {
        g.target = t;
    }
    if Target::parse(&g.target).is_none() {
        return Err(format!("unknown target '{}'", g.target));
    }

    let out = compile(&g);
    for d in &out.diagnostics {
        let tag = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        match &d.node {
            Some(n) => eprintln!("{tag}[{n}]: {}", d.message),
            None => eprintln!("{tag}: {}", d.message),
        }
    }
    if !out.ok {
        eprintln!(
            "wireasm: {} error(s), nothing generated",
            out.diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count()
        );
        return Ok(2);
    }
    if check {
        println!(
            "wireasm: graph '{}' is valid for {} ({} nodes, {} edges)",
            g.name, g.target, out.stats.nodes, out.stats.edges
        );
        return Ok(0);
    }

    let target = Target::parse(&g.target).unwrap();
    let stem = wireasm_core::artifact_stem(&g);

    if target.is_source() {
        if flash_port.is_some() {
            return Err("--flash does not apply to C/C++/C# targets".to_string());
        }
        let build = toolchain::build_source(&out.asm, &out_dir, &stem, target)?;
        eprintln!("wireasm: source {}", build.source.display());
        eprintln!("wireasm: program {}", build.exe.display());
        if do_run {
            let res = toolchain::run(&build.exe, 5)?;
            print!("{}", res.stdout);
            eprint!("{}", res.stderr);
            if res.timed_out {
                eprintln!("wireasm: program killed after 5 s");
                return Ok(124);
            }
            let code = res.status.unwrap_or(1);
            eprintln!("wireasm: exit status {code}");
            return Ok((code & 0xff) as u8);
        }
        return Ok(0);
    }

    if do_build || flash_port.is_some() {
        if target.is_avr() {
            let build = toolchain::build_avr(&out.asm, &out_dir, &stem)?;
            eprintln!("wireasm: assembled {}", build.asm.display());
            eprintln!("wireasm: firmware {}", build.elf.display());
            eprintln!("wireasm: hex      {}", build.hex.display());
            eprintln!("wireasm: binary   {}", build.bin.display());
            if !build.size.trim().is_empty() {
                eprintln!("{}", build.size.trim_end());
            }
            if let Some(port) = &flash_port {
                eprintln!(
                    "wireasm: uploading {} to {port} ({board})",
                    build.hex.display()
                );
                let log = toolchain::flash_avr(&build.hex, port, &board)?;
                eprintln!("{log}");
                eprintln!("wireasm: upload finished");
            }
        } else if target.is_stm32() {
            let build = toolchain::build_stm32(&out.asm, &out_dir, &stem)?;
            eprintln!("wireasm: assembled {}", build.asm.display());
            eprintln!("wireasm: firmware {}", build.elf.display());
            eprintln!("wireasm: hex      {}", build.hex.display());
            eprintln!("wireasm: binary   {}", build.bin.display());
            if !build.size.trim().is_empty() {
                eprintln!("{}", build.size.trim_end());
            }
            if let Some(transport) = &flash_port {
                eprintln!("wireasm: uploading {} via {transport}", build.bin.display());
                let log = toolchain::flash_stm32(&build.bin, transport)?;
                eprintln!("{log}");
                eprintln!("wireasm: upload finished");
            }
        } else if target.is_esp() {
            if do_build && flash_port.is_none() {
                let build = toolchain::build_esp(&out.asm, &out_dir, &stem, target)?;
                eprintln!("wireasm: sketch {}", build.ino.display());
                eprintln!("wireasm: output {}", build.output_dir.display());
                for p in build.binaries {
                    eprintln!("wireasm: binary {}", p.display());
                }
            } else if let Some(port) = &flash_port {
                eprintln!("wireasm: uploading to {port} ({})", target.label());
                let log = toolchain::flash_esp(&out.asm, &out_dir, &stem, target, port)?;
                eprintln!("{log}");
                eprintln!("wireasm: upload finished");
            }
        } else {
            return Err(format!(
                "--build/--flash do not apply to target {} (use --run or -o)",
                target.as_str()
            ));
        }
        return Ok(0);
    }

    if let Some(path) = &output {
        let mut path = path.clone();
        if path.extension().is_none() {
            path.set_extension(target.source_extension());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, &out.asm)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        eprintln!(
            "wireasm: wrote {} ({} lines, target {})",
            path.display(),
            out.stats.lines,
            g.target
        );
    } else if !do_run {
        print!("{}", out.asm);
    }

    if do_run {
        let t = Target::parse(&g.target).unwrap();
        if !t.is_host_asm() {
            return Err(format!(
                "--run builds host assembler targets only; {} needs --build or --flash",
                t.as_str()
            ));
        }
        if !toolchain::can_build(t) {
            return Err(format!(
                "target {} cannot run on this host ({:?}); use -o file.s",
                t.as_str(),
                toolchain::host_target()
            ));
        }
        let dir = std::env::temp_dir().join("wireasm-cli");
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
        let stem = wireasm_core::artifact_stem(&g);
        let asm_path = dir.join(format!("{stem}.s"));
        let exe_path = dir.join(format!("{stem}.bin"));
        std::fs::write(&asm_path, &out.asm).map_err(|e| format!("write asm: {e}"))?;
        toolchain::assemble(&asm_path, &exe_path, t)?;
        eprintln!("wireasm: assembled {}", asm_path.display());
        let res = toolchain::run(&exe_path, 5)?;
        print!("{}", res.stdout);
        eprint!("{}", res.stderr);
        if res.timed_out {
            eprintln!("wireasm: program killed after 5 s");
            return Ok(124);
        }
        let code = res.status.unwrap_or(1);
        eprintln!("wireasm: exit status {code}");
        return Ok((code & 0xff) as u8);
    }

    Ok(0)
}
