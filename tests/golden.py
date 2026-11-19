"""Check reference provenance with a miniature runner and recording."""
import importlib.util
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest


class Golden(unittest.TestCase):
    def test_registration_case_file_and_measurement(self):
        spec = importlib.util.spec_from_file_location("golden", Path(__file__).resolve().parents[1] / "runner/golden.py")
        golden = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(golden)
        with TemporaryDirectory() as temp:
            root = Path(temp)
            golden.ROOT = temp
            (root / "cases").mkdir()
            cases = root / "cases/gdi.txt"
            cases.write_text("case one\nend\n")
            (root / "runner").mkdir()
            header = root / "runner/apis.h"
            header.write_text("API(gdi)\n")
            digest = golden.src_hash("gdi-32")
            directory = root / "gdi-32"
            directory.mkdir()
            (directory / "src").write_text(digest)
            (directory / "exit").write_text("0")
            self.assertIn("stdout", golden.reference_error(directory, digest))
            (directory / "stdout.txt").write_text("kikizake gdi\ncase one\ndone\n")
            self.assertIsNone(golden.reference_error(directory, digest))
            header.write_text("API(gdi)\nAPI(d3d12)\n")
            self.assertIn("stale", golden.reference_error(directory, golden.src_hash("gdi-32")))
            header.write_text("API(gdi)\n")
            cases.write_text("case two\nend\n")
            self.assertNotEqual(golden.src_hash("gdi-32"), digest)
            (directory / "exit").write_text("125")
            self.assertIn("incomplete", golden.reference_error(directory, digest))


if __name__ == "__main__":
    unittest.main()
