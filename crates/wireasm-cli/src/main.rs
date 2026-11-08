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
    r#"WireASM 0.1.0 — visual assembler compiler

Usage:
  wireasm <graph.json> [options]

Options:
  -o <file.s>        write assembler to a file (default: stdout)
  --target <t>       aarch64-macos | aarch64-linux | x86_64-linux | x86_64-macos
  --check            validate only, do not emit assembler
  --run              assemble for this host with clang and run (5 s limit)
  -h, --help         this text

Examples:
  wireasm examples/sum_loop.json -o out/sum_loop.s
  wireasm examples/hello.json --run
"#
    .to_string()
}

fn run() -> Result<u8, String> {
    let mut args = std::env::args().skip(1);
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut target: Option<String> = None;
    let mut check = false;
    let mut do_run = false;

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
                target = Some(args.next().ok_or_else(|| "--target needs a value".to_string())?);
            }
            "--check" => check = true,
            "--run" => do_run = true,
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

    if let Some(path) = &output {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, &out.asm)
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
