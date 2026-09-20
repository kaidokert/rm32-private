import unittest
from drv_atomic_check import verify_backend


class AtomicBackendTests(unittest.TestCase):
    def test_optional_and_both_known_backends(self):
        self.assertIsNone(verify_backend('historical capture'))
        for mode in [False,True]:
            text=f'ATOMICBACKEND single_core={int(mode)} explicit_cs_preserved=1\n'
            self.assertEqual(verify_backend(text,single_core=mode),mode)
            with self.assertRaises(ValueError):verify_backend(text,single_core=not mode)
            with self.assertRaises(ValueError):verify_backend(text+text)

    def test_required_missing_and_unknown_refused(self):
        for text in ['', 'ATOMICBACKEND single_core=2 explicit_cs_preserved=1',
                     'ATOMICBACKEND single_core=1 explicit_cs_preserved=0']:
            with self.assertRaises(ValueError):verify_backend(text,single_core=True)
