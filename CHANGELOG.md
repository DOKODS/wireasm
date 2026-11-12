# Changelog

All notable changes to WireASM and WireStudio are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.4.2] — 2026-11-12

### Added

- The schematic editor follows Multisim now: a light sheet with a dotted
  grid, red wires with right-angle bends, black component bodies with blue
  designators and values, node numbers on the nets and dots at the
  junctions.
- Virtual instruments stand on the sheet as components and open a front
  panel: XSC1 oscilloscope (four channels), XMM1 multimeter, XFG1 function
  generator, XBP1 Bode plotter, XLA1 logic analyzer, XWM1 wattmeter and XFC1
  frequency counter. A run opens the panels of the instruments it measured.
- An example schematic with an oscilloscope: `circuit_scope_demo`.

### Fixed

- The code editor fills the window instead of stopping at 1200 pixels, uses
  the real font metrics for the gutter and no longer wraps lines, so a
  maximised window on Windows shows the whole file with aligned numbers.
- A source file given on the command line opens in the code editor instead
  of being parsed as a graph.
- The caption of a component no longer overlaps the caption of its
  neighbour.

### Changed

- The README was rewritten: the feature list reads as a list of features
  rather than an advertisement.

## [0.4.1] — 2026-11-12

### Added

- GOST graphical symbols for every part in the schematic editor: resistors
  as a rectangle (ГОСТ 2.722), inductors as three arcs (2.723), capacitors as
  two plates (2.725), diodes and LEDs (2.727), current and voltage sources
  (2.730, 2.735), transistors (2.741), the earth symbol (2.742), switches
  (2.759), logic elements as a box with the function mark (2.610) and
  integrated circuits as a package with the pins on its edges (2.318).
- A component catalogue of 133 parts in the families of the Multisim
  component toolbar: basic parts, TTL 74xx, TTL Schottky (74LS, 74S, 74HC),
  CMOS 4000, ECL 10xxx, analog blocks and discrete devices. Every entry
  carries its pinout, its supply and the logic of its units.
- Logic integrated circuits are simulated by their units: a 7400 is four NAND
  gates, a 7474 two D flip-flops, a 74161 a counter. Clocked units keep their
  state between edges and count on the real time of the circuit.
- Import and export of component libraries: File → Import/Export component
  library… writes and reads a JSON set of parts, so a user can share their own
  elements.
- Examples: `circuit_ttl_nand`, `circuit_ttl_counter`,
  `circuit_cmos_oscillator`.

### Fixed

- A catalogue part with unconnected pins no longer stops the netlist; the
  free pins simply keep their own nets.
- The clear input of a TTL counter and shift register is active low, as in
  the real chips.

## [0.4.0] — 2026-11-12

### Added

- New crate `wireasm-sim`: a mixed-signal simulator with no external
  dependencies.
  - Netlist model with resistors, capacitors, inductors, voltage and current
    sources (DC, sine, pulse), diodes, LEDs, switches, CMOS logic gates,
    MCU pins and ground; JSON input and output.
  - Solver: nodal analysis (MNA) with a dense LU factorisation,
    Newton-Raphson with damped pn-junction limiting, backward-Euler transient
    analysis honouring initial conditions, and a complex AC sweep.
  - Virtual microcontroller: the WireASM graph itself runs as the MCU, so
    firmware drives real pins on the schematic (Proteus-style
    co-simulation), with UART capture.
  - Scope traces: peak, min, mean, RMS, rising-edge detection, CSV export.
- New `wiresim` command-line simulator: DC, transient and AC analysis of a
  netlist or of a schematic file saved by the IDE, optional `--mcu` graph for
  co-simulation, `--csv` export.
- `wirestudio`: circuit workspace (third workspace next to Graph and Code):
  component palette, schematic canvas with pin wiring, properties inspector,
  DC/transient/AC analyses, oscilloscope with selectable traces, probes,
  MCU co-simulation binding, schematic save/open, netlist export.
- WireStudio can open a schematic file from the command line and detects
  the file kind automatically.
- Examples: `circuit_divider`, `circuit_rc_lowpass`, `circuit_led`,
  `circuit_mcu_led` (schematics) plus netlists `sim_voltage_divider`,
  `sim_rc_lowpass`, `sim_halfwave`, `sim_led_resistor`, `sim_logic_and`,
  `sim_mcu_led`.
- `wirebench` binary and the `tools/` scripts that compare the solver with
  ngspice on identical circuits.
- Release archives now include the `wiresim` binary and the `examples` folder.

### Changed

- Version bumped to 0.4.0 in the workspace, the core and the CLI output.
- Diode model defaults are `is=1e-14`, `n=1.0`, `rs=0.1`; the LED clamp in
  the solver allows 2 V parts to conduct.

### Fixed

- Transient analysis starts from the initial conditions instead of taking a
  phantom first step, so an RC charge curve matches the analytic solution.
- The virtual MCU resumes after a delay node instead of re-entering it.

## [0.3.0] — 2026-11-12

### Added

- STM32 target `stm32f411` (aliases `stm32`, `stm32f4`, `cortex-m4`,
  `blackpill`): bare-metal Cortex-M4 Thumb-2 assembly with a vector table,
  reset handler, SysTick delays, USART2 output, GPIO and ADC helpers.
  Built by `arm-none-eabi-gcc -nostdlib` with a generated linker script;
  `.elf`, `.hex` and `.bin` land in `out/`.
- ESP32 and ESP8266 targets: the graph becomes an Arduino C++ sketch
  (`setup()`/`loop()`) built with `arduino-cli` and the matching core.
- Host source targets `c`, `cpp` and `csharp`: a readable `main()` with
  `goto` flow, built with the system compiler or `dotnet`.
- `wirestudio`: classic code editor workspace with tabs, line numbers,
  VS Code-like syntax highlighting, open/save, generated-file refresh, and
  build/run for the current target. Graph/Code switch in the toolbar and
  View menu.
- Target-aware pin validation: AVR 0–19, STM32 ports A–H with pins 0–15,
  ESP32 0–39, ESP8266 0–16; per-family ADC ranges and register names.
- 32-bit truncation warnings for STM32 and ESP, STM32 register validation,
  and a clear error when `Call`/`Syscall`/custom nodes are used on an MCU
  or source target.
- CLI: target-aware `--build`, `--flash` (`stlink`/`dfu` for STM32, serial
  for ESP), `--run` for C/C++/C#, and output extensions from the target.
- WireStudio: target picker, firmware build/flash jobs for every MCU,
  STM32 port inspector, board window per target, and en/ru strings for all
  new controls.
- Examples: `stm32_blink`, `stm32_uart_hello`, `esp32_blink`,
  `esp32_uart_hello`, `c_hello`, `cpp_hello`, `csharp_hello`.
- Tests for STM32 Thumb assembly, the Arduino sketch backend, and the three
  source targets.

### Fixed

- STM32 `PrintU64` now pulls in the `udiv32` helper it calls.

## [0.2.0] — 2026-11-11

### Added

- AVR target `avr-atmega328p` (aliases `avr`, `atmega328p`, `arduino-uno`,
  `arduino-nano`): 16-bit code generation for ATmega328P, `.elf/.hex/.bin`
  output, `main` entry called by avr-libc `crt0`.
- MCU nodes: `McuPinMode`, `McuDigitalWrite`, `McuDigitalRead`, `McuDelayMs`,
  `McuDelayUs`, `McuUartInit`, `McuUartWrite`, `McuUartWriteStr`, `McuAdcRead`,
  `McuRegisterWrite`, `McuRegisterRead`.
- `PrintU64`/`PrintStr` on AVR write to UART0 (auto-init at 9600 baud).
- CLI: `--build`, `--out-dir`, `--flash <port>`, `--board`, `--ports`.
- WireStudio: Microcontroller menu, MCU palette group, MCU property editors,
  board settings (port/board/folder), Build firmware (F6) and Flash (F7),
  AVR examples in the Examples menu.
- AVR validation: pin range 0–19, ADC channel, baud rate, data address,
  rejection of `Call`/`Syscall`/custom nodes, 16-bit truncation warning.
- AVR tests, `examples/avr_blink.json`, `examples/avr_uart_hello.json`.

### Changed

- `RawAsm` comments on AVR use `;`; `//` is translated when text is emitted.

## [0.1.0] — 2026-11-08

First public release.

### Added

- `wireasm-core`: graph model, JSON parser/writer, validator, and code
  generators for AArch64 and x86-64.
- Four targets: `aarch64-macos`, `aarch64-linux`, `x86_64-linux`,
  `x86_64-macos`.
- Built-in nodes: `Entry`, `Const`, `VarGet`, `VarSet`, `BinOp`, `Cmp`,
  `Branch`, `PrintU64`, `PrintStr`, `Return`, `RegGet`, `RegSet`, `Call`,
  `Syscall`, `RawAsm`, `Anchor`, `Comment`.
- Custom nodes with per-architecture assembler templates, `{in0}`..`{in7}`,
  `{a}`, `{b}`, `{out}`, `{label}`, and `{prop:key}` placeholders.
- Node libraries (`*.wirelib.json`) with import and export.
- Import of a `.s` file as a starting custom node.
- `wireasm` CLI: `--check`, `--target`, `-o`, `--run`.
- `wirestudio`: native Rust/egui IDE with palette, canvas, inspector,
  variables editor, custom-node editor, assembler/problems/console panels,
  undo/redo, and Build & Run.
- Interface localization in English and Russian, following the system locale
  with a manual override.
- Examples: hello world, loop sum, Fibonacci, raw assembly, custom nodes.
- 12 unit tests covering JSON, validation, code generation, and libraries.
- GitHub Actions CI and a tagged release workflow for Linux, Windows, and
  macOS (Intel and Apple Silicon).
