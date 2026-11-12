#!/usr/bin/env python3
"""Generate the example schematics for the circuit workspace.

The format matches `Schematic::to_json_value` in crates/wireasm-sim/src/schematic.rs.
"""

import json
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "examples")


def part(pid, kind, x, y, **kw):
    d = {
        "id": pid, "kind": kind, "x": x, "y": y, "value": 0, "value2": 0,
        "on": False, "gate": "and", "pin": "", "wave": "dc", "amp": 1,
        "freq": 1000, "duty": 0.5,
    }
    d.update(kw)
    return d


def unit(func, ins, outs):
    return {"func": func, "ins": ins, "outs": outs}


def wire(a, ap, b, bp):
    return {"a": {"part": a, "pin": ap}, "b": {"part": b, "pin": bp}}


def power(vcc="VCC", gnd="GND", vdd=5.0, vth=1.4):
    return {"vcc": vcc, "gnd": gnd, "vdd": vdd, "vss": 0, "vth": vth}


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
            part("V1", "vsource", -190, 0, value=12),
            part("R1", "resistor", -60, 0, value=10000),
            part("R2", "resistor", 70, 0, value=4700),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [{"part": 2, "pin": "1", "name": "out"}],
        description="12 V across 10k and 4.7k: out is 3.837 V",
    )


def rc_lowpass():
    return circuit(
        "rc_lowpass",
        [
            part("V1", "vsource", -190, 0, value=0, wave="sine", amp=1, freq=1000),
            part("R1", "resistor", -60, 0, value=1000),
            part("C1", "capacitor", 70, 0, value=1e-6),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [{"part": 1, "pin": "2", "name": "out"}],
        analysis="tran", dt=2e-5, stop=2e-3,
        description="1k + 1uF low-pass driven by a 1 kHz sine",
    )


def led():
    return circuit(
        "led",
        [
            part("V1", "vsource", -190, 0, value=5),
            part("R1", "resistor", -60, 0, value=220),
            part("LED1", "led", 70, 0, value=2.0, value2=0.02),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
        ],
        [{"part": 2, "pin": "1", "name": "anode"}],
        description="5 V through 220 ohm into a red LED",
    )


def mcu_led():
    return circuit(
        "mcu_led",
        [
            part("P1", "mcu_pin", -190, 0, pin="C13"),
            part("R1", "resistor", -60, 0, value=220),
            part("LED1", "led", 70, 0, value=2.0, value2=0.02),
            part("GND1", "ground", 70, 90),
        ],
        [
            wire(0, "p", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
        ],
        [{"part": 0, "pin": "p", "name": "mcu"}],
        analysis="tran", dt=1e-3, stop=2.0, mcu=True, vdd=3.3,
        description="Virtual MCU pin C13 through 220 ohm into an LED",
    )


def ttl_nand():
    """A 7400 wired as an inverter, with a 7474 flip-flop after it."""
    return circuit(
        "ttl_nand",
        [
            part("V1", "vsource", -230, -60, value=5),
            part("U1", "gate", -70, -60, catalog="7400", units=[
                unit("nand2", ["1A", "1B"], ["1Y"]),
                unit("nand2", ["2A", "2B"], ["2Y"]),
                unit("nand2", ["3A", "3B"], ["3Y"]),
                unit("nand2", ["4A", "4B"], ["4Y"]),
            ], power=power()),
            part("U2", "gate", 130, 40, catalog="7474", units=[
                unit("dff", ["1D", "1CLK"], ["1Q", "1Qbar"]),
                unit("dff", ["2D", "2CLK"], ["2Q", "2Qbar"]),
            ], power=power()),
            part("V2", "vsource", 130, -120, value=5, wave="sine", amp=5, freq=1000),
            part("R1", "resistor", -150, 60, value=10000),
            part("GND1", "ground", -70, 110),
        ],
        [
            wire(0, "1", 1, "1A"),
            wire(0, "2", 1, "1B"),
            wire(1, "1Y", 2, "1D"),
            wire(3, "1", 2, "1CLK"),
            wire(1, "VCC", 0, "1"),
            wire(1, "GND", 5, "0"),
            wire(2, "GND", 5, "0"),
            wire(4, "1", 1, "2A"),
            wire(4, "2", 1, "2B"),
        ],
        [{"part": 1, "pin": "1Y", "name": "n1"}],
        analysis="tran", dt=1e-4, stop=4e-3,
        description="TTL 7400 as an inverter driving a 7474 flip-flop",
    )


def ttl_counter():
    """A 74161 counter clocked by a 1 kHz source."""
    # Parts: 0 = clock, 1 = supply, 2 = counter, 3 = ground.
    return circuit(
        "ttl_counter",
        [
            part("V1", "vsource", -230, -60, value=0, wave="pulse", amp=5, freq=1000, duty=0.5),
            part("V2", "vsource", -230, 40, value=5),
            part("U1", "gate", -60, -40, catalog="74161", units=[
                unit("counter4", ["CLK", "CLR", "LD", "A", "B", "C", "D", "ENP", "ENT"],
                     ["Q0", "Q1", "Q2", "Q3", "RCO"]),
            ], power=power()),
            part("GND1", "ground", -60, 110),
        ],
        [
            wire(0, "1", 2, "CLK"),
            # The clear and load inputs are active low: hold them high.
            wire(2, "CLR", 1, "1"),
            wire(2, "LD", 1, "1"),
            wire(2, "ENP", 1, "1"),
            wire(2, "ENT", 1, "1"),
            wire(2, "GND", 3, "0"),
            # Both sources return to ground.
            wire(0, "2", 3, "0"),
            wire(1, "2", 3, "0"),
        ],
        [
            {"part": 2, "pin": "Q0", "name": "q0"},
            {"part": 2, "pin": "Q1", "name": "q1"},
            {"part": 2, "pin": "Q2", "name": "q2"},
            {"part": 2, "pin": "Q3", "name": "q3"},
        ],
        analysis="tran", dt=1e-5, stop=5e-3,
        description="TTL 74161 4-bit counter clocked at 1 kHz",
    )


def cmos_oscillator():
    """A 4093 Schmitt NAND wired as an oscillator."""
    return circuit(
        "cmos_oscillator",
        [
            part("U1", "gate", -60, 0, catalog="4093", units=[
                unit("nand2", ["1A", "1B"], ["1Y"]),
                unit("nand2", ["2A", "2B"], ["2Y"]),
                unit("nand2", ["3A", "3B"], ["3Y"]),
                unit("nand2", ["4A", "4B"], ["4Y"]),
            ], power=power(vdd=5.0, vth=2.5)),
            part("R1", "resistor", 90, -50, value=10000),
            part("C1", "capacitor", 160, -50, value=1e-9),
            part("GND1", "ground", -60, 110),
        ],
        [
            # The first gate inverts the voltage on the RC node, the second
            # one buffers it. The RC in the feedback turns the pair into a
            # relaxation oscillator.
            wire(0, "1A", 1, "2"),
            wire(0, "1B", 1, "2"),
            wire(1, "1", 0, "1Y"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2A", 0, "1Y"),
            wire(0, "2B", 0, "1Y"),
            wire(0, "GND", 3, "0"),
        ],
        [{"part": 0, "pin": "1Y", "name": "osc"}],
        analysis="tran", dt=1e-6, stop=2e-3,
        description="CMOS 4093 Schmitt NAND as an oscillator",
    )


def scope_demo():
    """An RC filter watched by an oscilloscope, the way Multisim shows it."""
    # Parts: 0 = source, 1 = resistor, 2 = capacitor, 3 = ground, 4 = scope.
    return circuit(
        "scope_demo",
        [
            part("V1", "vsource", -230, -40, value=0, wave="sine", amp=2, freq=1000),
            part("R1", "resistor", -70, -40, value=1000),
            part("C1", "capacitor", 90, -40, value=1e-6),
            part("GND1", "ground", 90, 60),
            part("XSC1", "gate", 250, -20, catalog="scope", instrument="scope"),
        ],
        [
            wire(0, "1", 1, "1"),
            wire(1, "2", 2, "1"),
            wire(2, "2", 3, "0"),
            wire(0, "2", 3, "0"),
            # The scope watches the input and the output.
            wire(4, "A", 0, "1"),
            wire(4, "B", 2, "1"),
        ],
        [{"part": 2, "pin": "1", "name": "out"}],
        analysis="tran", dt=2e-5, stop=4e-3,
        description="RC filter watched by a four channel oscilloscope",
    )


def main():
    files = {
        "circuit_divider.json": divider(),
        "circuit_rc_lowpass.json": rc_lowpass(),
        "circuit_led.json": led(),
        "circuit_mcu_led.json": mcu_led(),
        "circuit_ttl_nand.json": ttl_nand(),
        "circuit_ttl_counter.json": ttl_counter(),
        "circuit_cmos_oscillator.json": cmos_oscillator(),
        "circuit_scope_demo.json": scope_demo(),
    }
    for name, data in files.items():
        path = os.path.join(OUT, name)
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
        print("wrote", os.path.normpath(path))


if __name__ == "__main__":
    main()
