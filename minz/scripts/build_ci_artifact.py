#!/usr/bin/env python3
"""Inline the temp_study JSON captures into the motor-catch24 HTML template.

Reads the given study JSONs, wraps them as a datasets array, and substitutes
them for the `/*__DATA__*/null` placeholder in the template -> final self-
contained HTML (all data inlined; no external fetch, artifact-CSP-safe).

Usage:
  python scripts/build_ci_artifact.py <template.html> <out.html> <a.json> [b.json ...]
"""
import json
import sys

if len(sys.argv) < 4:
    sys.exit("usage: build_ci_artifact.py <template> <out> <json> [json ...]")

template_path, out_path = sys.argv[1], sys.argv[2]
datasets = [json.load(open(p, encoding="utf-8")) for p in sys.argv[3:]]

with open(template_path, encoding="utf-8") as f:
    html = f.read()

blob = json.dumps({"datasets": datasets}, separators=(",", ":"))
if "/*__DATA__*/null" not in html:
    sys.exit("template missing /*__DATA__*/null placeholder")
html = html.replace("/*__DATA__*/null", blob)

with open(out_path, "w", encoding="utf-8") as f:
    f.write(html)

kb = len(html) / 1024
print(f"wrote {out_path} ({kb:.0f} KiB) with {len(datasets)} datasets: "
      + ", ".join(d.get('label', '?') for d in datasets))
