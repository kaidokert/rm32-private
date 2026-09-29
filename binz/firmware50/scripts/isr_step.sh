#!/bin/bash
# Run the path tracer; on an undeclared decision, show its listing context.
L=$1; S=$2
out=$(python scripts/isr_path.py $L $S 2>&1)
echo "$out" | tail -4
a=$(echo "$out" | grep -o "at [0-9a-f]\{8\}" | head -1 | cut -c4-)
[ -n "$a" ] && python scripts/isr_ctx.py $L $a ${3:-14}
