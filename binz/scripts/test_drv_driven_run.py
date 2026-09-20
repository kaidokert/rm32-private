import base64
import struct
import unittest
from pathlib import Path
import zlib
from drv_driven_run import verify,records,pwm_comp,irq_observation

def row(label,values):
    raw=struct.pack('<'+'H'*len(values),*values)
    return label+' '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()

def fixture():
    # Entirely synthetic idle-level record; deliberately zero BEMF candidates.
    commands=[(i,i%6+1,i*833) for i in range(25)]
    samples=[]
    for t in range(50,20000,50):
        e=t//833;step=commands[e][1];blank=t-e*833<200
        flags=step+int(bool(step&1))*8+(2 if blank else 0)*16+256
        samples.append(row('DQ85',[t,100,flags,e]))
    adc=[row('DA85',[t,100,2048,2048,2048,11700,1500]) for t in range(100,19900,120)]
    header=('DRIVEOBS reason=2 start_us=0 stop_us=20000 theta=0 rate=858993 duty_tenths=62 first_us=833 commands=24 '
            f'reads={len(samples)} scans={len(adc)} tick_max_us=20 sector_max_us=40 late_max_us=12 age_max_us=100 '
            'candidates=0 missed=24 handoff_authority=0 disabled=1')
    return '\n'.join([header,*samples,*adc,*[row('DC85',v) for v in commands],'DRIVEOBS END','COAST END','FINALOFF',
        'OUT: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 TIM1:moe=0 ccrA=0 ccrB=0 ccrC=0',
        'IN: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 nflt=1',
        'STACK span=10200 painted=8500 untouched=8000 diagnostic_only=1'])

class DrivenRunTests(unittest.TestCase):
    def test_actual_size_profile_full_sector_seeds(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,edge,average in [('driven_seed_full60_01',21728,1605),('driven_seed_full60_02',23222,1592)]:
            text=(root/(name+'.txt')).read_text();s=verify(text)
            self.assertIn('partial_epoch_excluded=1',text)
            self.assertTrue(s['driven_seed_ready'])
            self.assertEqual((s['driven_seed_edge_tick'],s['driven_seed_average']),(edge,average))
            self.assertFalse(s['bemf_lock_proven'])
            with self.assertRaises(ValueError): verify(text.replace('partial_epoch_excluded=1','partial_epoch_excluded=2'))
    def test_size_profile_partial_epoch_gap_is_not_a_seed(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['driven_seed_s60_02','driven_seed_s60_03']:
            text=(root/(name+'.txt')).read_text();s=verify(text)
            self.assertFalse(s['driven_seed_ready']);self.assertEqual(s['driven_seed_fault'],2)
            self.assertEqual([r[0] for r in records(text,'DI85',7)],[0,*range(2,24)])
            self.assertFalse(s['bemf_lock_proven'])
    def test_actual_live_seed_and_partial_sector_refusal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        with self.assertRaises(ValueError): verify((root/'driven_seed60_01.txt').read_text())
        s=verify((root/'driven_seed60_02.txt').read_text())
        self.assertTrue(s['driven_seed_ready'])
        self.assertEqual((s['driven_seed_edge_tick'],s['driven_seed_average']),(21014,1595))
        self.assertEqual(s['irq_max_us'],33)
        self.assertFalse(s['bemf_lock_proven'])
    def test_seed_summary_cannot_refresh_last_edge(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'driven_irq60_03.txt').read_text()
        edges=records(text,'DI85',7)[:13]
        gaps=[2*(b[2]-a[2]) for a,b in zip(edges,edges[1:])]
        cycles=[2*(edges[j+6][2]-edges[j][2]) for j in range(7)]
        header='DRIVENSEED fields=ready,step,edge_lo,edge_hi,average,intervals,cycles,cycle_min,cycle_max,fault conservative_bracket_start=1 handoff_authority=0'
        values=[1,edges[-1][1],edges[-1][2]*2,0,sum(gaps)//12,12,7,min(cycles),max(cycles),0]
        good=text+'\n'+header+'\n'+row('DS85',values)
        self.assertTrue(verify(good)['driven_seed_ready'])
        for index in [2,4,5,7,9]:
            bad=values.copy();bad[index]+=1
            with self.assertRaises(ValueError): verify(good.replace(row('DS85',values),row('DS85',bad)))
    def test_actual_irq_trials_preserve_failures_and_ordered_intervals(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['driven_irq60_01','driven_irq60_02']:
            with self.assertRaises(ValueError): verify((root/(name+'.txt')).read_text())
        for name,n in [('driven_irq60_03',23),('driven_irq60_04',24)]:
            text=(root/(name+'.txt')).read_text();s=verify(text)
            self.assertEqual(s['irq_accepts'],n);self.assertFalse(s['bemf_lock_proven'])
            rows=records(text,'DI85',7)
            self.assertEqual([r[0] for r in rows],list(range(rows[0][0],25)))
            # Offline numerical eligibility only: not a live, fresh Seed.
            intervals=[r[4] for r in rows[1:13]]
            self.assertEqual(len(intervals),12)
            self.assertTrue(all(666<=v<=2000 for v in intervals))
            self.assertTrue(all(8000<=sum(intervals[j:j+6])<=12000 for j in range(7)))
    def test_irq_records_are_bracketed_not_a_seed(self):
        commands=[(0,1,0),(1,2,833),(2,3,1666)]
        header='DRIVENIRQ calls=8 accepts=2 max_us=20 rate_peak=4 rate_limit=64 fixed_average_ticks=1666 filter_reads=12 raw_inverted=1 handoff_authority=0 masked=1'
        fields='DI85FIELDS epoch,step,before_us,after_us,interval_half_us,requested_arr,previous_accept_exists'
        first=row('DI85',[0,1,500,510,1005,417,0])
        second=row('DI85',[1,2,1333,1343,1666,417,1])
        text='\n'.join([header,fields,first,second])
        self.assertEqual(irq_observation(text,commands,2500)['irq_accepts'],2)
        for replacement in ([0,1,600,610,200,417,1],[1,2,1333,1343,1200,417,1],
                            [1,2,1333,1343,1666,417,0],[1,2,1660,1670,1666,417,1]):
            with self.assertRaises(ValueError):
                irq_observation(text.replace(second,row('DI85',replacement)),commands,2500)
        for a,b in [('masked=1','masked=0'),('rate_peak=4','rate_peak=65'),('max_us=20','max_us=51')]:
            with self.assertRaises(ValueError): irq_observation(text.replace(a,b),commands,2500)
        with self.assertRaises(ValueError): irq_observation(text.replace(first,''),commands,2500)
    def test_actual_reduced_duty_and_independent_coast_brackets(self):
        from drv_coast_periods import coast_bounds
        root=Path(__file__).resolve().parents[1]
        for name,duty in [('driven_du54_01',54),('driven_du46_01',46),('driven_du46_coast_01',46)]:
            self.assertEqual(verify((root/f'captures/{name}.txt').read_text())['duty_tenths'],duty)
        text=(root/'captures/driven_du46_coast_01.txt').read_text()
        self.assertNotIn('PHASEANCHOR commanded_q32=',text)
        bounds=coast_bounds(text,100)
        self.assertEqual([p['phase'] for p in bounds],list('ABC'))
        self.assertTrue(all(len(p['periods'])>=2 for p in bounds))
        with self.assertRaises(ValueError): coast_bounds(text.replace('COASTCOMP row=0','MISSING row=0'),100)
    def test_recorded_sustained_path_also_inverts_raw(self):
        text=(Path(__file__).resolve().parents[1]/'captures/range300_fastreturn50_02.txt').read_text()
        self.assertIn('COREPOL live_raw_inverted=1 physical_raw_inverted=1',text)
    def test_new_sample_target_is_preserved_in_actual_capture(self):
        text=(Path(__file__).resolve().parents[1]/'captures/driven_pwm320_01.txt').read_text()
        self.assertEqual(verify(text)['pwm_target'],320)
        with self.assertRaises(ValueError): verify(text.replace('target=320 flags=','target=321 flags='))
        with self.assertRaises(ValueError): verify(text.replace('duty_tenths=62','duty_tenths=40'))
    def test_pwm_epoch_samples_are_not_mixed_with_off_samples(self):
        commands=[(i,i%6+1,i*833) for i in range(25)];lines=[]
        for i,(_,step,_) in enumerate(commands):
            mux=[8,6,7,8,6,7][step-1]
            lines.append(row('PD85',[i,i*8,i*8]))
            for j in range(8):
                level=bool(step&1) if j>=4 else not bool(step&1)
                value=1|(mux<<4)|(2<<8)|(int(not level)<<30)
                lines.append(row('PC85',[i*8+j,value&65535,value>>16]))
        text='PWMCOMP n=200 target=192 flags=5 stopped=1 source_comp2_csr=1 handoff_authority=0\n'+'\n'.join(lines)
        self.assertEqual(pwm_comp(text,commands)['pwm_offline_candidate_sectors'],25)
        self.assertEqual(pwm_comp(text,commands)['pwm_raw_polarity_candidate_sectors'],0)
        with self.assertRaises(ValueError): pwm_comp(text.replace('source_comp2_csr=1','source_comp2_csr=0'),commands)
        with self.assertRaises(ValueError): pwm_comp(text.replace(row('PD85',[2,16,16]),row('PD85',[2,15,16])),commands[:-1])
    def test_complete_capture_is_not_bemf_lock(self):
        s=verify(fixture())
        self.assertEqual(s['longest_ordered_candidate_edges'],0)
        self.assertFalse(s['bemf_lock_proven'])
    def test_missing_crc_record_refuses(self):
        lines=fixture().splitlines();del lines[2]
        with self.assertRaises(ValueError): verify('\n'.join(lines))
    def test_crc_corruption_refuses(self):
        with self.assertRaises(ValueError): records('DQ85 '+base64.a85encode(b'\x01'*12).decode(),'DQ85',4)
    def test_fault_or_authority_is_not_completion(self):
        for a,b in [('reason=2','reason=5'),('handoff_authority=0','handoff_authority=1'),('disabled=1','disabled=0')]:
            with self.subTest(a=a),self.assertRaises(ValueError): verify(fixture().replace(a,b))
    def test_missing_off_or_stack_refuses(self):
        for a,b in [('FINALOFF','MISSING'),('untouched=8000','untouched=511'),('nflt=1','nflt=0')]:
            with self.subTest(a=a),self.assertRaises((ValueError,RuntimeError)): verify(fixture().replace(a,b))
    def test_false_candidate_refuses(self):
        with self.assertRaises(ValueError): verify(fixture().replace('candidates=0','candidates=1'))
    def test_bad_adc_is_not_hidden_by_summary(self):
        text=fixture();before=row('DA85',[100,100,2048,2048,2048,11700,1500])
        for raw,bus in [(3249,11700),(2048,8399)]:
            with self.assertRaises(ValueError): verify(text.replace(before,row('DA85',[100,100,raw,2048,2048,bus,1500])))
    def test_timing_limits_refuse(self):
        for a,b in [('late_max_us=12','late_max_us=51'),('tick_max_us=20','tick_max_us=51'),('first_us=833','first_us=1000')]:
            with self.subTest(a=a),self.assertRaises(ValueError): verify(fixture().replace(a,b))
    def test_adc_after_reported_physical_stop_refuses(self):
        text=fixture();before=row('DA85',[19780,100,2048,2048,2048,11700,1500])
        self.assertIn(before,text)
        with self.assertRaises(ValueError): verify(text.replace(before,row('DA85',[19950,100,2048,2048,2048,11700,1500])))

if __name__=='__main__': unittest.main()
