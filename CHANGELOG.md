# Changelog

All notable changes to WireASM and WireStudio are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
