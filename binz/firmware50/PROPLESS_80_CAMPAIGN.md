# Propless 60–80% campaign — 2026-09-26

The operator transferred sole bench ownership to Codex; the previous agent is
idle. Scope is firmware50 only. The active goal is the attached propless-80
objective, not the older loaded campaign or historical AGENTS duty ceilings.

## Baseline and decisions

- Source starts at `299ba32`; firmware50 was clean on takeover.
- Existing production ELF SHA256:
  `72581795EFE2E9865E9132E18A70FEAE8F370BD1C60FE0D81A2083CE9A9F247F`
  (CRC32 `81F85AB5`), features `advance-ref,deep-filter`, release-s/thin-LTO.
- Retained propless capture: `captures/2026-09-26/noprop-open-50pct_01.txt`.
  This is prior evidence, not a new baseline or qualification by this campaign.
- Preserve loaded captures and their fixture gates. Unloaded comparisons must
  explicitly identify the different load; no prop-oracle speed/current gate.
- First batch decides whether unchanged firmware can establish gentle operation
  and then a bounded propless 60% point. No duty-ceiling change in that batch.
- Then explore 65/70/75/80, retaining all attempts and using a midpoint only to
  resolve a particular failure. Never raise a protection threshold to reach a rung.

## Safety and evidence

Serial MCP must work; no transport bypass after an MCP failure. Verify bridge-off
readback before and after each run. Confirm image identity by explicit-probe flash
when it cannot otherwise be established. Preserve original ELF before rebuilding.

Operator identifies FlyFishRC Flash 1404 4500KV. Manufacturer specifies 9N12P
(six pole pairs) and 3-4S; eHz times ten gives inferred mechanical rpm. No
explicit rated maximum rpm was found; KV times voltage is not a mechanical
speed certification. No temperature channel is established as winding
temperature. Initial exposures are finite, one at a time, with at least 60 seconds
off between exploratory runs; this is an exposure precaution, not a thermal
rating or proof. Escalate only from evidence, retaining electrical and timing
guards. Do not assume low average current proves motor thermal safety.

Motor specification source:
[FlyFishRC Flash1404](https://www.flyfish-rc.com/products/flash-1404-4500kv-fpv-motor).
The operator's “×5” identifies purchase quantity, not pole count.

Each batch has a predeclared question/prediction and a result entry. Two fresh,
independent, narrowly scoped reviews are required before adopting a hypothesis
or conclusion and before dependent work: evidence recomputation and adversarial
safety/causality. Store their text and dispositions in the append-only notebook.

Intermediate changes get targeted tests and representative regressions, not a
full ladder per hash. Preserve release/LTO, host tests/replay/clippy, structure,
four-root arithmetic audits and changed-path timing/disassembly checks.

## Completion (not yet claimed)

One reproducible final image: three predeclared >=30-second actual-target holds
at 60/70/80%, targeted intermediate/lower-point regression, 3/3 restart at 60/80%,
and demonstrated protection coverage. No foldback or sustained supply-limited
run qualifies. Deliver the unloaded curve, uncertainties and measured constraints
toward 100%. No extrapolated hardware wall or transferred loaded qualification.

E360-E362 amendment: the initial bounded baseline exposed lost foreground event
counts. Repair that instrument first; retain the old 986-per-mille identity
failure without retroactive correction. Gentle15 then50 on one candidate decide
whether the corrected counting permits meaningful propless60 characterization.

E383 checkpoint: minimum OFF cooling interval is now120seconds, superseding
the initial60second precaution above. Corrected-count lean50 has passed;
propless60 is not established. COM-top lean B reached60 for320ms then sagged;
floor6 did not clear the refusal and was retired. Compact order+sag diagnostic
BC21E369 passed50/19.778s; its sole60 request stopped on fast sag at56, with
all-off confirmed and complete traces. These diagnostic results qualify no
lean image. Source changes and failed captures are retained; review/causal
analysis is in LAB_NOTEBOOK E381-E383. No duty cap above600 has been built.
