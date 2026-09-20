import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from drv_seed_profile import profile,verify

class Profiles(unittest.TestCase):
    def test_retained_profiles_and_selection(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        cases=[('pdirect_689_80_recovery30s.txt',(5000,476,834),dict(required=True)),
               ('seed450_691_90_recovery30s.txt',(4445,476,741),dict(seed450=True)),
               ('seed500_693_90_recovery30s.txt',(4000,476,667),dict(seed500=True))]
        for name,expected,selection in cases:
            text=(root/name).read_text()
            self.assertEqual(profile(text),expected)
            verify(text,**selection)
            with self.assertRaises(ValueError):verify(text)
            marker=next(line for line in text.splitlines() if line.startswith('SEEDPROFILE '))
            with self.assertRaises(ValueError):profile(text+'\n'+marker+'\n')
