"""
Pytest runner for AM32 black-box test vectors.

Usage:
    cd tests/blackbox
    python -m pytest test_vectors.py -v

Or from repo root:
    python -m pytest tests/blackbox/test_vectors.py -v
"""

import pytest
from pathlib import Path
from harness import AM32Harness, read_tuning, run_test_vectors, ticks_for

VECTORS_DIR = Path(__file__).parent / "vectors"


@pytest.fixture
def harness():
    import os
    exe = os.environ.get("AM32_HARNESS", None)
    with AM32Harness(exe_path=exe) as h:
        yield h


def get_vector_files():
    """Discover all .txt vector files.

    Fails fast when no vectors are discovered so missing test data
    (e.g. due to path typos or CI misconfiguration) cannot silently
    skip the parametrized tests.
    """
    files = sorted(VECTORS_DIR.glob("*.txt"))
    assert files, f"No vector files found in {VECTORS_DIR!s}"
    return files


# Vectors with known Rust vs C behavioral differences (pending investigation)
XFAIL_VECTORS: set[str] = set()  # No known xfails — all vectors pass


@pytest.mark.parametrize(
    "vector_file",
    get_vector_files(),
    ids=lambda p: p.stem,
)
def test_vector(harness, vector_file):
    """Run a single test vector file."""
    if vector_file.stem in XFAIL_VECTORS:
        pytest.xfail(f"Known Rust vs C difference: {vector_file.stem}")
    # Only AM32-tuning vectors have a C counterpart (design §5.8).
    if harness.is_c and read_tuning(vector_file) != "am32":
        pytest.skip("rig tuning: rm32 divergence, no AM32 counterpart")
    run_test_vectors(harness, vector_file)


# Convenience: also run direct API tests

class TestHarnessAPI:
    """Basic sanity tests for the harness itself."""

    def test_start_stop(self, harness):
        state = harness.state()
        assert state["armed"] == 0
        assert state["running"] == 0
        assert state["tick"] == 0

    def test_single_tick(self, harness):
        s1 = harness.tick()
        assert s1["tick"] == 1
        assert s1["signaltimeout"] == 1

    def test_bulk_ticks(self, harness):
        state = harness.ticks(100)
        assert state["tick"] == 100

    def test_config(self, harness):
        harness.config(armed=1)
        state = harness.state()
        assert state["armed"] == 1

    def test_reset(self, harness):
        harness.config(armed=1)
        harness.reset()
        state = harness.state()
        assert state["armed"] == 0

    def test_arm_convenience(self, harness):
        state = harness.arm()
        assert state["armed"] == 1

    def test_duration_operands(self, harness):
        """Time-based `ticks` operands convert exactly at the harness rate
        and refuse anything that is not a whole tick count."""
        assert harness.tick_hz() == 20000
        assert ticks_for("19999", 20000) == 19999
        assert ticks_for("999.95ms", 20000) == 19999
        assert ticks_for("999.95ms", 40000) == 39998
        assert ticks_for("2100ms", 20000) == ticks_for("2.1s", 20000) == 42000
        assert ticks_for("50us", 20000) == 1
        for bad in ("25us", "0ms", "1.5", "5 ms", "-1ms"):
            with pytest.raises(ValueError):
                ticks_for(bad, 20000)

    def test_tuning_selects_policy(self, harness):
        """`tuning` must really swap the policy: the rig vector's
        stay-interrupt assertion fails under AM32 tuning."""
        if harness.is_c:
            pytest.skip("C harness has no tuning command")
        rig_vector = VECTORS_DIR / "desync_detection.txt"
        assert read_tuning(rig_vector) == "rig"
        run_test_vectors(harness, rig_vector)
        with pytest.raises(AssertionError):
            run_test_vectors(harness, rig_vector, tuning="am32")
