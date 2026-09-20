# Intermediate 48 kHz inactive-host-abort probe (retired)

ELF SHA256 `FE95F672241E3EA636327FF472ED4E70351B8D75EB913D53AE0E525D4ADC52BD`.
This predates the1333-tick live-duty fix. It recorded only bytes
received while powered owner or real IRQ was inactive. A first live
command caused HostAbort with `HOSTABORT observed=0`; that result
excludes only the probe's inactive branch. Source later identified
the missing1333 geometry. Do not infer that all host-abort causes
were absent. Image retired; no upper-duty evidence.
