#!/usr/bin/env python3
"""Assert the backtest receipt still catches the 0.2.0 defects.

The name-keyed reach table cleared every `is_available` as "reaches-boundary",
so the gate missed the headline RUSTSEC-2026-0273 capability lie while
reporting 11 other violations. This pins both halves.
"""
import json
import sys

receipt = json.load(open(sys.argv[1]))
rows = receipt["rows"]
violations = [r["fn"] for r in rows if r["verdict"] == "VIOLATION"]
cap_lies = [r["fn"] for r in rows if r["status"] == "CAPABILITY-WITHOUT-PROBE"]

for fn in ("sign", "verify", "delete", "infer", "dispatch"):
    assert fn in violations, f"{fn} must be a violation; got {violations}"
assert "is_available" in cap_lies, (
    f"is_available must be CAPABILITY-WITHOUT-PROBE; got {cap_lies}. "
    "The name-keyed gate cleared it as reaches-boundary."
)
assert receipt["violations"] >= 15, receipt["violations"]
print(f"ok: {receipt['violations']} violations, cap-lies={sorted(set(cap_lies))}")
