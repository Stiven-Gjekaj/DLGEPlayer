"""Tests scripts/set-version.py.

Run: python3 -m unittest discover -s scripts -p "test_*.py"
"""
import importlib.util
import os
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("set_version", os.path.join(HERE, "set-version.py"))
set_version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(set_version)

PRESETS = """[preset.0.options]

application/short_version="0.0.1"
application/version = "0.0.1"
application/min_macos_version_arm64="11.00"

[preset.1.options]

version/code=7
version/name="0.0.1"
package/name="Version 0.0.1 Player"
"""


class SetVersion(unittest.TestCase):
    def test_writes_the_version_and_the_android_code(self):
        text, changed = set_version.set_version(PRESETS, "1.2.3")
        self.assertEqual(changed, 4)
        self.assertIn('application/short_version="1.2.3"', text)
        self.assertIn('application/version = "1.2.3"', text)
        self.assertIn("version/code=10203", text)
        self.assertIn('version/name="1.2.3"', text)
        # Other keys keep their values.
        self.assertIn('application/min_macos_version_arm64="11.00"', text)
        self.assertIn('package/name="Version 0.0.1 Player"', text)

    def test_a_newer_version_has_a_larger_code(self):
        self.assertLess(set_version.version_code("1.9.9"), set_version.version_code("1.10.0"))
        self.assertLess(set_version.version_code("1.99.99"), set_version.version_code("2.0.0"))

    def test_refuses_a_version_of_a_different_form(self):
        for version in ("1.0", "1.0.0-beta", "1.100.0"):
            with self.assertRaises(ValueError):
                set_version.version_code(version)


if __name__ == "__main__":
    unittest.main()
