//! Compare the solver against ngspice on the same circuits.
//!
//! Run `ngspice -b /tmp/bench/*.sp` for the reference numbers, then this
//! binary prints ours side by side.

use std::time::Instant;

use wireasm_sim::schematic::Schematic;
use wireasm_sim::{Circuit, Component, ComponentKind, Options, Simulator, Source, Waveform};

fn main() {
    // 1. Divider, DC.
    let mut c = Circuit::new("divider");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source::dc(12.0)),
        &["in", "0"],
    ));
    c.add(Component::new(
        "R1",
        ComponentKind::Resistor { r: 10_000.0 },
        &["in", "out"],
    ));
    c.add(Component::new(
        "R2",
        ComponentKind::Resistor { r: 4700.0 },
        &["out", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("out", "out");
    let mut sim = Simulator::new(&c, Options::default()).unwrap();
    let sol = sim.dc().unwrap();
    println!(
        "divider   v(out) = {:.6} V   (ngspice 3.836735)",
        sol.node("out")
    );

    // 2. RC low-pass, AC sweep at 10 Hz, 158.49 Hz and 10 kHz.
    let mut c = Circuit::new("rc");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source {
            dc: 0.0,
            ac: 1.0,
            waveform: None,
        }),
        &["in", "0"],
    ));
    c.add(Component::new(
        "R1",
        ComponentKind::Resistor { r: 1000.0 },
        &["in", "out"],
    ));
    c.add(Component::new(
        "C1",
        ComponentKind::Capacitor { c: 1e-6, ic: 0.0 },
        &["out", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("out", "out");
    let mut sim = Simulator::new(&c, Options::default()).unwrap();
    let sweep = sim.ac(10.0, 100_000.0, 201).unwrap();
    for f in [10.0, 158.489_319, 1000.0, 10_000.0] {
        let sweep = sim.ac(f * 0.999, f * 1.001, 2).unwrap();
        let db = 20.0 * sweep.magnitude("out")[1].max(1e-18).log10();
        println!("rc ac     f = {f:>10.3} Hz -> {db:>8.4} dB");
    }
    // Dump the sweep for the ngspice comparison script.
    if let Ok(path) = std::env::var("WIREASM_AC_CSV") {
        let mut out = String::new();
        let mag = sweep.magnitude("out");
        for (f, m) in sweep.frequency.iter().zip(mag.iter()) {
            out.push_str(&format!("{f:.9} {:.9}\n", 20.0 * m.max(1e-18).log10()));
        }
        std::fs::write(path, out).expect("write ac dump");
    }

    // 3. RC step response at the same sample points as the ngspice run.
    let mut c = Circuit::new("rc_step");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source {
            dc: 0.0,
            ac: 0.0,
            waveform: Some(Waveform::Pulse {
                low: 0.0,
                high: 1.0,
                period: 2e-3,
                duty: 0.5,
                rise: 1e-9,
                fall: 1e-9,
            }),
        }),
        &["in", "0"],
    ));
    c.add(Component::new(
        "R1",
        ComponentKind::Resistor { r: 1000.0 },
        &["in", "out"],
    ));
    c.add(Component::new(
        "C1",
        ComponentKind::Capacitor { c: 1e-6, ic: 0.0 },
        &["out", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("out", "out");
    let opts = Options {
        dt: 1e-6,
        t_stop: 2e-3,
        ..Options::default()
    };
    let mut sim = Simulator::new(&c, opts).unwrap();
    let trace = sim.transient().unwrap();
    let series = &trace.series["out"];
    for i in [0usize, 10, 100, 500, 1000] {
        println!("rc step   t = {:.6} s -> {:.6} V", trace.time[i], series[i]);
    }
    // Dump the same curve for the ngspice comparison script.
    if let Ok(path) = std::env::var("WIREASM_STEP_CSV") {
        let mut out = String::new();
        for (t, v) in trace.time.iter().zip(series.iter()) {
            out.push_str(&format!("{t:.9} {v:.9}\n"));
        }
        std::fs::write(path, out).expect("write step dump");
    }

    // 4. Diode clamp: 5 V through 220 ohm into a 2 V LED.
    let mut c = Circuit::new("led");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source::dc(5.0)),
        &["in", "0"],
    ));
    c.add(Component::new(
        "R1",
        ComponentKind::Resistor { r: 220.0 },
        &["in", "a"],
    ));
    c.add(Component::new(
        "D1",
        ComponentKind::Led(wireasm_sim::LedModel {
            vf: 2.0,
            if_nom: 0.02,
            n: 1.8,
            rs: 5.0,
        }),
        &["a", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("a", "a");
    let mut sim = Simulator::new(&c, Options::default()).unwrap();
    let sol = sim.dc().unwrap();
    println!("led       v(a)  = {:.6} V", sol.node("a"));
    println!(
        "led       i(V1) = {:.6} A",
        -sol.currents.get("V1").copied().unwrap_or(0.0)
    );

    // 5. The same divider, but built as a schematic (what the IDE does).
    let s = Schematic::divider();
    let circuit = s.netlist().unwrap();
    let mut sim = Simulator::new(&circuit, Options::default()).unwrap();
    let sol = sim.dc().unwrap();
    println!("схема     v(out) = {:.6} V", sol.node("out"));

    // 6. Speed: a 200 ms transient with 200 001 points, the same job ngspice
    //    runs in `tools/bench_ngspice.py`.
    let mut c = Circuit::new("speed");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source {
            dc: 0.0,
            ac: 0.0,
            waveform: Some(Waveform::Sine {
                amplitude: 1.0,
                frequency: 1000.0,
                phase: 0.0,
                offset: 0.0,
            }),
        }),
        &["in", "0"],
    ));
    c.add(Component::new(
        "R1",
        ComponentKind::Resistor { r: 1000.0 },
        &["in", "out"],
    ));
    c.add(Component::new(
        "C1",
        ComponentKind::Capacitor { c: 1e-6, ic: 0.0 },
        &["out", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("out", "out");
    let opts = Options {
        dt: 1e-6,
        t_stop: 2e-1,
        ..Options::default()
    };
    let started = Instant::now();
    let mut sim = Simulator::new(&c, opts).unwrap();
    let trace = sim.transient().unwrap();
    let elapsed = started.elapsed();
    println!(
        "переходный процесс: {} точек за {:?} ({:.2} мкс/точка)",
        trace.len(),
        elapsed,
        elapsed.as_secs_f64() * 1e6 / trace.len() as f64
    );

    // 7. Speed of an AC sweep with 1000 points.
    let started = Instant::now();
    let mut sim = Simulator::new(&c, Options::default()).unwrap();
    let sweep = sim.ac(10.0, 100_000.0, 1000).unwrap();
    let elapsed = started.elapsed();
    println!(
        "АЧХ: {} точек за {:?} ({:.1} мкс/точка)",
        sweep.frequency.len(),
        elapsed,
        elapsed.as_secs_f64() * 1e6 / sweep.frequency.len() as f64
    );

    // 8. A 200-node ladder: the cost of dense MNA.
    let mut c = Circuit::new("ladder");
    c.add(Component::new(
        "V1",
        ComponentKind::VSource(Source::dc(5.0)),
        &["n0", "0"],
    ));
    for i in 0..200 {
        c.add(Component::new(
            format!("R{i}"),
            ComponentKind::Resistor { r: 1000.0 },
            &[&format!("n{i}"), &format!("n{}", i + 1)],
        ));
    }
    c.add(Component::new(
        "RL",
        ComponentKind::Resistor { r: 1000.0 },
        &["n200", "0"],
    ));
    c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
    c.probe("n200", "n200");
    let started = Instant::now();
    let mut sim = Simulator::new(&c, Options::default()).unwrap();
    let sol = sim.dc().unwrap();
    let elapsed = started.elapsed();
    let expected = 5.0 / 201.0;
    println!(
        "лестница 200 R: {:.6} V (расчёт {:.6}), решение {:?}",
        sol.node("n200"),
        expected,
        elapsed
    );
}
