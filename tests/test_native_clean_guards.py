"""Filesystem/profile rejection tests only. No fake Maya and no native acceptance claim."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "scripts" / "maya_standalone_clean.py"
spec = importlib.util.spec_from_file_location("native_clean", MODULE)
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class NativeProfileGuards(unittest.TestCase):
    def test_missing_profile_preserves_original_before_importing_maya(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "production.mb"
            original = b"FOR4\x00\x00\x00\x10Mayaimport vaccine"
            source.write_bytes(original)
            job = root / "job.json"
            job.write_text(json.dumps({"profile": str(root / "missing.json"),
                                       "plugin": str(root / "plugin.so"),
                                       "maya_version": "2024", "files": [str(source)]}))
            with self.assertRaises(IOError):
                native.main(str(job))
            self.assertEqual(source.read_bytes(), original)
            self.assertFalse((root / "verified-results.json").exists())

    def test_incomplete_profile_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            profile = Path(directory) / "profile.json"
            profile.write_text('{"schema_version": 1}')
            with self.assertRaisesRegex(RuntimeError, "Incomplete environment profile"):
                native.load_profile(str(profile), "plugin", "2024")

    def test_wrong_platform_is_rejected_before_loading_any_plugin(self):
        with tempfile.TemporaryDirectory() as directory:
            profile = Path(directory) / "profile.json"
            profile.write_text(json.dumps({"schema_version": 1, "maya_version": "2024",
                "api_version": 20240000, "platform": "definitely-not-this-platform",
                "mayapy_sha256": native.digest(sys.executable), "plugins": [], "dependencies": []}))
            with self.assertRaisesRegex(RuntimeError, "platform does not match"):
                native.load_profile(str(profile), "plugin", "2024")

    def test_dependency_hash_mismatch_is_read_only(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "runtime.so"
            content = b"not a Maya runtime; profile validation fixture only"
            path.write_bytes(content)
            with self.assertRaisesRegex(RuntimeError, "hash mismatch"):
                native.verify_file({"path": str(path), "sha256": "0" * 64})
            self.assertEqual(path.read_bytes(), content)
            self.assertEqual(native.digest(str(path)), hashlib.sha256(content).hexdigest())

    def test_relative_dependency_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "missing or not absolute"):
            native.verify_file({"path": "relative-runtime.so", "sha256": "0" * 64})


if __name__ == "__main__":
    unittest.main()
