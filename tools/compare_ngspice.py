#!/usr/bin/env python3
"""Compare wireasm-sim with ngspice on the same circuits.

Both write `time value` pairs; the comparison interpolates the ngspice curve
at the wireasm sample times.
"""

import subprocess
import sys


def read_ngspice(path):
    points = []
    for line in open(path):
        parts = line.split()
        if len(parts) < 2:
            continue
        try:
            t = float(parts[0])
            v = float(parts[1])
        except ValueError:
            continue
        points.append((t, v))
    points.sort()
    return points


def read_wireasm(path):
    points = []
    for line in open(path):
        parts = line.split()
        if len(parts) < 2:
            continue
        points.append((float(parts[0]), float(parts[1])))
    return points


def interp(points, t):
    if t <= points[0][0]:
        return points[0][1]
    if t >= points[-1][0]:
        return points[-1][1]
    lo, hi = 0, len(points) - 1
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if points[mid][0] <= t:
            lo = mid
        else:
            hi = mid
    t0, v0 = points[lo]
    t1, v1 = points[hi]
    if t1 == t0:
        return v0
    return v0 + (v1 - v0) * (t - t0) / (t1 - t0)


def compare(label, ng_path, wa_path, limit, unit, step):
    ng = read_ngspice(ng_path)
    wa = read_wireasm(wa_path)
    print(f"--- {label} ---")
    print(f"ngspice точек: {len(ng)}, wireasm точек: {len(wa)}")
    worst = 0.0
    worst_t = 0.0
    for t, v in wa[:: max(1, len(wa) // step)]:
        ref = interp(ng, t)
        err = abs(v - ref)
        if err > worst:
            worst, worst_t = err, t
        print(f"  t = {t:>12.6} : wireasm {v:>11.5} {unit}, "
              f"ngspice {ref:>11.5} {unit}, разница {err:>9.5}")
    print(f"худшая разница: {worst:.5f} {unit} при t = {worst_t:.6f}")
    return worst


def main():
    ok = True
    worst = compare(
        "переходный процесс RC (ступенька)",
        "/tmp/bench/rc_step_ngspice.txt",
        "/tmp/bench/rc_step_wireasm.txt",
        0.02, "V", 12,
    )
    ok = ok and worst < 0.02

    worst = compare(
        "АЧХ RC-фильтра",
        "/tmp/bench/rc_ac_ngspice.txt",
        "/tmp/bench/rc_ac_wireasm.txt",
        0.05, "dB", 12,
    )
    ok = ok and worst < 0.05
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
