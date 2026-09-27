Raw evidence supports one deadline-completed 15% run with 20.278s target dwell and outputs subsequently off. Tail/coast agreement supports speed consistency; it does not establish commutation correctness, efficiency, or calibrated current. The −295mA zero drift exceeds the reported 174mA mean proxy, undermining thermal inference.

I would **not yet admit 25%** on this evidence. No demonstrated defect explains a failure here, but two unresolved safety arguments matter:

- COMG transfers roles before the next native update transfers CCRs. That creates a mixed old-compare/new-role interval. Steady pad sampling after settling and geometry tests do not validate this transition. “Bounded pulsewidth” needs a transition-specific bound including startup/carrier changes and interrupted shutdown.
- Approximately 22,070RPM is already substantial. Neither a retired image’s higher speed nor 120s off establishes mechanical margin or winding/magnet temperature recovery. The 45s exposure limit lacks a demonstrated thermal basis here.

The next bounded step is disabled-bridge transition waveform verification on this exact image, plus a measured cooled baseline and a defensible mechanical speed ceiling. If those support another run, predeclare a short same-image probe with an enforced speed ceiling and temperature checks; duty and elapsed-time bounds alone do not close these risks.
