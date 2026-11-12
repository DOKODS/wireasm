//! Modified nodal analysis with Newton-Raphson for diodes, backward-Euler
//! transient analysis and a complex AC sweep.
//!
//! The solver is deliberately explicit: stamps are written out in
//! [`Simulator::stamp_linear`], so the math can be checked against a textbook
//! and extended without a macro layer. Nonlinear devices are linearized at
//! each Newton step; voltage sources get one extra MNA unknown, while gates and
//! MCU pins use a Norton equivalent (Thevenin voltage plus output resistance),
//! which keeps the matrix smaller.

use std::collections::{BTreeMap, HashSet};

use crate::linalg::{CMatrix, Matrix, C};
use crate::mcu::VirtualMcu;
use crate::netlist::{Circuit, Component, ComponentKind, DiodeModel, LedModel};
use crate::trace::{AcSweep, Trace};

#[derive(Clone, Debug)]
pub struct Options {
    /// Ambient temperature in Celsius.
    pub temperature: f64,
    /// Minimum conductance from every node to ground.
    pub gmin: f64,
    /// Absolute voltage tolerance for convergence.
    pub vntol: f64,
    /// Relative voltage tolerance.
    pub reltol: f64,
    pub max_iter: usize,
    /// Timestep and stop time for transient analysis.
    pub dt: f64,
    pub t_stop: f64,
    /// Rounds used to settle digital gates after an analog solve.
    pub settle: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            temperature: 27.0,
            gmin: 1e-12,
            vntol: 1e-6,
            reltol: 1e-3,
            max_iter: 80,
            dt: 1e-4,
            t_stop: 1e-3,
            settle: 16,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Solution {
    pub time: f64,
    pub nodes: BTreeMap<String, f64>,
    pub currents: BTreeMap<String, f64>,
}

impl Solution {
    pub fn node(&self, name: &str) -> f64 {
        self.nodes.get(name).copied().unwrap_or(0.0)
    }

    pub fn voltage(&self, a: &str, b: &str) -> f64 {
        self.node(a) - self.node(b)
    }
}

/// Connect a virtual MCU pin to a circuit node.
#[derive(Clone, Debug)]
pub struct McuBinding {
    pub pin: String,
    pub node: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Dc,
    Tran,
}

#[derive(Clone, Copy, Debug)]
struct DiodeState {
    g: f64,
    v: f64,
}

pub struct Simulator<'a> {
    circuit: &'a Circuit,
    opts: Options,
    nodes: Vec<String>,
    index: BTreeMap<String, usize>,
    ground: HashSet<String>,
    sources: Vec<String>,
    source_pos: BTreeMap<String, usize>,
    cap_v: BTreeMap<String, f64>,
    ind_i: BTreeMap<String, f64>,
    gate_v: BTreeMap<String, f64>,
    /// Output voltage of every logic unit, keyed by component id.
    logic_v: BTreeMap<String, f64>,
    /// State of every clocked unit, keyed by component id.
    seq_state: BTreeMap<String, crate::ic::State>,
    pin_v: BTreeMap<String, f64>,
    pin_comps: BTreeMap<String, Vec<String>>,
    diode_g: BTreeMap<String, f64>,
    mcu: Option<VirtualMcu>,
    bindings: Vec<McuBinding>,
    mcu_vdd: f64,
    mcu_r: f64,
    time: f64,
}

impl<'a> Simulator<'a> {
    pub fn new(circuit: &'a Circuit, opts: Options) -> Result<Simulator<'a>, String> {
        circuit.validate()?;
        let ground: HashSet<String> = circuit.ground_nodes().into_iter().collect();
        let mut nodes = Vec::new();
        let mut index = BTreeMap::new();
        let mut sources = Vec::new();
        let mut source_pos = BTreeMap::new();
        let mut cap_v = BTreeMap::new();
        let mut ind_i = BTreeMap::new();
        let mut gate_v = BTreeMap::new();
        let mut logic_v = BTreeMap::new();
        let mut seq_state = BTreeMap::new();
        let mut pin_v = BTreeMap::new();
        let mut pin_comps: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for c in &circuit.components {
            match &c.kind {
                ComponentKind::VSource(_) => {
                    source_pos.insert(c.id.clone(), sources.len());
                    sources.push(c.id.clone());
                }
                ComponentKind::Capacitor { ic, .. } => {
                    cap_v.insert(c.id.clone(), *ic);
                }
                ComponentKind::Inductor { ic, .. } => {
                    ind_i.insert(c.id.clone(), *ic);
                }
                ComponentKind::Gate(_) => {
                    gate_v.insert(c.id.clone(), 0.0);
                }
                ComponentKind::Logic(l) => {
                    for out in &c.pins[l.n_in..] {
                        logic_v.insert(out.clone(), 0.0);
                    }
                }
                ComponentKind::Seq(s) => {
                    for out in &c.pins[s.n_in..] {
                        logic_v.insert(out.clone(), 0.0);
                    }
                    seq_state.insert(c.id.clone(), crate::ic::State::new());
                }
                ComponentKind::McuPin => {
                    pin_v.insert(c.id.clone(), 0.0);
                    for p in &c.pins {
                        pin_comps.entry(p.clone()).or_default().push(c.id.clone());
                    }
                }
                _ => {}
            }
            for p in &c.pins {
                if ground.contains(p) || index.contains_key(p) {
                    continue;
                }
                index.insert(p.clone(), nodes.len());
                nodes.push(p.clone());
            }
        }
        if nodes.is_empty() {
            return Err("circuit has no non-ground nodes".into());
        }

        Ok(Simulator {
            circuit,
            opts,
            nodes,
            index,
            ground,
            sources,
            source_pos,
            cap_v,
            ind_i,
            gate_v,
            logic_v,
            seq_state,
            pin_v,
            pin_comps,
            diode_g: BTreeMap::new(),
            mcu: None,
            bindings: Vec::new(),
            mcu_vdd: 5.0,
            mcu_r: 30.0,
            time: 0.0,
        })
    }

    pub fn options(&self) -> &Options {
        &self.opts
    }

    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.opts
    }

    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }

    /// Attach the graph-level virtual MCU. `bindings` maps MCU pin names
    /// (`13`, `C13`) to circuit nodes. Pins without a binding stay high-Z.
    pub fn attach_mcu(
        &mut self,
        mcu: VirtualMcu,
        bindings: Vec<McuBinding>,
        vdd: f64,
        output_resistance: f64,
    ) {
        self.mcu = Some(mcu);
        self.bindings = bindings;
        self.mcu_vdd = vdd;
        self.mcu_r = output_resistance.max(0.01);
    }

    pub fn detach_mcu(&mut self) {
        self.mcu = None;
        self.bindings.clear();
    }

    pub fn mcu(&self) -> Option<&VirtualMcu> {
        self.mcu.as_ref()
    }

    pub fn mcu_mut(&mut self) -> Option<&mut VirtualMcu> {
        self.mcu.as_mut()
    }

    pub fn set_pin(&mut self, node: &str, voltage: f64) {
        if let Some(ids) = self.pin_comps.get(node) {
            for id in ids {
                self.pin_v.insert(id.clone(), voltage);
            }
        }
    }

    /// DC operating point with capacitors open and inductors shorted.
    pub fn dc(&mut self) -> Result<Solution, String> {
        // If an MCU is attached, its pins must be at their t=0 state before the
        // operating point is solved.
        self.advance_to_mcu_time(0.0)?;
        let sol = self.solve_point(Mode::Dc, 0.0)?;
        self.time = 0.0;
        Ok(sol)
    }

    /// Transient analysis using `opts.dt` and `opts.t_stop`.
    pub fn transient(&mut self) -> Result<Trace, String> {
        let dt = self.opts.dt;
        let stop = self.opts.t_stop;
        if !(dt.is_finite() && dt > 0.0) {
            return Err("transient needs dt > 0".into());
        }
        if !(stop.is_finite() && stop > 0.0) {
            return Err("transient needs t_stop > 0".into());
        }
        let steps = (stop / dt).ceil() as usize;
        if steps > 50_000_000 {
            return Err(format!(
                "transient would take {steps} steps; increase dt or reduce t_stop"
            ));
        }
        let names = self.probe_names();
        let mut trace = Trace::default();
        for n in &names {
            trace
                .series
                .insert(n.clone(), Vec::with_capacity(steps + 1));
        }

        // t=0 respects the initial conditions: capacitors are held at their
        // `ic` voltage, inductors carry their `ic` current. Backward Euler then
        // starts from that point instead of taking a phantom first step.
        self.reset_reactive_state();
        self.advance_to_mcu_time(0.0)?;
        let sol = self.solve_initial()?;
        self.push_sample(&mut trace, 0.0, &sol);

        for k in 1..=steps {
            let t = k as f64 * dt;
            self.advance_to_mcu_time(t)?;
            self.time = t;
            let sol = self.solve_point(Mode::Tran, t)?;
            self.update_reactive_state(&sol);
            self.push_sample(&mut trace, t, &sol);
        }
        Ok(trace)
    }

    /// Complex AC sweep, log-spaced from `f_start` to `f_stop`.
    ///
    /// The operating point is solved first; diodes are linearized around it.
    /// Sources with `ac != 0` drive the sweep. If no source declares an AC
    /// amplitude, the first voltage source is used with 1 V.
    pub fn ac(&mut self, f_start: f64, f_stop: f64, points: usize) -> Result<AcSweep, String> {
        if f_start <= 0.0 || f_stop < f_start || points < 2 {
            return Err("AC sweep needs 0 < f_start <= f_stop and at least 2 points".into());
        }
        self.dc()?;
        let mut sweep = AcSweep::default();
        let names = self.probe_names();
        for n in &names {
            sweep.series.insert(n.clone(), Vec::with_capacity(points));
        }
        let has_ac = self
            .circuit
            .components
            .iter()
            .any(|c| matches!(&c.kind, ComponentKind::VSource(s) if s.ac != 0.0));
        let fallback = self.sources.first().cloned();
        let n = self.nodes.len();
        let size = n + self.sources.len();
        for i in 0..points {
            let f = log_frequency(f_start, f_stop, i, points);
            sweep.frequency.push(f);
            let omega = 2.0 * std::f64::consts::PI * f;
            let mut a = CMatrix::zeros(size);
            let mut b = vec![C::ZERO; size];
            for k in 0..n {
                a.add(k, k, C::new(self.opts.gmin, 0.0));
            }
            for c in &self.circuit.components {
                match &c.kind {
                    ComponentKind::Resistor { r } => {
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], C::new(1.0 / r, 0.0))
                    }
                    ComponentKind::Capacitor { c: cap, .. } => {
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], C::jw(omega) * *cap)
                    }
                    ComponentKind::Inductor { l, .. } => {
                        let y = if omega == 0.0 {
                            C::new(1e9, 0.0)
                        } else {
                            C::ONE / (C::jw(omega) * *l)
                        };
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], y);
                    }
                    ComponentKind::VSource(s) => {
                        let ac = if has_ac {
                            s.ac
                        } else if fallback.as_deref() == Some(c.id.as_str()) {
                            1.0
                        } else {
                            0.0
                        };
                        if let Some(&pos) = self.source_pos.get(&c.id) {
                            let k = n + pos;
                            self.stamp_source_row(&mut a, &mut b, c, k, C::new(ac, 0.0));
                        }
                    }
                    ComponentKind::ISource(s) => {
                        self.add_current_c(&mut b, &c.pins[0], &c.pins[1], C::new(s.ac, 0.0))
                    }
                    ComponentKind::Diode(_) => {
                        let g = self.diode_g.get(&c.id).copied().unwrap_or(1e-9);
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], C::new(g, 0.0));
                    }
                    ComponentKind::Led(l) => {
                        let g = self
                            .diode_g
                            .get(&c.id)
                            .copied()
                            .unwrap_or(1.0 / l.rs.max(0.01));
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], C::new(g, 0.0));
                    }
                    ComponentKind::Switch { on, ron, roff } => {
                        let r = if *on { *ron } else { *roff };
                        self.stamp_c(&mut a, &c.pins[0], &c.pins[1], C::new(1.0 / r, 0.0));
                    }
                    ComponentKind::Gate(g) => {
                        if let Some(out) = c.pins.last() {
                            self.stamp_norton_c(&mut a, out, 1.0 / g.r);
                        }
                    }
                    ComponentKind::Instrument(_) => {}
                    ComponentKind::Logic(l) => {
                        for out in &c.pins[l.n_in..] {
                            self.stamp_norton_c(&mut a, out, 1.0 / l.r);
                        }
                    }
                    ComponentKind::Seq(s) => {
                        for out in &c.pins[s.n_in..] {
                            self.stamp_norton_c(&mut a, out, 1.0 / s.r);
                        }
                    }
                    ComponentKind::McuPin => {
                        if let Some(out) = c.pins.first() {
                            self.stamp_norton_c(&mut a, out, 1.0 / self.mcu_r);
                        }
                    }
                    ComponentKind::Ground => {}
                }
            }
            let x = a.solve(b)?;
            for name in &names {
                let node = self.probe_node(name);
                let v = self.voltage_c(&x, &node, "0");
                sweep.series.get_mut(name).unwrap().push(v);
            }
        }
        Ok(sweep)
    }

    /// Names of trace series: explicit probes, or every non-ground node.
    pub fn probe_names(&self) -> Vec<String> {
        if self.circuit.probes.is_empty() {
            return self.nodes.clone();
        }
        self.circuit.probes.iter().map(|p| p.name.clone()).collect()
    }

    /// The nodes an instrument watches, with the name of the instrument.
    ///
    /// The measurements of an instrument are recorded under the name of the
    /// instrument and the channel, so a scope shows the nodes it is wired to.
    pub fn instrument_channels(&self) -> Vec<(String, usize, String)> {
        let mut out = Vec::new();
        for c in &self.circuit.components {
            let ComponentKind::Instrument(inst) = &c.kind else {
                continue;
            };
            for (index, node) in inst.channels.iter().enumerate() {
                out.push((c.id.clone(), index, node.clone()));
            }
        }
        out
    }

    fn probe_node(&self, name: &str) -> String {
        self.circuit
            .probes
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.node.clone())
            .unwrap_or_else(|| name.to_string())
    }

    fn probe_value(&self, sol: &Solution, name: &str) -> f64 {
        sol.node(&self.probe_node(name))
    }

    fn push_sample(&self, trace: &mut Trace, t: f64, sol: &Solution) {
        trace.time.push(t);
        let names: Vec<String> = trace.series.keys().cloned().collect();
        for name in names {
            let v = self.probe_value(sol, &name);
            if let Some(s) = trace.series.get_mut(&name) {
                s.push(v);
            }
        }
        // The instruments record what they see on their own channels.
        for (id, index, node) in self.instrument_channels() {
            let v = sol.node(&node);
            trace
                .instruments
                .entry(id)
                .or_default()
                .entry(index)
                .or_default()
                .push(v);
        }
    }

    /// Restore every stateful device to the values declared in the netlist.
    fn reset_reactive_state(&mut self) {
        self.cap_v.clear();
        self.ind_i.clear();
        self.gate_v.clear();
        self.pin_v.clear();
        self.diode_g.clear();
        for c in &self.circuit.components {
            match &c.kind {
                ComponentKind::Capacitor { ic, .. } => {
                    self.cap_v.insert(c.id.clone(), *ic);
                }
                ComponentKind::Inductor { ic, .. } => {
                    self.ind_i.insert(c.id.clone(), *ic);
                }
                ComponentKind::Gate(_) => {
                    self.gate_v.insert(c.id.clone(), 0.0);
                }
                ComponentKind::McuPin => {
                    self.pin_v.insert(c.id.clone(), 0.0);
                }
                _ => {}
            }
        }
    }

    /// Operating point at t=0 that honours the initial conditions: capacitors
    /// become voltage sources of `ic`, inductors become current sources of
    /// `ic`. This is what a scope shows at t=0 before the first Euler step.
    fn solve_initial(&mut self) -> Result<Solution, String> {
        let n = self.nodes.len();
        let caps = self
            .circuit
            .components
            .iter()
            .filter(|c| matches!(c.kind, ComponentKind::Capacitor { .. }))
            .count();
        let size = n + self.sources.len() + caps;
        let mut a = Matrix::zeros(size);
        let mut b = vec![0.0; size];
        for k in 0..n {
            a.add(k, k, self.opts.gmin);
        }
        let mut cap_index = 0;
        for c in &self.circuit.components {
            match &c.kind {
                ComponentKind::Resistor { r } => {
                    self.stamp_g(&mut a, &c.pins[0], &c.pins[1], 1.0 / r);
                }
                ComponentKind::Capacitor { ic, .. } => {
                    let k = n + self.sources.len() + cap_index;
                    self.stamp_source_row_real(&mut a, &mut b, c, k, *ic);
                    cap_index += 1;
                }
                ComponentKind::Inductor { ic, .. } => {
                    self.add_current(&mut b, &c.pins[0], &c.pins[1], *ic);
                }
                ComponentKind::VSource(src) => {
                    if let Some(&pos) = self.source_pos.get(&c.id) {
                        let k = n + pos;
                        self.stamp_source_row_real(&mut a, &mut b, c, k, src.value(0.0));
                    }
                }
                ComponentKind::ISource(src) => {
                    self.add_current(&mut b, &c.pins[0], &c.pins[1], src.value(0.0));
                }
                ComponentKind::Switch { on, ron, roff } => {
                    let r = if *on { *ron } else { *roff };
                    self.stamp_g(&mut a, &c.pins[0], &c.pins[1], 1.0 / r);
                }
                ComponentKind::Gate(g) => {
                    let vout = self.gate_v.get(&c.id).copied().unwrap_or(0.0);
                    if let Some(out) = c.pins.last() {
                        self.stamp_norton(&mut a, &mut b, out, vout, 1.0 / g.r);
                    }
                }
                ComponentKind::McuPin => {
                    let vout = self.pin_v.get(&c.id).copied().unwrap_or(0.0);
                    if let Some(out) = c.pins.first() {
                        self.stamp_norton(&mut a, &mut b, out, vout, 1.0 / self.mcu_r);
                    }
                }
                ComponentKind::Diode(_) | ComponentKind::Led(_) => {
                    self.stamp_g(&mut a, &c.pins[0], &c.pins[1], self.opts.gmin);
                }
                ComponentKind::Ground
                | ComponentKind::Logic(_)
                | ComponentKind::Seq(_)
                | ComponentKind::Instrument(_) => {}
            }
        }
        let x = a.solve(b)?;
        let mut sol = Solution::default();
        for (name, i) in &self.index {
            sol.nodes.insert(name.clone(), x[*i]);
        }
        for (k, id) in self.sources.iter().enumerate() {
            sol.currents.insert(id.clone(), x[n + k]);
        }
        Ok(sol)
    }

    fn update_reactive_state(&mut self, sol: &Solution) {
        for c in &self.circuit.components {
            match &c.kind {
                ComponentKind::Capacitor { .. } => {
                    let v = sol.voltage(&c.pins[0], &c.pins[1]);
                    self.cap_v.insert(c.id.clone(), v);
                }
                ComponentKind::Inductor { l, .. } => {
                    let prev = self.ind_i.get(&c.id).copied().unwrap_or(0.0);
                    let v = sol.voltage(&c.pins[0], &c.pins[1]);
                    let i = prev + self.opts.dt / l * v;
                    self.ind_i.insert(c.id.clone(), i);
                }
                _ => {}
            }
        }
    }

    fn solve_point(&mut self, mode: Mode, time: f64) -> Result<Solution, String> {
        let mut sol = self.solve_linear(mode, time)?;
        for _ in 0..self.opts.settle.max(1) {
            let changed = self.update_gates(&sol) | self.update_logic(&sol);
            if !changed {
                break;
            }
            sol = self.solve_linear(mode, time)?;
        }
        if let Some(mcu) = &mut self.mcu {
            for b in &self.bindings {
                if let Some(v) = sol.nodes.get(&b.node) {
                    mcu.set_analog(&b.pin, *v);
                }
            }
        }
        Ok(sol)
    }

    fn solve_linear(&mut self, mode: Mode, time: f64) -> Result<Solution, String> {
        let n = self.nodes.len();
        let size = n + self.sources.len();
        let diode_ids: Vec<String> = self
            .circuit
            .components
            .iter()
            .filter(|c| matches!(c.kind, ComponentKind::Diode(_) | ComponentKind::Led(_)))
            .map(|c| c.id.clone())
            .collect();
        let mut states: Vec<DiodeState> = diode_ids
            .iter()
            .map(|_| DiodeState {
                g: self.opts.gmin,
                v: 0.0,
            })
            .collect();
        let mut x = vec![0.0; size];
        let mut converged = diode_ids.is_empty();
        let vt = self.thermal_voltage();
        for _ in 0..self.opts.max_iter.max(1) {
            let mut a = Matrix::zeros(size);
            let mut b = vec![0.0; size];
            for k in 0..n {
                a.add(k, k, self.opts.gmin);
            }
            self.stamp_linear(&mut a, &mut b, mode, time);
            for (k, id) in diode_ids.iter().enumerate() {
                let c = self.circuit.component(id).ok_or("diode vanished")?;
                let (i, g) = diode_terminal(states[k].v, &c.kind, vt);
                states[k].g = g;
                self.stamp_g(&mut a, &c.pins[0], &c.pins[1], g);
                let ieq = i - g * states[k].v;
                self.add_current(&mut b, &c.pins[0], &c.pins[1], ieq);
            }
            let next = a.solve(b)?;
            let mut max_delta: f64 = 0.0;
            for (k, id) in diode_ids.iter().enumerate() {
                let c = self.circuit.component(id).ok_or("diode vanished")?;
                let v = self.voltage(&next, &c.pins[0], &c.pins[1]);
                // Limit the junction step: an exponential device will otherwise
                // jump from 0 V to the whole supply and diverge on the first
                // Newton pass.
                let limited = limit_junction(v, states[k].v);
                max_delta = max_delta.max((limited - states[k].v).abs());
                states[k].v = limited;
            }
            x = next;
            if max_delta < self.opts.vntol * 10.0 {
                converged = true;
                break;
            }
        }
        if !converged && !diode_ids.is_empty() {
            return Err(format!(
                "Newton did not converge at t={time:.6e} (diodes: {})",
                diode_ids.join(", ")
            ));
        }
        for (k, id) in diode_ids.iter().enumerate() {
            self.diode_g.insert(id.clone(), states[k].g);
        }
        let mut sol = Solution {
            time,
            ..Solution::default()
        };
        for (name, i) in &self.index {
            sol.nodes.insert(name.clone(), x[*i]);
        }
        for (k, id) in self.sources.iter().enumerate() {
            sol.currents.insert(id.clone(), x[n + k]);
        }
        Ok(sol)
    }

    fn stamp_linear(&self, a: &mut Matrix, b: &mut [f64], mode: Mode, time: f64) {
        for c in &self.circuit.components {
            match &c.kind {
                ComponentKind::Resistor { r } => {
                    self.stamp_g(a, &c.pins[0], &c.pins[1], 1.0 / r);
                }
                ComponentKind::Capacitor { c: cap, .. } => {
                    if mode == Mode::Tran {
                        let g = cap / self.opts.dt;
                        self.stamp_g(a, &c.pins[0], &c.pins[1], g);
                        let vprev = self.cap_v.get(&c.id).copied().unwrap_or(0.0);
                        self.add_current(b, &c.pins[0], &c.pins[1], -g * vprev);
                    }
                }
                ComponentKind::Inductor { l, .. } => {
                    let g = if mode == Mode::Tran {
                        self.opts.dt / l
                    } else {
                        1e9
                    };
                    self.stamp_g(a, &c.pins[0], &c.pins[1], g);
                    if mode == Mode::Tran {
                        let iprev = self.ind_i.get(&c.id).copied().unwrap_or(0.0);
                        self.add_current(b, &c.pins[0], &c.pins[1], iprev);
                    }
                }
                ComponentKind::VSource(src) => {
                    if let Some(&pos) = self.source_pos.get(&c.id) {
                        let k = self.nodes.len() + pos;
                        self.stamp_source_row_real(a, b, c, k, src.value(time));
                    }
                }
                ComponentKind::ISource(src) => {
                    self.add_current(b, &c.pins[0], &c.pins[1], src.value(time));
                }
                ComponentKind::Switch { on, ron, roff } => {
                    let r = if *on { *ron } else { *roff };
                    self.stamp_g(a, &c.pins[0], &c.pins[1], 1.0 / r);
                }
                ComponentKind::Gate(g) => {
                    let vout = self.gate_v.get(&c.id).copied().unwrap_or(0.0);
                    if let Some(out) = c.pins.last() {
                        self.stamp_norton(a, b, out, vout, 1.0 / g.r);
                    }
                }
                ComponentKind::Logic(l) => {
                    // A unit can drive several outputs, so the level is kept
                    // per node.
                    for out in &c.pins[l.n_in..] {
                        let vout = self.logic_v.get(out).copied().unwrap_or(0.0);
                        self.stamp_norton(a, b, out, vout, 1.0 / l.r);
                    }
                }
                ComponentKind::Seq(s) => {
                    for out in &c.pins[s.n_in..] {
                        let vout = self.logic_v.get(out).copied().unwrap_or(0.0);
                        self.stamp_norton(a, b, out, vout, 1.0 / s.r);
                    }
                }
                ComponentKind::McuPin => {
                    let vout = self.pin_v.get(&c.id).copied().unwrap_or(0.0);
                    if let Some(out) = c.pins.first() {
                        self.stamp_norton(a, b, out, vout, 1.0 / self.mcu_r);
                    }
                }
                ComponentKind::Diode(_) | ComponentKind::Led(_) | ComponentKind::Ground => {}
                ComponentKind::Instrument(_) => {}
            }
        }
    }

    fn update_gates(&mut self, sol: &Solution) -> bool {
        let mut changed = false;
        for c in &self.circuit.components {
            let ComponentKind::Gate(g) = &c.kind else {
                continue;
            };
            let a_in = sol.node(&c.pins[0]) > g.vth;
            let b_in = if g.kind.inputs() > 1 {
                sol.node(&c.pins[1]) > g.vth
            } else {
                false
            };
            let out = g.kind.eval(a_in, b_in);
            let v = if out { g.vdd } else { 0.0 };
            let prev = self.gate_v.get(&c.id).copied().unwrap_or(0.0);
            if (prev - v).abs() > 1e-9 {
                self.gate_v.insert(c.id.clone(), v);
                changed = true;
            }
        }
        changed
    }

    /// Evaluate the logic units: combinational ones on every iteration,
    /// clocked ones when their clock edge arrives.
    fn update_logic(&mut self, sol: &Solution) -> bool {
        let mut changed = false;
        for c in &self.circuit.components {
            match &c.kind {
                ComponentKind::Logic(l) => {
                    let ins: Vec<bool> = c.pins[..l.n_in]
                        .iter()
                        .map(|p| sol.node(p) > l.vth)
                        .collect();
                    let outs = crate::ic::combinational(l.func, &ins, l.sel_bits);
                    for (i, out) in outs.iter().enumerate() {
                        let Some(node) = c.pins.get(l.n_in + i) else {
                            continue;
                        };
                        let v = if *out { l.vdd } else { 0.0 };
                        let prev = self.logic_v.get(node).copied().unwrap_or(0.0);
                        if (prev - v).abs() > 1e-9 {
                            self.logic_v.insert(node.clone(), v);
                            changed = true;
                        }
                    }
                }
                ComponentKind::Seq(s) => {
                    let ins: Vec<bool> = c.pins[..s.n_in]
                        .iter()
                        .map(|p| sol.node(p) > s.vth)
                        .collect();
                    let mut state = self
                        .seq_state
                        .get(&c.id)
                        .copied()
                        .unwrap_or_else(crate::ic::State::new);
                    let outs = crate::ic::sequential(s.kind.func(), &ins, &mut state);
                    self.seq_state.insert(c.id.clone(), state);
                    for (i, out) in outs.iter().enumerate() {
                        let Some(node) = c.pins.get(s.n_in + i) else {
                            continue;
                        };
                        let v = if *out { s.vdd } else { 0.0 };
                        let prev = self.logic_v.get(node).copied().unwrap_or(0.0);
                        if (prev - v).abs() > 1e-9 {
                            self.logic_v.insert(node.clone(), v);
                            changed = true;
                        }
                    }
                }
                _ => {}
            }
        }
        changed
    }

    fn advance_to_mcu_time(&mut self, t: f64) -> Result<(), String> {
        let Some(mcu) = &mut self.mcu else {
            return Ok(());
        };
        for _ in 0..1024 {
            if mcu.is_finished() {
                break;
            }
            let ev = mcu.next_event().unwrap_or(f64::INFINITY);
            if ev > t + 1e-12 {
                break;
            }
            let before = ev;
            mcu.run_until(t)?;
            if mcu.next_event().unwrap_or(f64::INFINITY) <= before + 1e-15 {
                return Err("virtual MCU is stuck on a zero delay".into());
            }
        }
        for b in &self.bindings {
            let v = if mcu.pin_high(&b.pin, self.mcu_vdd) {
                self.mcu_vdd
            } else {
                0.0
            };
            if let Some(ids) = self.pin_comps.get(&b.node) {
                for id in ids {
                    self.pin_v.insert(id.clone(), v);
                }
            }
        }
        Ok(())
    }

    fn thermal_voltage(&self) -> f64 {
        8.617_333_262e-5 * (self.opts.temperature + 273.15)
    }

    #[inline]
    fn idx(&self, node: &str) -> Option<usize> {
        if self.ground.contains(node) {
            None
        } else {
            self.index.get(node).copied()
        }
    }

    fn stamp_g(&self, a: &mut Matrix, n1: &str, n2: &str, g: f64) {
        match (self.idx(n1), self.idx(n2)) {
            (Some(i), Some(j)) => {
                a.add(i, i, g);
                a.add(j, j, g);
                a.add(i, j, -g);
                a.add(j, i, -g);
            }
            (Some(i), None) => a.add(i, i, g),
            (None, Some(j)) => a.add(j, j, g),
            (None, None) => {}
        }
    }

    fn add_current(&self, b: &mut [f64], from: &str, to: &str, i: f64) {
        if let Some(k) = self.idx(from) {
            b[k] -= i;
        }
        if let Some(k) = self.idx(to) {
            b[k] += i;
        }
    }

    /// Thevenin source `v` with series `conductance` to ground, as a Norton
    /// source: `b[n] += v*g`, `A[n][n] += g`.
    fn stamp_norton(&self, a: &mut Matrix, b: &mut [f64], node: &str, v: f64, g: f64) {
        if let Some(k) = self.idx(node) {
            a.add(k, k, g);
            b[k] += v * g;
        }
    }

    fn stamp_source_row_real(
        &self,
        a: &mut Matrix,
        b: &mut [f64],
        c: &Component,
        k: usize,
        value: f64,
    ) {
        if let Some(p) = self.idx(&c.pins[0]) {
            a.add(p, k, 1.0);
            a.add(k, p, 1.0);
        }
        if let Some(n) = self.idx(&c.pins[1]) {
            a.add(n, k, -1.0);
            a.add(k, n, -1.0);
        }
        b[k] = value;
    }

    fn voltage(&self, x: &[f64], a: &str, b: &str) -> f64 {
        self.idx(a).map(|i| x[i]).unwrap_or(0.0) - self.idx(b).map(|i| x[i]).unwrap_or(0.0)
    }

    // Complex helpers used by the AC sweep.
    fn stamp_c(&self, a: &mut CMatrix, n1: &str, n2: &str, y: C) {
        match (self.idx(n1), self.idx(n2)) {
            (Some(i), Some(j)) => {
                a.add(i, i, y);
                a.add(j, j, y);
                a.add(i, j, -y);
                a.add(j, i, -y);
            }
            (Some(i), None) => a.add(i, i, y),
            (None, Some(j)) => a.add(j, j, y),
            (None, None) => {}
        }
    }

    fn add_current_c(&self, b: &mut [C], from: &str, to: &str, i: C) {
        if let Some(k) = self.idx(from) {
            b[k] = b[k] - i;
        }
        if let Some(k) = self.idx(to) {
            b[k] = b[k] + i;
        }
    }

    fn stamp_norton_c(&self, a: &mut CMatrix, node: &str, g: f64) {
        if let Some(k) = self.idx(node) {
            a.add(k, k, C::new(g, 0.0));
        }
    }

    fn stamp_source_row(&self, a: &mut CMatrix, b: &mut [C], c: &Component, k: usize, value: C) {
        let one = C::ONE;
        if let Some(p) = self.idx(&c.pins[0]) {
            a.add(p, k, one);
            a.add(k, p, one);
        }
        if let Some(n) = self.idx(&c.pins[1]) {
            a.add(n, k, -one);
            a.add(k, n, -one);
        }
        b[k] = value;
    }

    fn voltage_c(&self, x: &[C], a: &str, b: &str) -> C {
        let va = self.idx(a).map(|i| x[i]).unwrap_or(C::ZERO);
        let vb = self.idx(b).map(|i| x[i]).unwrap_or(C::ZERO);
        va - vb
    }
}

fn log_frequency(f_start: f64, f_stop: f64, index: usize, points: usize) -> f64 {
    if f_start == f_stop || points <= 1 {
        return f_start;
    }
    let a = f_start.log10();
    let b = f_stop.log10();
    let t = index as f64 / (points - 1) as f64;
    10f64.powf(a + (b - a) * t)
}

/// Damped Newton update for a PN junction: at most 0.25 V per iteration and
/// never outside a sane voltage window. The exponential I(V) curve makes an
/// unclamped first step explode.
fn limit_junction(vnew: f64, vold: f64) -> f64 {
    let v = if vnew.is_finite() { vnew } else { vold };
    let step = (v.clamp(-10.0, 10.0) - vold).clamp(-0.25, 0.25);
    (vold + step).clamp(-10.0, 10.0)
}

/// Linearize a two-terminal diode (including its series resistance) at the
/// terminal voltage `v`. Returns the terminal current `i` and conductance
/// `di/dv`.
fn diode_terminal(v: f64, kind: &ComponentKind, vt: f64) -> (f64, f64) {
    let (is, n, rs) = match kind {
        ComponentKind::Diode(DiodeModel { is, n, rs }) => (*is, *n, *rs),
        ComponentKind::Led(LedModel { vf, if_nom, n, rs }) => {
            let nvt = n * vt;
            let is = *if_nom / ((vf / nvt).exp() - 1.0).max(1e-18);
            (is, *n, *rs)
        }
        _ => return (0.0, 0.0),
    };
    let nvt = (n * vt).max(1e-9);
    let rs = rs.max(0.0);
    // Solve vd + rs * i(vd) = v for the internal diode voltage.
    let mut vd = v.clamp(-5.0, 5.0);
    for _ in 0..60 {
        let (i, gd) = diode_internal(vd, is, nvt);
        let f = vd + rs * i - v;
        let df = 1.0 + rs * gd;
        let step = (f / df).clamp(-0.5, 0.5);
        vd -= step;
        if step.abs() < 1e-10 {
            break;
        }
    }
    let (i, gd) = diode_internal(vd, is, nvt);
    let g = gd / (1.0 + gd * rs).max(1e-18);
    (i, g)
}

fn diode_internal(vd: f64, is: f64, nvt: f64) -> (f64, f64) {
    // LEDs sit at 2 V or more with a large emission coefficient, so the upper
    // clamp has to leave room for `exp(vf / nvt)` without overflowing `f64`.
    let arg = (vd / nvt).clamp(-40.0, 80.0);
    let e = arg.exp();
    (is * (e - 1.0), is * e / nvt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netlist::*;

    fn divider() -> Circuit {
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
            ComponentKind::Resistor { r: 4_700.0 },
            &["out", "0"],
        ));
        c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
        c
    }

    #[test]
    fn dc_divider() {
        let c = divider();
        let mut s = Simulator::new(&c, Options::default()).unwrap();
        let sol = s.dc().unwrap();
        let expected = 12.0 * 4700.0 / 14700.0;
        assert!((sol.node("out") - expected).abs() < 1e-6, "{:?}", sol.nodes);
        let current = 12.0 / 14_700.0;
        assert!(
            (sol.currents["V1"] + current).abs() < 1e-9,
            "{:?}",
            sol.currents
        );
    }

    #[test]
    fn rc_charge_matches_analytic() {
        let mut c = Circuit::new("rc");
        c.add(Component::new(
            "V1",
            ComponentKind::VSource(Source::dc(5.0)),
            &["in", "0"],
        ));
        c.add(Component::new(
            "R1",
            ComponentKind::Resistor { r: 1_000.0 },
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
            dt: 1e-5,
            t_stop: 5e-3,
            ..Options::default()
        };
        let mut s = Simulator::new(&c, opts).unwrap();
        let tr = s.transient().unwrap();
        let tau = 1e-3;
        for i in [10, 100, 300, 500] {
            let t = tr.time[i];
            let expected = 5.0 * (1.0 - (-t / tau).exp());
            let actual = tr.series["out"][i];
            assert!(
                (actual - expected).abs() < 0.02,
                "t={t} actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn diode_clips_forward_voltage() {
        let mut c = Circuit::new("clamp");
        c.add(Component::new(
            "V1",
            ComponentKind::VSource(Source::dc(5.0)),
            &["in", "0"],
        ));
        c.add(Component::new(
            "R1",
            ComponentKind::Resistor { r: 1_000.0 },
            &["in", "out"],
        ));
        c.add(Component::new(
            "D1",
            ComponentKind::Diode(DiodeModel::default()),
            &["out", "0"],
        ));
        c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
        let mut s = Simulator::new(&c, Options::default()).unwrap();
        let sol = s.dc().unwrap();
        let v = sol.node("out");
        assert!(v > 0.3 && v < 1.0, "diode drop {v}");
    }

    #[test]
    fn ac_low_pass_cutoff() {
        let mut c = Circuit::new("lowpass");
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
            ComponentKind::Resistor { r: 1_000.0 },
            &["in", "out"],
        ));
        c.add(Component::new(
            "C1",
            ComponentKind::Capacitor { c: 1e-6, ic: 0.0 },
            &["out", "0"],
        ));
        c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
        c.probe("out", "out");
        let mut s = Simulator::new(&c, Options::default()).unwrap();
        let fc = 1.0 / (2.0 * std::f64::consts::PI * 1_000.0 * 1e-6);
        let sweep = s.ac(fc, fc, 2).unwrap();
        let mag = sweep.magnitude("out")[0];
        let expected = std::f64::consts::FRAC_1_SQRT_2;
        assert!((mag - expected).abs() < 0.01, "mag={mag}");
    }

    #[test]
    fn and_gate_truth_table() {
        let mut c = Circuit::new("and");
        c.add(Component::new(
            "V1",
            ComponentKind::VSource(Source::dc(5.0)),
            &["vdd", "0"],
        ));
        c.add(Component::new(
            "S1",
            ComponentKind::Switch {
                on: true,
                ron: 0.1,
                roff: 1e9,
            },
            &["vdd", "a"],
        ));
        c.add(Component::new(
            "S2",
            ComponentKind::Switch {
                on: true,
                ron: 0.1,
                roff: 1e9,
            },
            &["vdd", "b"],
        ));
        c.add(Component::new(
            "R1",
            ComponentKind::Resistor { r: 10_000.0 },
            &["a", "0"],
        ));
        c.add(Component::new(
            "R2",
            ComponentKind::Resistor { r: 10_000.0 },
            &["b", "0"],
        ));
        c.add(Component::new(
            "G1",
            ComponentKind::Gate(Gate::cmos(GateKind::And)),
            &["a", "b", "out"],
        ));
        c.add(Component::new("GND", ComponentKind::Ground, &["0"]));
        let mut s = Simulator::new(&c, Options::default()).unwrap();
        let sol = s.dc().unwrap();
        assert!(sol.node("out") > 3.0, "{:?}", sol.nodes);
    }
}
