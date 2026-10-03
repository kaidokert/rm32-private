#!/usr/bin/env python3
"""Interpretive text for the battery study (verdict, section notes, conclusions,
AM32 comparison). Written after reading the section tables; every number quoted
here is in a table or figure of the page. Merges into captures/char/fig/prose.json
(the definitions are kept)."""

import json
import pathlib

P_PATH = pathlib.Path(__file__).resolve().parent.parent / "captures" / "char" / "fig" / "prose.json"
P = json.load(open(P_PATH, encoding="utf-8"))

P["subtitle"] = (
    "NUCLEO-G071RB + DRV8304 board, boxed motor and prop, 3S battery at ~12.0–12.3 V rest · qualified image "
    "<code>fw50-env99-987</code> (98.7 % top, 8 A metered allowance, 9 V floor) measured through the "
    "<code>char-capture</code> image <code>65621AAE</code>: production controller and protections, plus a duty "
    "scheduler and recorders that compile to nothing in production · 2026-10-01 · 64 captures, one wire format")
P["verdict"] = [
    "<strong>Steady state: clean from 10 % to 100 %.</strong> 384 → 3267 eHz, 0.07 → 7.31 A; closed-loop speed agrees "
    "with the coast measurement within 0.1–2.4 %, and <strong>no late arm in any settled window</strong> of the 19 held rungs.",
    "<strong>Timing degrades only at the very top.</strong> Late commutations (service ≥ 5 µs past schedule) sit at 42–52 "
    "per 1k up to 85 %, then 59 / 91 / 106 at 90 / 95 / 100 %; excursions appear from 55 % and peak at 32 per 1k at 90 %; "
    "worst-sector p99 jitter steps from ~15 to ~31–35 µs at 60–65 %.",
    "<strong>On battery, the fast-sag guard ends every hard up-step within 4 ms.</strong> 14 of 15 up-steps, all six 50 / "
    "100 % restarts and both fast ramps stopped on reason 26 with the commutation intervals steady to the trip; the one "
    "20→50 % run that rode through reached 90 % in 70 ms. The guard was tuned on the PSU and is kept as qualified.",
    "<strong>Down-steps brake hard and hold, until the rotor falls below ~900 eHz.</strong> 100→50 and 80→20 % held 6/6 "
    "(t90 76–97 ms, regeneration to about −2 A); 100→10 % lost lock 3/3, and both slow ramps lost lock on the way down "
    "near 20 % duty.",
    "<strong>Low end: 8 % holds 30 s, 7 % does not.</strong> Restart from stop is the fixed 5.23 s startup plus 0.14 s to "
    "90 % at 10 %; at 50 and 100 % the step at loop close trips the sag guard (see 3).",
]
P["map"] = (
    "<p>Each rung: from the 10 % close at 10 %/s, then a 10 s hold; the commutation statistics count from 3 s after the "
    "rung is reached. Current rises as duty² to 7.31 A at 100 % — the 8 A allowance is not reached on this run (it was, "
    "at 8.06 A, in one of three qualification holds). The bus sags 1.0 V from 10 % to 100 %, about 0.14 Ω of pack and "
    "leads. Only 5 % failed: the loop lost lock within 0.3 s of ramping below the 10 % close duty.</p>"
    "<p>The jitter panel has two regimes: below 60 % every sector's p99 is at most 20 µs; from 60–65 % four sectors "
    "(2–5) move to 25–35 µs while sectors 1 and 6 stay near 10 µs. That split is also where excursions begin.</p>")
P["steps"] = (
    "<p>A step is commanded in one scan; the rotor's response is limited by the prop: 20→50 % covers 90 % in 70 ms, "
    "100→50 % in 76–85 ms, 80→20 % in 95–97 ms, all with at most 6 % overshoot. Down-steps drive current negative "
    "(regenerative braking through the complementary PWM) for about 50 ms. The failed up-step rows are not loop failures: "
    "in every one the intervals were steady right up to the trip (section 5) and the bus guard acted on the inrush alone "
    "— 3.8–4.6 A through the pack's resistance dips the bus 0.4–0.5 V, and 5 % of 12 V is 0.6 V including ripple. "
    "The 100→10 % stops are different: speed falls from 3300 to about 750 eHz in 100 ms and the loop stops receiving "
    "crossings.</p>")
P["ramps"] = (
    "<p>The production ramp is a <strong>staircase of 1 % per 500 ms</strong> (<code>ramp.rs</code>), not 1 %/50 ms; it is "
    "the slowest slew here, and the reason production never produces the steps of section 2. The two slow ramps reach "
    "100 % (peak 7.4 / 8.1 A) and both lose lock on the way down near 20 % duty; the 200 %/s ramp trips the sag guard at "
    "88 % on the way up, and the step trips it at loop close. Hysteresis is negative over the top of the range: on the "
    "down leg the rotor is faster than on the up leg at the same duty (inertia), by up to 68 eHz at the production slew "
    "and 216 eHz at 20 %/s.</p>")
P["lowend"] = (
    "<p>The low-end runs close the loop at 10 % and step down on the production staircase before a 30 s hold. 10, 9 and "
    "8 % held; 7, 6 and 5 % lost lock within 2 s of reaching the duty. The restart time is dominated by the fixed open-loop "
    "startup (sine and driven stages, 5.23 s on every run); once closed, 10 % is reached in 0.13–0.14 s.</p>")
P["stops"] = (
    "<p>28 events, one panel per kind; the table lists every one. Two mechanisms only. <strong>Fast bus sag</strong> "
    "(reason 26, 19 events): the commutation intervals are steady to the last crossing — the loop had lock and the bus "
    "guard saw the inrush. <strong>Lost lock while decelerating or below 8 %</strong> (reason 8, 9 events: the 100→10 % "
    "steps, both slow ramps, the 5 % map rung and the 7 / 6 / 5 % low-end holds): the intervals stretch steadily as the rotor slows, and crossings stop arriving at "
    "190–230 µs (about 720–880 eHz) on the ramps and the 100→10 % steps. No late arm appears in any trace; no stop was a "
    "current trip, a timing stop or a storm.</p>")
P["conclusion"] = (
    "<p><strong>Does well:</strong> holds lock over the whole duty range on battery; closed-loop speed agrees with coast to "
    "about 1 %; the commutation chain never arms late, even at 3267 eHz where a sector lasts 51 µs; current is calibrated "
    "and the 8 A allowance is the binding top-end limit; active braking gives sub-100 ms down-steps that hold.</p>"
    "<p><strong>Where it is limited:</strong> (1) the fast-sag guard, tuned on a folding PSU, makes the firmware unable to "
    "accept a hard throttle step on a battery — the first thing a flight controller would ask of it; (2) deceleration "
    "into the low-speed region loses lock below roughly 900 eHz and ends the run instead of recovering; (3) timing "
    "quality softens above 90 % (late services doubled, excursions); (4) the 5.2 s startup.</p>")
P["am32"] = (
    "<p>AM32 behaviour from the local tree <code>E:/m/robot/esc/AM32</code>, default eeprom image (G071):</p><ul>"
    "<li><strong>Throttle slew:</strong> AM32 limits the duty change per 20 kHz tick to 2 / 6 / 16 %/ms (startup / low rpm "
    "/ otherwise; <code>main.c:1899-1919</code>), up and down alike. firmware50's production ramp is 1 % per 500 ms, about "
    "8000× slower at speed, so a step like those of section 2 never occurs in production; AM32 is built for them.</li>"
    "<li><strong>Bus protection:</strong> AM32 has no fast bus-sag stop; its low-voltage cut-off is off by default and, "
    "when on, waits about 10 s below 3.0 V/cell (<code>main.c:2564-2590</code>). firmware50 stops on a 5 % / 3-scan sag, "
    "the cause of every battery step stop above.</li>"
    "<li><strong>Current:</strong> AM32's current limit is off by default; when on, a PID trims the duty setpoint from a "
    "50 ms moving average (<code>main.c:831-842, 1854-1863</code>), and there is no hard over-current cut-off. firmware50 "
    "folds back and then stops on a 207 ms EWMA (8 A), with a 13 ms surge tracker.</li>"
    "<li><strong>Lost lock:</strong> AM32's desync check drops to polling mode and restarts at half the startup duty "
    "(<code>main.c:2453-2465</code>); a 22.5 ms crossing timeout also re-enters polling (<code>main.c:2664-2677</code>). "
    "firmware50 ends the run (Tracking). The deceleration losses of sections 2–3 would be a recovery on AM32 and are a "
    "stop here.</li>"
    "<li><strong>Startup and low end:</strong> AM32 starts in polling mode at 7 % and changes over once a commutation takes "
    "under 1 ms (<code>main.c:2062-2065</code>); it caps duty at 20 % at low erpm (<code>main.c:2615-2625</code>) and runs "
    "down to 2 %. firmware50 takes 5.2 s to close and holds down to 8 %. AM32 does not document a time to running.</li>"
    "<li><strong>The same:</strong> complementary PWM with active braking on deceleration; 100 % duty fully on "
    "(<code>main.c:1927</code>); the zero-cross detector and filter (transcribed in goal B); nominal advance 16/64.</li></ul>")
P["footer"] = (
    "Scripts: <code>scripts/char_run.py</code> (runner), <code>char_campaign.py</code> (every run), "
    "<code>char_parse.py</code> (one parser), <code>char_map.py</code>, <code>char_steps.py</code>, "
    "<code>char_ramps.py</code>, <code>char_lowend.py</code>, <code>char_stops.py</code>, <code>char_prose.py</code>, "
    "<code>char_study.py</code> (this page). Instruments: <code>src/charz.rs</code>, <code>bin/char-capture.rs</code>. "
    "Lab notebook ENV-100.")
json.dump(P, open(P_PATH, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print("prose written")
