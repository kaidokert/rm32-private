#!/bin/bash
# One climb rung: N quiet holds at APPLIED duty $1 (tenths), each followed by
# the raw hold i/b lines read from the capture (gate 2: numbers from the
# capture, not the script summary). Usage: scripts/binz_rung.sh 175 [N]
cd "$(dirname "$0")/.."
R=$1; N=${2:-3}
for i in $(seq 1 $N); do
  probe-rs reset --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430
  sleep 5
  python scripts/binz_spin.py --rung $R --hold 15 --arm 6 --quiet --walk-step ${WALK:-10} 2>&1 | grep -a -E "ABORT|stop:|hold duty"
  CAP=$(ls -t captures/binz/spin_*.txt | head -1)
  echo "-- run $i  $CAP"
  # Hold snapshot (`r` line, printed by the first query after the stop —
  # nothing is queried while the bridge drives).
  tr -d '\r' < $CAP | grep -a -E "^r duty=" | tail -1
  tr -d '\r' < $CAP | grep -a -c "indep-watchdog" | sed 's/^/   watchdog resets in capture: /'
  # Sector statistics (dumped by the stopped query) as signed means + excursions.
  tr -d '\r' < $CAP | grep -a -E "^h[1-6] |^hx " | tail -7 |     awk '/^h[1-6]/{split($3,a,"=");printf "%s:%s ", $1, a[2]} /^hx/{print " " $2, $3}'
  if tr -d '\r' < $CAP | grep -aq "BENCH KILL"; then
    tr -d '\r' < $CAP | grep -a "BENCH KILL" | head -1
    # Gate 2: classify the kill from the latched firmware counters.
    echo -n "   post-kill: "
    python scripts/binz_console.py --send i --listen 1 | tr -d '\r' | grep -a "^i step" | \
      grep -o -E "(ci|avg|zc|duty|dsy|arr)=[0-9]+" | tr '\n' ' '; echo
  fi
done
