import unittest
from pwm_time_occupancy import dwell


class DwellTests(unittest.TestCase):
    def test_exact_against_tick_enumeration(self):
        intervals=[0,1,99,100,101,150,299,750]
        expected=[0]*10
        for duration in intervals:
            for tick in range(duration):expected[(tick%100)//10]+=1
        self.assertEqual(dwell(intervals,period=100,bins=10),expected)
        self.assertEqual(sum(expected),sum(intervals))

    def test_uniform_time_can_have_nonuniform_counter_histogram(self):
        # Synthetic resets every150ticks. Uniform-in-time acquisition has
        # twice as many observations below counter50: that is correct dwell.
        weights=dwell([150]*100,period=100,bins=10)
        self.assertEqual(weights,[2000]*5+[1000]*5)
        value=[10]*5+[0]*5
        time_mean=sum(w*x for w,x in zip(weights,value))/sum(weights)
        flat_phase_mean=sum(value)/len(value)
        self.assertAlmostEqual(time_mean,20/3)
        self.assertEqual(flat_phase_mean,5)

    def test_invalid_inputs(self):
        for kwargs in [dict(period=0),dict(bins=3),dict(bins=0)]:
            with self.assertRaises(ValueError):dwell([10],**kwargs)
        with self.assertRaises(ValueError):dwell([-1])
