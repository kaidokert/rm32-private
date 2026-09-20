import unittest
from pathlib import Path
from drv_sustained_report import summarize

ROOT=Path(__file__).resolve().parents[1]/'captures'

class SustainedReportTests(unittest.TestCase):
    def test_range350_synthetic_metadata_requires_matching_profile(self):
        # Protocol mutation in memory only; NOT hardware evidence.
        text=(ROOT/'range345_start61_reentry70_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=2899 event_min_us=241','cycle_min_us=2858 event_min_us=238')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=5798','cycle_min_ticks=5716').replace('individual_min_ticks=482','individual_min_ticks=476')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=238','event_min_us=237'))

    def test_range345_synthetic_metadata_requires_matching_profile(self):
        # In-memory protocol mutation, not hardware evidence.
        text=(ROOT/'range340_start61_reentry70_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=2942 event_min_us=245','cycle_min_us=2899 event_min_us=241')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=5884','cycle_min_ticks=5798').replace('individual_min_ticks=490','individual_min_ticks=482')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=241','event_min_us=240'))

    def test_range340_synthetic_metadata_requires_matching_profile(self):
        # In-memory protocol fixture only, never a hardware qualification.
        text=(ROOT/'range335_start61_reentry69_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=2986 event_min_us=248','cycle_min_us=2942 event_min_us=245')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=5972','cycle_min_ticks=5884').replace('individual_min_ticks=496','individual_min_ticks=490')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=245','event_min_us=244'))

    def test_range335_synthetic_metadata_requires_matching_profile(self):
        # Parser test only: edited text is NOT a new hardware capture.
        text=(ROOT/'quietstamp_start61_reentry68_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=3031 event_min_us=252','cycle_min_us=2986 event_min_us=248')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=6062','cycle_min_ticks=5972').replace('individual_min_ticks=504','individual_min_ticks=496')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=248','event_min_us=247'))

    def test_range330_synthetic_metadata_requires_matching_profile(self):
        # Protocol mutation only, not hardware evidence at the new boundary.
        text=(ROOT/'seedlean_reentry66_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=3125 event_min_us=260','cycle_min_us=3031 event_min_us=252')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=6250','cycle_min_ticks=6062').replace('individual_min_ticks=520','individual_min_ticks=504')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=252','event_min_us=251'))

    def test_range320_synthetic_metadata_requires_matching_profile(self):
        text=(ROOT/'peerexti_reentry64_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=3226 event_min_us=268','cycle_min_us=3125 event_min_us=260')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=6452','cycle_min_ticks=6250').replace('individual_min_ticks=536','individual_min_ticks=520')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=260','event_min_us=259'))

    def test_range310_synthetic_metadata_requires_matching_recovery_profile(self):
        # Relabeled existing evidence tests protocol only, NOT a310 hardware run.
        text=(ROOT/'fast_start_reentry53_30s_01.txt').read_text()
        text=text.replace('cycle_min_us=3333 event_min_us=277','cycle_min_us=3226 event_min_us=268')
        self.assertEqual(summarize(text)['reentry_verified'],0)
        text=text.replace('cycle_min_ticks=6666','cycle_min_ticks=6452').replace('individual_min_ticks=554','individual_min_ticks=536')
        self.assertEqual(summarize(text)['reentry_verified'],1)
        with self.assertRaises(ValueError):summarize(text.replace('event_min_us=268','event_min_us=267'))

    def test_faster_recovery_requires_profile_cycle_and_original_deadline(self):
        text=(ROOT/'range300_fastreturn50_01.txt').read_text()
        row=summarize(text)
        self.assertEqual(row['reentry_verified'],1)
        self.assertEqual(row['outcome'],'powered_window_complete')
        for changed in [text.replace('cycle_min_ticks=6666','cycle_min_ticks=8000'),
                        text.replace('individual_min_ticks=554','individual_min_ticks=500'),
                        text.replace('final_elapsed_us=34712662','final_elapsed_us=34712805'),
                        text.replace('RUNLIMIT ','MISSINGLIMIT ')]:
            self.assertEqual(summarize(changed)['reentry_verified'],0)

    def test_faster_acquisition_does_not_certify_late_handoff(self):
        text=(ROOT/'range300_seed50_01.txt').read_text()
        row=summarize(text)
        self.assertEqual(row['recovery_cycles_verified'],1)
        self.assertEqual(row['reentry_verified'],0)
        self.assertEqual(row['outcome'],'powered_stopped')
        for changed in [text.replace('cycle_min_ticks=6666','cycle_min_ticks=6000'),
                        text.replace('individual_min_ticks=554','individual_min_ticks=500'),
                        text.replace('min_ticks=7456','min_ticks=6665'),
                        text.replace('RUNLIMIT ','MISSINGLIMIT ')]:
            self.assertEqual(summarize(changed)['recovery_cycles_verified'],0)

    def test_experimental_running_guard_is_explicit(self):
        text=(ROOT/'range300_46_01.txt').read_text()
        row=summarize(text)
        self.assertEqual(row['run_cycle_min_us'],'3333')
        self.assertEqual(row['experimental_running_guard'],'1')
        self.assertEqual(row['outcome'],'powered_window_complete')
        with self.assertRaises(ValueError): summarize(text.replace('event_max_us=1000','event_max_us=2000'))

    def test_post_stop_acceptance_is_not_a_clean_recovery_certificate(self):
        row=summarize((ROOT/'reentry_range_45_01.txt').read_text())
        self.assertEqual(row['post_stop_tail_records'],1)
        self.assertEqual(row['outcome'],'powered_stopped')
        self.assertEqual(row['reentry_verified'],0)

    def test_cycle_qualified_recovery_requires_cycle_and_sample_evidence(self):
        text=(ROOT/'reentry45_confirm_01.txt').read_text()
        self.assertEqual(summarize(text)['reentry_verified'],1)
        self.assertEqual(summarize(text)['recovery_cycles_verified'],1)
        for changed in (text.replace('RECOVERYCYCLE checked=7','RECOVERYCYCLE checked=6'),
                        text.replace('RECOVERYCYCLE checked=7','MISSINGCYCLE checked=7'),
                        text.replace('min_ticks=8468','min_ticks=7999'),
                        text.replace('max_ticks=8664','max_ticks=12001'),
                        text.replace('rejected_ticks=0','rejected_ticks=7920'),
                        text.replace('intervals=12 max_gap_ticks=134','intervals=12 max_gap_ticks=201'),
                        text.replace('elapsed_us=9056','elapsed_us=20001'),
                        text.replace('RECOVERYACQ result=1 step=1','RECOVERYACQ result=1 step=0')):
            self.assertEqual(summarize(changed)['reentry_verified'],0)

    def test_guarded_reentry_keeps_first_segment_and_original_deadline(self):
        text=(ROOT/'reentry_power_01.txt').read_text()
        row=summarize(text)
        self.assertEqual(row['reentry_verified'],1)
        self.assertEqual(row['recovery_passive_seed_verified'],0)
        for modified in (text.replace('original_end_elapsed_us=14712860','original_end_elapsed_us=24712860'),
                         text.replace('final_elapsed_us=14712766','final_elapsed_us=14712861'),
                         text.replace('FIRSTSEG fault=8','FIRSTSEG fault=5'),
                         text.replace('attempts_max=1','attempts_max=2'),
                         text.replace('untouched=1832','untouched=128'),
                         text.replace('STACK span=','NO_STACK span=')):
            self.assertEqual(summarize(modified)['reentry_verified'],0)

    def test_passive_reacquisition_is_not_powered_recovery(self):
        text=(ROOT/'dropout_reacq_01.txt').read_text()
        row=summarize(text)
        self.assertEqual(row['outcome'],'powered_stopped')
        self.assertEqual(row['recovery_passive_seed_verified'],1)
        self.assertEqual(row['recovery_seed_ticks'],'1645')
        for modified in (text.replace('intervals=12 max_gap_ticks','intervals=11 max_gap_ticks'),
                         text.replace('disabled=1 gate_authority=0','disabled=0 gate_authority=0'),
                         text.replace('result=1 step=2 interval_ticks=1645','result=5 step=2 interval_ticks=1645')):
            self.assertEqual(summarize(modified)['recovery_passive_seed_verified'],0)

    def test_injected_dropout_is_safe_stop_not_completed_hold(self):
        text=(ROOT/'dropout_stop_01.txt').read_text()
        result=summarize(text)
        self.assertEqual(result['outcome'],'powered_stopped')
        self.assertEqual(result['dropout_stop_verified'],1)
        self.assertEqual(result['dropout_to_disabled_observation_us'],979)
        for changed in (text.replace('reason=8 stop_us=','reason=5 stop_us='),
                        text.replace('automatic_restart=0','automatic_restart=1'),
                        text.replace('end_us=2001045','end_us=2002000')):
            self.assertEqual(summarize(changed)['dropout_stop_verified'],0)

    def test_minute_capture_preserves_counts_above_u16(self):
        result=summarize((ROOT/'sustain40_60s_01.txt').read_text())
        self.assertEqual(result['outcome'],'powered_window_complete')
        self.assertEqual(int(result['commutations']),74368)
        self.assertEqual(int(result['accepted_events']),74367)
        self.assertEqual(int(result['omitted_events']),74303)
        self.assertEqual(int(result['requested_us']),60000000)

    def test_deferred_polls_are_distinct_from_missing_legacy_metric(self):
        text=(ROOT/'acq_pending_count_01.txt').read_text()
        self.assertEqual(summarize(text)['acquisition_deferred_polls'],'0')
        self.assertEqual(summarize(text.replace('FLYWAIT polls=0','FLYWAIT polls=3'))[
            'acquisition_deferred_polls'],'3')
        legacy=(ROOT/'sustain40_stepped_sparse_01.txt').read_text()
        self.assertEqual(summarize(legacy)['acquisition_deferred_polls'],'')
        measured=summarize((ROOT/'acq_repeat_03.txt').read_text())
        self.assertEqual(measured['acquisition_deferred_polls'],'2')
        self.assertEqual(measured['outcome'],'powered_window_complete')

    def test_foreground_and_independent_deadlines_both_complete(self):
        for tag in ('sustain40_stepped_sparse_01','sustain40_stepped_sparse_03'):
            with self.subTest(tag=tag):
                result=summarize((ROOT/f'{tag}.txt').read_text())
                self.assertEqual(result['outcome'],'powered_window_complete')
                self.assertGreater(float(result['accepted_rate_ehz']),200)
                self.assertLess(float(result['accepted_rate_ehz']),215)
                self.assertEqual(result['outputs_off_verified'],1)

    def test_acquisition_and_powered_failure_are_distinct(self):
        self.assertEqual(summarize((ROOT/'sustain40_stepped_sparse_02.txt').read_text())['outcome'],
                         'acquisition_refused')
        self.assertEqual(summarize((ROOT/'sustain45_200_10s_03.txt').read_text())['outcome'],
                         'powered_stopped')
        self.assertEqual(summarize((ROOT/'sustain40_200_10s_01.txt').read_text())['outcome'],
                         'startup_stopped')

    def test_premature_guard_or_active_writer_cannot_pass(self):
        text=(ROOT/'sustain40_stepped_sparse_03.txt').read_text()
        for modified in (text.replace('reason=2 stop_us=10000005','reason=8 stop_us=10000005'),
                         text.replace('stop_us=10000005','stop_us=20000'),
                         text.replace('active=0 disabled=1','active=1 disabled=0')):
            self.assertEqual(summarize(modified)['outcome'],'powered_stopped')

    def test_missing_final_safe_readback_rejects(self):
        text=(ROOT/'sustain40_stepped_sparse_01.txt').read_text()
        with self.assertRaises(RuntimeError): summarize(text[:text.rfind('OUT:')])

if __name__=='__main__': unittest.main()
