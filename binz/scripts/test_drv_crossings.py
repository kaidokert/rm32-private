import unittest
from drv_crossings import sectors


def rows(values):
    return [dict(step=1,sector_us=i*600,us=100+i*600,wire_version=5,reject=0,
                 on=v,late_on=v,off=v) for i,v in enumerate(values)]


class Crossings(unittest.TestCase):
    def test_supported_bracket(self):
        r=sectors(rows([0,0,1,1]))[0]
        self.assertEqual((r['edges'],r['supported'],r['brackets']),(1,1,'600:1200:1'))
        self.assertFalse(r['complete'])
    def test_spike_not_supported(self):
        r=sectors(rows([0,0,1,0,0]))[0]
        self.assertEqual((r['edges'],r['supported']),(2,0))
    def test_rejection_breaks_adjacency(self):
        data=rows([0,0,1,1]);data[1]['reject']=1
        r=sectors(data)[0]
        self.assertEqual((r['edges'],r['supported']),(0,0))
    def test_no_cross_sector_edge(self):
        data=rows([0,0,1,1]);data[2]['step']=2;data[3]['step']=2
        result=sectors(data)
        self.assertTrue(all(r['edges']==0 for r in result))
        self.assertEqual(len(result),6)
    def test_slot_independent_validity(self):
        data=rows([0,0,1,1])
        for r in data:r.update(wire_version=6,reject=2,pwm_640=r['on'])
        result=sectors(data)
        self.assertEqual(next(r for r in result if r['signal']=='pwm_640')['supported'],1)
        self.assertEqual(result[0]['valid'],0)
        data[1]['pwm_640']=None
        self.assertEqual(next(r for r in sectors(data) if r['signal']=='pwm_640')['supported'],0)


if __name__=='__main__':unittest.main()
