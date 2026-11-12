//! Mixed-signal simulation for WireASM.
//!
//! The crate has five layers:
//!
//! * [`netlist`] — components, pins and JSON input;
//! * [`schematic`] — placed parts, wires and the netlist they describe;
//! * [`solver`] — MNA, Newton-Raphson, transient and AC analysis;
//! * [`mcu`] — a virtual microcontroller that runs the WireASM graph with
//!   circuit time (Proteus-style co-simulation);
//! * [`trace`] — oscilloscope traces and AC sweeps.
//!
//! Everything is dependency-free apart from `wireasm-core` (JSON and the graph
//! model), so the solver can be embedded in the IDE and in the CLI.

pub mod linalg;
pub mod mcu;
pub mod netlist;
pub mod schematic;
pub mod solver;
pub mod trace;

pub use linalg::C;
pub use mcu::VirtualMcu;
pub use netlist::{
    Circuit, Component, ComponentKind, DiodeModel, Gate, GateKind, LedModel, Probe, Source,
    Waveform,
};
pub use schematic::{
    Analysis, CircuitProbe, Part, PartKind, PinAddr, Schematic, SimResult, WaveKind, Wire,
};
pub use solver::{McuBinding, Options, Simulator, Solution};
pub use trace::{AcSweep, Trace};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
