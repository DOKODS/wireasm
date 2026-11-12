#!/usr/bin/env python3
"""Measure the solver against ngspice on identical circuits.

Run once with `scripts/bench.sh`; it builds the decks, runs both simulators
and prints the numbers that go into the README.
"""

import subprocess
import sys
import time
from pathlib import Path

BENCH = Path("/tmp/bench")
DECKS = {
    "divider": """* Divider: 12 V across 10k and 4.7k
V1 in 0 DC 12
R1 in out 10k
R2 out 0 4.7k
.control
op
print v(out)
.endc
.end
""",
    "rc_ac": """* RC low-pass, AC sweep
V1 in 0 DC 0 AC 1
R1 in out 1k
C1 out 0 1u
.control
set filetype=ascii
ac dec 50 10 100k
wrdata /tmp/bench/rc_ac_ngspice.txt vdb(out)
.endc
.end
""",
    "rc_step": """* RC low-pass, step response
V1 in 0 PULSE(0 1 0 1n 1n 1m 2m)
R1 in out 1k
C1 out 0 1u
.control
set filetype=ascii
tran 1u 2m
wrdata /tmp/bench/rc_step_ngspice.txt v(out)
.endc
.end
""",
    "long_tran": """* RC step, 200 ms window: 200 001 output points
V1 in 0 PULSE(0 1 0 1n 1n 100m 200m)
R1 in out 1k
C1 out 0 1u
.control
set noaskquit
tran 1u 200m
.endc
.end
""",
    "ladder": """* 200 resistors in a ladder, 5 V across it
V1 n0 0 DC 5
{ladder}
RL n200 0 1k
.control
op
print v(n200)
.endc
.end
""",
}


def ladder_deck():
    """200 resistors from n0 to n200, then a load to ground."""
    rows = []
    for i in range(200):
        rows.append(f"R{i + 1} n{i} n{i + 1} 1k")
    return DECKS["ladder"].format(ladder="\n".join(rows))


def run_ngspice(name):
    deck = BENCH / f"{name}.sp"
    deck.write_text(ladder_deck() if name == "ladder" else DECKS[name])
    # Two runs: the first pays for the process start, the second is the number
    # that matters.
    subprocess.run(["ngspice", "-b", str(deck)], capture_output=True, text=True)
    started = time.perf_counter()
    out = subprocess.run(
        ["ngspice", "-b", str(deck)],
        capture_output=True,
        text=True,
    )
    elapsed = time.perf_counter() - started
    return out.stdout, elapsed


def main():
    print("=== ngspice ===")
    for name in DECKS:
        out, elapsed = run_ngspice(name)
        rows = ""
        for line in out.splitlines():
            if "Data Rows" in line:
                rows = line.strip()
        for line in out.splitlines():
            if "v(" in line and "=" in line and "branch" not in line:
                print(f"{name:>10}: {line.strip()}   [{rows}, {elapsed:.3f} с]")
                break
        else:
            print(f"{name:>10}: {rows}   ({elapsed:.3f} с)")
    print()
    print("=== wireasm-sim ===")
    subprocess.run(
        [
            "cargo", "run", "--release", "-p", "wireasm-sim", "--bin", "wirebench",
        ],
        check=True,
        env={
            **dict(__import__("os").environ),
            "WIREASM_STEP_CSV": str(BENCH / "rc_step_wireasm.txt"),
            "WIREASM_AC_CSV": str(BENCH / "rc_ac_wireasm.txt"),
        },
    )
    print()
    print("=== сравнение кривых ===")
    rc = subprocess.run(
        [sys.executable, str(Path(__file__).with_name("compare_ngspice.py"))],
        capture_output=True,
        text=True,
    )
    print(rc.stdout)
    return rc.returncode


if __name__ == "__main__":
    sys.exit(main())
