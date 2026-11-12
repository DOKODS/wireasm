#!/usr/bin/env python3
"""Generate the example schematics for the circuit workspace.

The format matches `Schematic::to_json_value` in crates/wirestudio/src/circuit.rs.
"""

import json
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "examples")


def part(pid, kind, x, y, value=0.0, value2=0.0, on=False, gate="and",
         pin="", wave="dc", amp=1.0, freq=1000.0, duty=0.5):
    return {
        "id": pid, "kind": kind, "x": x, "y": y, "value": value,
        "value2": value2, "on": on, "gate": gate, "pin": pin,
        "wave": wave, "amp": amp, "freq": freq, "duty": duty,
    }


def wire(a_part, a_pin, b_part, b_pin):
    return {"a": {"part": a_part, "pin": a_pin}, "b": {"part": b_part, "pin": b_pin}}


def probe(part_index, pin, name):
    return {"part": part_index, "pin": pin, "name": name}


def circuit(name, parts, wires, probes, analysis="dc", dt=1e-4, stop=2e-3,
            fstart=10.0, fstop=100000.0, points=40, mcu=False, vdd=3.3,
            description=""):
    return {
        "kind": "wireasm-circuit",
        "version": "1",
        "name": name,
        "description": description,
        "parts": parts,
        "wires": wires,
        "probes": probes,
        "analysis": analysis,
        "dt": dt,
        "stop": stop,
        "fstart": fstart,
        "fstop": fstop,
        "points": points,
        "mcu": mcu,
        "mcu_vdd": vdd,
    }


def divider():
    return circuit(
        "divider",
        [
            part("V1", "vsource", -190, 0, value=12.0),
            part("R1", "resistor", -60, 0, value=10000.0),
            part("R2", "resistor", 70, 0, value=4700.0),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [probe(2, "1", "out")],
        description="12 V across 10k and 4.7k: out is 3.837 V",
    )


def rc_lowpass():
    return circuit(
        "rc_lowpass",
        [
            part("V1", "vsource", -190, 0, value=0.0, wave="sine", amp=1.0, freq=1000.0),
            part("R1", "resistor", -60, 0, value=1000.0),
            part("C1", "capacitor", 70, 0, value=1e-6),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [probe(1, "2", "out")],
        analysis="tran", dt=2e-5, stop=2e-3,
        description="1k + 1uF low-pass driven by a 1 kHz sine",
    )


def led():
    return circuit(
        "led",
        [
            part("V1", "vsource", -190, 0, value=5.0),
            part("R1", "resistor", -60, 0, value=220.0),
            part("LED1", "led", 70, 0, value=2.0, value2=0.02),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [probe(2, "1", "anode")],
        description="5 V through 220 ohm into a red LED",
    )


def mcu_led():
    return circuit(
        "mcu_led",
        [
            part("P1", "mcu_pin", -190, 0, pin="C13"),
            part("R1", "resistor", -60, 0, value=220.0),
            part("LED1", "led", 70, 0, value=2.0, value2=0.02),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "p", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
        ],
        [probe(0, "p", "mcu")],
        analysis="tran", dt=1e-3, stop=2.0, mcu=True, vdd=3.3,
        description="Virtual MCU pin C13 through 220 ohm into an LED",
    )


def main():
    files = {
        "circuit_divider.json": divider(),
        "circuit_rc_lowpass.json": rc_lowpass(),
        "circuit_led.json": led(),
        "circuit_mcu_led.json": mcu_led(),
    }
    for name, data in files.items():
        path = os.path.join(OUT, name)
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
        print("wrote", os.path.normpath(path))


if __name__ == "__main__":
    main()
