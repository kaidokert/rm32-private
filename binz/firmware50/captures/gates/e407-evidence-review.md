- **Identity:** Both captures report identical SHA256 `BF725A29…BBDF8D` and CRC32 `F9CC7F36`, with entry/run periods of 1333 ticks. At an assumed 64MHz timer clock, that is **48.012kHz**.
- **50% attempt:** CCR/period = 666/1333 = **49.9625%**. Reported target hold is **19.778s**, within **39.778s closed-loop**; reason 2 indicates deadline completion. Hold acceptance rate is 299916/19.778 = **15,164.1 events/s**, or **2527.4eHz** assuming six events/cycle. Last interval 65µs gives **2564.1eHz**; reported coast speed is **2530eHz**.
- **60% attempt:** Final applied CCR/period = 733/1333 = **54.9887%**, despite requested/ceiling fields of 600. Target hold and hold blocks are zero. Reason 26, `streak=3`, and `tripped=1` establish a sag stop before a recorded 60% hold. Last interval 54µs gives **3086.4eHz**; reported coast speed is **2775eHz**. These measure different windows.
- **Coverage:** The nominal 2s tail actually spans **3.777957s**, ending 113µs before stop; its rate implies approximately **2529eHz**. It cannot be presented as a verified final-2s statistic. The 60% zero-hold rate fields—including 166666eHz—are invalid placeholders. All six 50% sector counters saturate at 65535. At 60%, sector counts sum to **221257**, exactly accepted minus **2877 coalesced accepts**; they exclude those accepts.
- Both post-stop checks show outputs disabled and nFAULT high.

**Supported:** One 50% screen completed a reported 19.778s target hold; one 60% request sag-stopped near 55%. Setup compensation did not eliminate this observed failure.

**Unsupported:** Repeated qualification, a 30s target hold, causal attribution, physical-crossing timing margins, or calibrated peak/RMS current. Aggregate timing maxima and thin counts cannot reconstruct the failure sequence.
