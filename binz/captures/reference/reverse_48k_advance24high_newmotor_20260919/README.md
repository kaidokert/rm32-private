# Invalid reverse 48 kHz, attempted high-duty advance 24 A/B

Frozen `shell-pwm.elf` SHA256: `0E2D8AB9D7A0B900893790EE5A3A54D260C6EAFB788D727F07ECE1153E8D7AE6`.

Same protected reverse48k feature closure as the advance22 image, substituting the now-retired `bench-reverse-advance24-high` feature. Release-hybrid opt-s/thinLTO/codegen1 and four motor ISR-root math audit passed. Protected10% and45%/30s gates passed, but a climb toward49% fast-sag stopped at48%, before49 ACK. **Invalid A/B:** minz-core's scheduled ISR permits levels18..22; published24 silently fell back to `TEMP_ADVANCE=18`. The printed `ADVANCEPROFILE` was the published level, not effective ISR level. This capture is not evidence about effective level24. The feature was removed and the valid advance22 frozen image restored.
