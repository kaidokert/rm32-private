import unittest
from pathlib import Path
from drv_cycle_fault import decode, context, core_snapshot, reference_cycles


class CycleFaultTests(unittest.TestCase):
    def test_fault_clock_closure_is_not_latency_or_quantization_proof(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        cases=[('range330_hold69_01.txt',3.0),
               ('binmath_reentry69_30s_01.txt',3.0),
               ('dmapeer_reentry69_30s_01.txt',2.0),
               ('compcritical_reentry69_30s_01.txt',2.5)]
        for name,residual in cases:
            with self.subTest(name=name):
                r=context((root/name).read_text())['clock_closure']
                self.assertEqual(r['guard_minus_reference_us'],residual)
                self.assertTrue(r['threshold_selected_failure'])
                self.assertFalse(r['independent_clocks'])
                self.assertFalse(r['is_isr_latency_measurement'])
                self.assertFalse(r['pwm_quantization_proven'])

    def test_bin_math_recovery_retains_resumed_cycle_failure(self):
        from drv_driven_handoff import verify, verify_bin_math
        from drv_sustained_report import summarize
        text=(Path(__file__).resolve().parents[1]/'captures/binmath_reentry69_30s_01.txt').read_text()
        verify_bin_math(text,required=True)
        r=context(text)
        self.assertEqual(r['guard']['step'],4)
        self.assertEqual(r['guard']['previous_us'],5002442)
        self.assertEqual(r['guard']['decision_us'],5005454)
        self.assertEqual(r['guard']['delta_us'],3012)
        self.assertEqual(r['reference_cycles']['previous_cycle_ticks'],6410)
        self.assertEqual(r['reference_cycles']['refused_cycle_ticks'],6018)
        self.assertEqual(r['reference_cycles']['two_cycle_mean_us'],3107.0)
        self.assertEqual(r['recorder_after_previous_guard_us'],29)
        self.assertEqual(r['outputs_off_verified'],1)
        self.assertFalse(r['cause_identified'])
        self.assertFalse(r['physical_overspeed_proven'])
        result=summarize(text)
        self.assertEqual(int(result['reentry_result']),7)
        self.assertEqual(int(result['observed_us']),5005492)
        self.assertEqual(int(result['reentry_verified']),0)
        with self.assertRaises(ValueError):
            verify(text,30000,dropout=True,reentry=True)

    def test_range330_seed_math_does_not_erase_cycle_refusal(self):
        from drv_driven_handoff import verify, verify_static_comp
        text=(Path(__file__).resolve().parents[1]/'captures/range330_hold69_01.txt').read_text()
        verify_static_comp(text,True)
        self.assertIn('RUNLIMIT cycle_min_us=3031 event_min_us=252',text)
        r=context(text)
        self.assertEqual(r['guard']['step'],1)
        self.assertEqual(r['guard']['previous_us'],418247)
        self.assertEqual(r['guard']['decision_us'],421256)
        self.assertEqual(r['guard']['delta_us'],3009)
        self.assertEqual(r['reference_cycles']['previous_cycle_ticks'],6424)
        self.assertEqual(r['reference_cycles']['refused_cycle_ticks'],6012)
        self.assertEqual(r['reference_cycles']['two_cycle_mean_us'],3109.0)
        self.assertEqual(r['outputs_off_verified'],1)
        self.assertFalse(r['physical_overspeed_proven'])
        self.assertFalse(r['cause_identified'])
        with self.assertRaises(ValueError):verify(text,10000)

    def test_static_comparator_does_not_erase_one_microsecond_refusal(self):
        from drv_driven_handoff import verify, verify_static_comp
        text=(Path(__file__).resolve().parents[1]/'captures/staticcomp_hold67_01.txt').read_text()
        verify_static_comp(text,True)
        r=context(text)
        self.assertEqual(r['guard']['decision_us'],304399)
        self.assertEqual(r['guard']['previous_us'],301275)
        self.assertEqual(r['guard']['delta_us'],3124)
        self.assertEqual(r['reference_cycles']['refused_cycle_ticks'],6243)
        self.assertEqual(r['reference_cycles']['two_cycle_mean_us'],3182.25)
        self.assertEqual(r['outputs_off_verified'],1)
        self.assertFalse(r['physical_overspeed_proven'])
        self.assertFalse(r['cause_identified'])
        with self.assertRaises(ValueError):verify(text,10000)

    def test_peer_priority_does_not_erase_real_cycle_refusal(self):
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/peerexti_hold65_01.txt').read_text()
        r=context(text)
        self.assertEqual(r['guard']['delta_us'],3211)
        self.assertEqual(r['guard']['step'],5)
        self.assertEqual(r['reference_cycles']['two_cycle_mean_us'],3320.25)
        self.assertEqual(r['outputs_off_verified'],1)
        self.assertFalse(r['physical_overspeed_proven'])
        with self.assertRaises(ValueError):verify(text,10000)

    prefix = 'RUNLIMIT cycle_min_us=3333 event_min_us=277 cycle_max_us=6000\nPOWERPATH reason=12\n'
    def test_reference_cycle_redistribution_not_mixed_clock(self):
        # One boundary shifted by100 ticks: neighboring intervals compensate.
        ticks=[1100]*11
        ticks[5]+=100;ticks[6]-=100
        tail=[dict(step=i%6+1,us=i*1000+1,reference_interval_ticks=t)
              for i,t in enumerate(ticks)]
        state=dict(step=6,polling=0,running=1,last_zc_ticks=1100,this_zc_ticks=1100)
        result=reference_cycles(tail,state)
        self.assertEqual(result['previous_cycle_ticks'],6700)
        self.assertEqual(result['refused_cycle_ticks'],6500)
        self.assertEqual(result['two_cycle_mean_us'],3300)
        self.assertFalse(result['independent_rotor_period'])
        # Recorder spacing is deliberately unrelated and cannot alter sums.
        self.assertEqual(result,reference_cycles([dict(r,us=r['us']*3) for r in tail],state))
        for changed in [dict(state,step=5),dict(state,last_zc_ticks=999),dict(state,polling=1)]:
            with self.assertRaises(ValueError):reference_cycles(tail,changed)
        self.assertIsNone(reference_cycles(tail[:10],state))
        self.assertIsNone(reference_cycles(tail,None))

    def test_actual_traceoff_reference_cycle_pair(self):
        text=(Path(__file__).resolve().parents[1]/'captures/traceoff_hold55_01.txt').read_text()
        result=context(text)['reference_cycles']
        self.assertEqual(result['refused_cycle_ticks'],6427)
        self.assertEqual(result['previous_cycle_ticks'],7058)
        self.assertEqual(result['two_cycle_mean_us'],3371.25)
    def test_controller_snapshot_provenance(self):
        text=self.prefix+'CYCLEFAULT step=3 previous_us=100 decision_us=3431 delta_us=3331 guard_timestamp=1\n'
        row='CYCLECORE step=3 rising=1 average_ticks=1130 previous_average_ticks=1132 interval_ticks=1131 this_zc_ticks=1080 last_zc_ticks=1289 wait_ticks=282 filter=12 zero_crosses=12 polling=0 running=1 after_safing=1 before_ev_acc_return=1\n'
        self.assertIsNone(core_snapshot(text))
        self.assertEqual(core_snapshot(text+row)['last_zc_ticks'],1289)
        for changed in [row+row,row.replace('step=3','step=4'),row.replace('after_safing=1','after_safing=0'),row.replace('polling=0','polling=2')]:
            with self.assertRaises(ValueError):core_snapshot(text+changed)

    def test_real_controller_snapshot_on_different_sector(self):
        from drv_accepted_events import decode_windows
        text=(Path(__file__).resolve().parents[1]/'captures/cycle_core_hold55_01.txt').read_text()
        state=core_snapshot(text)
        self.assertEqual(state['step'],1)
        self.assertEqual(state['average_ticks'],1127)
        self.assertEqual(state['polling'],0)
        tail=decode_windows(text)['tail']
        self.assertEqual(sum(r['reference_interval_ticks'] for r in tail[-5:])+state['this_zc_ticks'],6446)
        self.assertEqual(context(text)['guard']['delta_us'],3221)

    def test_range310_tail_context_keeps_guard_and_recorder_clocks_separate(self):
        path=Path(__file__).resolve().parents[1]/'captures/range310_hold55_01.txt'
        text=path.read_text()
        result=context(text)
        self.assertEqual(result['guard']['delta_us'],3222)
        self.assertEqual(result['recorder_tail_cycles_us'][3], [3429,3368,3352,3498])
        self.assertEqual(result['recorder_after_previous_guard_us'],33)
        self.assertEqual(result['outputs_off_verified'],1)
        self.assertFalse(result['cause_identified'])
        self.assertFalse(result['physical_overspeed_proven'])
        with self.assertRaises(ValueError): context(text.replace('delta_us=3222','delta_us=3498'))

    def test_exact_and_wrap(self):
        for previous in (100, 0xfffffff0):
            decision = (previous + 3330) % 2**32
            text = self.prefix + f'CYCLEFAULT step=2 previous_us={previous} decision_us={decision} delta_us=3330 guard_timestamp=1\n'
            self.assertEqual(decode(text)['delta_us'], 3330)
            with self.assertRaises(ValueError):
                decode(text.replace('delta_us=3330', 'delta_us=3329'))
            with self.assertRaises(ValueError):
                decode(text + text.splitlines()[-1] + '\n')

    def test_legacy_and_unavailable(self):
        self.assertIsNone(decode(self.prefix))
        self.assertEqual(decode(self.prefix + 'CYCLEFAULT step=0 previous_us=0 decision_us=0 delta_us=0 guard_timestamp=1\n'), {'available': False})

    def test_healthy_interval_not_a_fault(self):
        with self.assertRaises(ValueError):
            decode(self.prefix + 'CYCLEFAULT step=2 previous_us=100 decision_us=3433 delta_us=3333 guard_timestamp=1\n')

    def test_hardware_guard_is_not_recorder_minimum(self):
        path = Path(__file__).resolve().parents[1] / 'captures/cyclefault_range54_01.txt'
        text = path.read_text()
        snapshot = decode(text)
        self.assertEqual(snapshot['delta_us'], 3331)
        self.assertEqual(snapshot['decision_us'] - snapshot['previous_us'], 3331)
        self.assertIn('kind=cycle n=264 min_us=3348', text)
        with self.assertRaises(ValueError):
            decode(text.replace('delta_us=3331', 'delta_us=3348'))

    def test_fast_start_hardware_limit_is_not_a_completed_run(self):
        from drv_driven_handoff import verify
        from drv_sustained_report import summarize
        path = Path(__file__).resolve().parents[1] / 'captures/fast_start_hold54_01.txt'
        text = path.read_text()
        snapshot = decode(text)
        self.assertEqual(snapshot['delta_us'], 3331)
        self.assertEqual(snapshot['decision_us'], 689501)
        self.assertEqual(snapshot['previous_us'], 686170)
        self.assertIn('kind=cycle n=1181 min_us=3336', text)
        result = summarize(text)
        self.assertEqual(result['outcome'], 'powered_stopped')
        self.assertEqual(result['outputs_off_verified'], 1)
        with self.assertRaises(ValueError):
            verify(text, 10000)


if __name__ == '__main__':
    unittest.main()
