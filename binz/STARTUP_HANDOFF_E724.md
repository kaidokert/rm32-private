# E724 — startup admission and a retained silent failure

Normal startup now has a selected40ms driven window. Forward missing epochs
start fresh windows instead of permanently poisoning later good inputs. Long
intervals are discarded only if TIM2 agrees with timestamp brackets. Missing
edges are never included in the estimate; duplicates, corrupt order/brackets,
timer disagreement and the total window still refuse. Default flying policies
retain their prior requirements. Recording arrays are bounded independently
of the operational command count, avoiding the26-command diagnostic stop.

The first40ms candidate still latched a long transient. The second collected
43 accepted events across48 commanded boundaries but ended with only4 fresh
intervals after6 reanchors. Both stopped before BEMF handoff/higher-duty ACK.
No average-current refusal or recorded per-call overrun occurred.

The normal-startup cycle candidate selects six fresh ordered intervals, one
complete checked electrical cycle, instead of two uninterrupted cycles.
Flying acquisition remains twelve. Its bounded32bit reciprocal mean is exact
over all12001 possible sums. Initial controller zero-cross count is six to
match the actual seed, not twelve fabricated observations. That powered
attempt stopped at the65th dispatch in1ms against the old64/ms quota.

The rate-report-only candidate records counts without stopping at that quota;
the independent higher-priority guard timer, current DMA, overall40ms startup
window and per-call50us refusal remain. Host policy suite55tests passed and
release-s/thinLTO plus TIM16 arithmetic audit passed. This candidate nevertheless
FAILED SILENTLY during its powered test: host timed out without COAST END, and
final UART off readback failed. No throttle ACK or verified BEMF result exists.
Longer host runtime is not evidence the motor was running or locked.

## Recovery and actual board state

SWD halt/direct-write attempt failed at0xf0000fe4 before any safing memory write
could execute. A reset-halt attempt reached reset PC080000bc, but its memory
writes also failed. No direct GPIO-clearing success is claimed. Exact previous
723d firmware was restored using checked probe-rs download followed by successful
OpenOCD reset. UART guard test then passed3timerfaults/18poststoprefusals and
actual gates/ENABLE/MOE/CCRs-off readback. Host sessions closed.

ACTUAL installed image is again
`captures/reference/nopcmp_723d/shell-pwm.elf`, SHA256
`5ef1f7f665cb1647f47de186ecef86cfb17ea943befa59d572ac9c8bcf13c257`.
Root and failed candidate `captures/reference/startrate_724d/shell-pwm.elf`:
`b2b6ce7e883c39c64c59049a7560a7b4a70bd3b1b956e3ff301eaf05a1f36606`.
Do not confuse these or flash/run the failed candidate as qualified.

Artifacts: startwindow_724_direct300.txt, startwindow_724b_direct300.txt,
startcycle_724c_direct300.txt, startrate_724d_direct300.txt,
startrate_724d_restored_guard.txt. The silent capture retains only the initial
RUN transcript and failed FINALOFF boundary, not a valid complete motor dump.
Reset recovery lost live fault state. Failure cause remains unknown; neither
hardware blame nor successful BEMF entry follows. Next is offline examination
of seed handoff, rate behavior and watchdog containment before another powered
attempt on that candidate. The30% exploration goal remains unfinished.
