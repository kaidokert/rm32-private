import unittest
from pathlib import Path
from drv_baseline_residual import report


class ResidualTests(unittest.TestCase):
    def test_real_same_dma_capture_still_has_negative_residual(self):
        text=(Path(__file__).resolve().parents[1]/'captures/prestartdma_619_hold70_10s.txt').read_text()
        result=report(text)
        self.assertTrue(result['baseline_acquisition']['dma'])
        self.assertEqual(result['scans'],49751)
        self.assertAlmostEqual(result['sum_residual_counts'],-2.1789039102229104)
        self.assertIsNone(result['amps'])

    def test_dma_metadata_is_not_calibration(self):
        # Synthetic metadata exercises parsing only, not historical DMA hardware.
        text=(Path(__file__).resolve().parents[1]/'captures/cycle450_602_hold85_30s.txt').read_text()
        marker='BASEACQ dma=1 trigger_us=201 first_us=1 channels=0,1,4,6,13 irq_consumed=0 offsets_applied=0\n'
        result=report(marker+text)
        from drv_driven_handoff import verify_prestart_dma
        verify_prestart_dma(marker+text,True)
        verify_prestart_dma(text,False)
        with self.assertRaises(ValueError):verify_prestart_dma(marker+text,False)
        with self.assertRaises(ValueError):verify_prestart_dma(text,True)
        self.assertTrue(result['baseline_acquisition']['dma'])
        self.assertIsNone(result['amps'])
        self.assertFalse(result['calibrated_current'])
        for invalid in (marker+marker,marker.replace('201','101'),marker.replace('irq_consumed=0','irq_consumed=1')):
            with self.assertRaises(ValueError):report(invalid+text)

    def test_operator_130ma_capture_does_not_become_adc_calibration(self):
        text=(Path(__file__).resolve().parents[1]/'captures/cycle450_602_hold85_30s.txt').read_text()
        result=report(text)
        self.assertEqual(result['scans'],149254)
        self.assertAlmostEqual(result['sum_residual_counts'],-5.430050555596501)
        self.assertTrue(result['initial_staging_epoch_matched'])
        self.assertIsNone(result['amps'])
        self.assertFalse(result['calibrated_current'])
        self.assertFalse(result['drift_verified'])

    def test_thirty_second_current_capture_retains_uncalibrated_status(self):
        text=(Path(__file__).resolve().parents[1]/'captures/cachedoff_current54_30s_01.txt').read_text()
        result=report(text)
        self.assertEqual(result['scans'],149253)
        self.assertAlmostEqual(result['sum_residual_counts'],-0.08830852646178045)
        self.assertIsNone(result['amps'])
        self.assertFalse(result['drift_verified'])
        self.assertFalse(result['calibrated_current'])

    def test_real_residual_not_amps(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        result=report((root/'adc_phase45_01.txt').read_text())
        self.assertAlmostEqual(result['sum_residual_counts'],-2.202245249346472)
        self.assertIsNone(result['amps']);self.assertFalse(result['calibrated_current'])
    def test_recovery_baseline_refused(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        with self.assertRaises(ValueError):report((root/'early_trigger_reentry45_01.txt').read_text())
