# Intermediate 48 kHz live-duty geometry fix (retired)

ELF SHA256 `CA8EA6826F97AAFB07AE051B37F8D500B04C01A9D20202311F2CC33D6E297492`.
This image added valid1333-tick admission to `live_duty::Prepared::new`
and the disabled live-duty hardware check. Host policy tests4/4 and
disabled `livedutycheck`24/24 passed. A 10%/15s protected gate passed.
The first live command still stopped HostAbort with no transaction
veto; the later classified image showed that a burst UART request can
return an active RX error0x300. This intermediate image supplied no
48k upper-duty evidence and is retired. See
`captures/reference/reverse_48k_abort_class_20260919/README.md`.
