#!/usr/bin/env python3
"""Replace the /*__DATA__*/null placeholder in an HTML template with a raw JSON
file, UTF-8 safe (PowerShell's Get-Content/Set-Content mangle multibyte chars).

Usage: python scripts/inline_json.py <template.html> <out.html> <data.json>
"""
import sys

if len(sys.argv) != 4:
    sys.exit("usage: inline_json.py <template> <out> <json>")

tpl, out, js = sys.argv[1], sys.argv[2], sys.argv[3]
html = open(tpl, encoding="utf-8").read()
data = open(js, encoding="utf-8").read()
if "/*__DATA__*/null" not in html:
    sys.exit("template missing /*__DATA__*/null placeholder")
open(out, "w", encoding="utf-8").write(html.replace("/*__DATA__*/null", data))
print(f"wrote {out} ({len(html) + len(data)} bytes)")
