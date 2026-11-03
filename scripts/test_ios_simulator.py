"""Tests scripts/ios-simulator.py. Needs Xcode.

Run: python3 -m unittest discover -s scripts -p "test_*.py"
"""
import importlib.util
import os
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("ios_simulator", os.path.join(HERE, "ios-simulator.py"))
ios_simulator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ios_simulator)


def platforms(path: str) -> list[str]:
    out = subprocess.run(["vtool", "-show-build", path], check=True,
                         capture_output=True, text=True).stdout
    return [line.split()[1] for line in out.splitlines() if line.strip().startswith("platform")]


class ToSimulator(unittest.TestCase):
    def test_marks_each_object_as_simulator_code(self):
        with tempfile.TemporaryDirectory() as work:
            # A long name gives a BSD long name. An odd text length gives
            # members of different sizes.
            names = ["a_long_object_name_for_bsd.o", "b.o"]
            for index, name in enumerate(names):
                source = os.path.join(work, f"{index}.c")
                with open(source, "w") as f:
                    f.write(f"int value{index}(void) {{ return {index}; }}\n" + "//" * index)
                subprocess.run(["xcrun", "-sdk", "iphoneos", "clang", "-target", "arm64-apple-ios15.0",
                                "-c", source, "-o", os.path.join(work, name)], check=True)
            archive = os.path.join(work, "lib.a")
            subprocess.run(["xcrun", "libtool", "-static", "-o", archive,
                            *[os.path.join(work, n) for n in names]], check=True)
            self.assertEqual(platforms(os.path.join(work, names[0])), ["IOS"])

            with open(archive, "rb") as f:
                data, changed = ios_simulator.to_simulator(f.read())
            self.assertEqual(changed, 2)
            out = os.path.join(work, "out")
            os.mkdir(out)
            with open(os.path.join(out, "lib.a"), "wb") as f:
                f.write(data)
            subprocess.run(["ar", "-x", "lib.a"], cwd=out, check=True)
            for name in names:
                self.assertEqual(platforms(os.path.join(out, name)), ["IOSSIMULATOR"])

    def test_refuses_a_file_that_is_not_an_archive(self):
        with self.assertRaises(ValueError):
            ios_simulator.to_simulator(b"\xcf\xfa\xed\xfe" + bytes(60))


if __name__ == "__main__":
    unittest.main()
