//! `wiresim` — command-line mixed-signal simulation.

use std::path::PathBuf;

use wireasm_core::json::Json;
use wireasm_sim::schematic::Schematic;
use wireasm_sim::{Circuit, McuBinding, Options, Simulator, VirtualMcu};

fn main() {
    if let Err(e) = run() {
        eprintln!("wiresim: {e}");
        std::process::exit(1);
    }
}

fn usage() -> String {
    format!(
        r#"wiresim {} — circuit simulation for WireASM

Usage:
  wiresim <circuit.json> [options]

The file is either a netlist (components and pins) or a schematic saved by
the IDE (`"kind": "wireasm-circuit"`); schematics are converted first.

Options:
  --analysis <kind>   dc (default), tran or ac
  --dt <seconds>      transient step (default 1e-4)
  --stop <seconds>    transient stop time (default 1e-3)
  --fstart <hz>       AC sweep start (default 10)
  --fstop <hz>        AC sweep stop (default 100000)
  --points <n>        AC sweep points (default 40)
  --mcu <graph.json>  run a WireASM graph as the MCU (co-simulation)
  --pin <mcu>=<node>  bind an MCU pin to a circuit node, repeatable
  --vdd <volts>       logic supply for the virtual MCU (default 5)
  --csv <file>        write the trace or sweep as CSV
  -h, --help          this text

Examples:
  wiresim examples/sim_voltage_divider.json --analysis dc
  wiresim examples/sim_rc_lowpass.json --analysis tran --dt 1e-5 --stop 5e-3
  wiresim examples/sim_rc_lowpass.json --analysis ac --fstart 100 --fstop 10000
  wiresim examples/sim_mcu_led.json --mcu examples/stm32_blink.json --pin C13=led \
      --analysis tran --dt 1e-4 --stop 2
"#,
        wireasm_sim::VERSION
    )
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut input: Option<PathBuf> = None;
    let mut analysis = "dc".to_string();
    let mut opts = Options::default();
    let mut fstart = 10.0;
    let mut fstop = 100_000.0;
    let mut points = 40usize;
    let mut mcu_path: Option<PathBuf> = None;
    let mut bindings: Vec<McuBinding> = Vec::new();
    let mut vdd = 5.0;
    let mut csv: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{}", usage());
                return Ok(());
            }
            "--analysis" => analysis = next(&mut args, "--analysis")?,
            "--dt" => opts.dt = num(&next(&mut args, "--dt")?)?,
            "--stop" => opts.t_stop = num(&next(&mut args, "--stop")?)?,
            "--fstart" => fstart = num(&next(&mut args, "--fstart")?)?,
            "--fstop" => fstop = num(&next(&mut args, "--fstop")?)?,
            "--points" => points = num(&next(&mut args, "--points")?)? as usize,
            "--mcu" => mcu_path = Some(PathBuf::from(next(&mut args, "--mcu")?)),
            "--vdd" => vdd = num(&next(&mut args, "--vdd")?)?,
            "--pin" => {
                let spec = next(&mut args, "--pin")?;
                let (pin, node) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--pin needs <mcu>=<node>, got `{spec}`"))?;
                bindings.push(McuBinding {
                    pin: pin.to_string(),
                    node: node.to_string(),
                });
            }
            "--csv" => csv = Some(PathBuf::from(next(&mut args, "--csv")?)),
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => {
                if input.is_some() {
                    return Err(format!("unexpected argument `{other}`"));
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    let input = input.ok_or("give a circuit JSON file (see --help)")?;
    let src = std::fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let (circuit, auto_bindings, auto_vdd) =
        load_circuit(&src).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut sim = Simulator::new(&circuit, opts.clone())?;

    if let Some(path) = mcu_path {
        let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut mcu =
            VirtualMcu::from_json(&src).map_err(|e| format!("{}: {e}", path.display()))?;
        // Explicit `--pin` bindings win; a schematic knows its own pins.
        let bindings = if bindings.is_empty() {
            auto_bindings
        } else {
            bindings
        };
        if bindings.is_empty() {
            return Err("--mcu needs at least one --pin <mcu>=<node>".into());
        }
        mcu.run_until(0.0)?;
        sim.attach_mcu(mcu, bindings, auto_vdd.unwrap_or(vdd), 30.0);
    }

    match analysis.as_str() {
        "dc" => {
            let sol = sim.dc()?;
            println!("DC operating point — {}", circuit.name);
            for (name, v) in &sol.nodes {
                println!("  {name:<16} {v:>12.6} V");
            }
            for (id, i) in &sol.currents {
                println!("  {id:<16} {i:>12.6} A");
            }
            if let Some(mcu) = sim.mcu() {
                print_uart(mcu);
            }
        }
        "tran" => {
            let trace = sim.transient()?;
            println!(
                "Transient — {} — {} points, dt {:e}, stop {:e}",
                circuit.name,
                trace.len(),
                opts.dt,
                opts.t_stop
            );
            print_trace(&trace);
            if let Some(mcu) = sim.mcu() {
                print_uart(mcu);
            }
            if let Some(path) = csv {
                std::fs::write(&path, trace.to_csv())
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                println!("CSV: {}", path.display());
            }
        }
        "ac" => {
            let sweep = sim.ac(fstart, fstop, points)?;
            println!("AC sweep — {} — {points} points", circuit.name);
            let names = sweep.series.keys().cloned().collect::<Vec<_>>();
            println!("  {:>12}  {}", "f, Hz", names.join(" / mag dB"));
            for (i, f) in sweep.frequency.iter().enumerate() {
                print!("  {f:>12.3}");
                for name in &names {
                    let v = sweep.series.get(name).and_then(|s| s.get(i)).copied();
                    if let Some(v) = v {
                        print!("  {:>9.3} dB", 20.0 * v.abs().log10());
                    }
                }
                println!();
            }
            if let Some(path) = csv {
                std::fs::write(&path, sweep.to_csv())
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                println!("CSV: {}", path.display());
            }
        }
        other => return Err(format!("unknown analysis `{other}` (dc, tran, ac)")),
    }
    Ok(())
}

/// Read either a netlist or a schematic (the format the IDE saves).
///
/// A schematic also carries its own MCU pin bindings and logic supply, so
/// co-simulation works without `--pin` and `--vdd`.
fn load_circuit(src: &str) -> Result<(Circuit, Vec<McuBinding>, Option<f64>), String> {
    let value = Json::parse(src)?;
    if value.get("kind").and_then(|v| v.as_str()) == Some("wireasm-circuit") {
        let schematic = Schematic::from_json_value(&value)?;
        let bindings = schematic.mcu_bindings().unwrap_or_default();
        let vdd = if schematic.mcu_on {
            Some(schematic.mcu_vdd)
        } else {
            None
        };
        return Ok((schematic.netlist()?, bindings, vdd));
    }
    Ok((Circuit::from_json(&value)?, Vec::new(), None))
}

fn print_trace(trace: &wireasm_sim::Trace) {
    let names = trace
        .names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let step = (trace.len() / 20).max(1);
    print!("  {:>12}", "t, s");
    for n in &names {
        print!("  {n:>12}");
    }
    println!();
    for i in (0..trace.len()).step_by(step) {
        print!("  {:>12.6}", trace.time[i]);
        for n in &names {
            if let Some(v) = trace.series.get(n).and_then(|s| s.get(i)) {
                print!("  {v:>12.5}");
            }
        }
        println!();
    }
    for n in &names {
        if let (Some(min), Some(max), Some(rms)) = (trace.min(n), trace.peak(n), trace.rms(n)) {
            println!("  {n}: min {min:.5} V, max {max:.5} V, rms {rms:.5} V");
        }
    }
}

fn print_uart(mcu: &VirtualMcu) {
    let text = mcu.uart();
    if !text.is_empty() {
        println!("UART:");
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
    }
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn num(s: &str) -> Result<f64, String> {
    s.parse::<f64>().map_err(|_| format!("bad number `{s}`"))
}
