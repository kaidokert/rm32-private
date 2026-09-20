import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from freeze_minz_reference import digest, verify, verify_source


class ReferenceTests(unittest.TestCase):
    def fixture(self, root, data=b"reference", stored=b"reference"):
        source = root / "minz"
        (source / "core/src").mkdir(parents=True)
        (source / "core/src/lib.rs").write_bytes(data)
        archive = root / "reference.zip"
        manifest = {"files": [{"path": "minz/core/src/lib.rs", "bytes": len(data),
                               "sha256": digest(data)}]}
        with zipfile.ZipFile(archive, "x") as z:
            z.writestr("minz/core/src/lib.rs", stored)
            z.writestr("manifest.json", json.dumps(manifest))
        return archive, source

    def test_archive_and_live_match(self):
        with tempfile.TemporaryDirectory() as temp:
            archive, source = self.fixture(Path(temp))
            verify(archive)
            verify_source(archive, source)

    def test_corrupt_archive_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            archive, _ = self.fixture(Path(temp), stored=b"changed!!")
            with self.assertRaisesRegex(ValueError, "hash mismatch"):
                verify(archive)

    def test_live_edit_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            archive, source = self.fixture(Path(temp))
            (source / "core/src/lib.rs").write_bytes(b"edit")
            with self.assertRaisesRegex(ValueError, "live reference differs"):
                verify_source(archive, source)

    def test_live_addition_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            archive, source = self.fixture(Path(temp))
            (source / "core/src/extra.rs").write_bytes(b"extra")
            with self.assertRaisesRegex(ValueError, "membership"):
                verify_source(archive, source)
