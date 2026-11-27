"""Independent PST interoperability check (skipped when pypff is unavailable).

Run locally with `python -m unittest discover -s tests`. When `pypff`
(libpff-python) is installed, this test opens the committed sample PST with the
independent libyal implementation and walks the folder/message hierarchy. It
verifies that the writer's NBT, BTH property contexts and 0x7C hierarchy tables
are readable by software other than mail2rust itself.
"""
import os
import sys
import tempfile
import unittest
from pathlib import Path

try:
    import pypff
except ImportError:
    # The audit environment installs libpff-python into %TEMP%\pfflib.
    candidate = Path(tempfile.gettempdir()) / "pfflib"
    if candidate.is_dir():
        sys.path.insert(0, str(candidate))
        try:
            import pypff
        except ImportError:
            pypff = None
    else:
        pypff = None


@unittest.skipIf(pypff is None, "pypff/libpff-python not installed")
class TestPstInteroperability(unittest.TestCase):
    def test_sample_pst_walks_expected_hierarchy(self):
        sample = Path(__file__).resolve().parents[1] / "samples" / "pst" / "sample_unicode.pst"
        self.assertTrue(sample.is_file(), f"missing sample fixture: {sample}")

        pst = pypff.file()
        pst.open(str(sample))
        try:
            root = pst.get_root_folder()
            self.assertEqual(root.get_name(), "Top of Personal Folders")

            subjects = []

            def walk(folder):
                for i in range(folder.get_number_of_sub_messages()):
                    subjects.append(folder.get_sub_message(i).get_subject())
                for i in range(folder.get_number_of_sub_folders()):
                    walk(folder.get_sub_folder(i))

            walk(root)
            self.assertEqual(len(subjects), 3, f"unexpected subjects: {subjects}")
            self.assertTrue(
                any("Project Titan" in (s or "") for s in subjects),
                f"message subjects not recovered: {subjects}",
            )
        finally:
            try:
                pst.close()
            except Exception:
                pass


class TestAnsiPstExtraction(unittest.TestCase):
    """Full-fidelity extraction from a genuine 32-bit ANSI PST.

    Uses the EDRM file-format sample when it is present on this machine; skips
    otherwise so the test remains portable.
    """

    SAMPLE = Path(
        r"C:\EDRM_Data-Set_File-Formats_1-0\EDRM_Data-Set_File-Formats_1-0"
        r"\data-set\pst\sample.pst"
    )

    def test_edrm_ansi_message_roundtrip(self):
        if not self.SAMPLE.is_file():
            self.skipTest("EDRM ANSI PST sample not present")
        binary = Path(__file__).resolve().parents[1] / "target" / "debug" / "mail2rust.exe"
        if not binary.is_file():
            self.skipTest("mail2rust CLI binary not built")
        import shutil
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            result = subprocess.run(
                [str(binary), "eml", str(self.SAMPLE), str(out)],
                capture_output=True,
                text=True,
                timeout=120,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            files = list(out.rglob("*.eml"))
            self.assertEqual(len(files), 1)
            eml = files[0].read_text(errors="replace")
            self.assertIn("Subject: Updated: Olympus training for new hires", eml)
            self.assertIn("Cyndy Foulkrod", eml)
            self.assertIn("Tue, 17 Aug 2004 14:00:46 +0000", eml)
            self.assertIn("Patty will provide Olympus training", eml)
            self.assertIn("Content-Type: multipart/alternative", eml)
            self.assertIn("<!DOCTYPE HTML", eml)
            self.assertIn("Olympus training for new hires</TITLE>", eml)


class TestUnicodePstExtraction(unittest.TestCase):
    """Full-fidelity extraction from a genuine Unicode PST with real mail."""

    SAMPLE = Path(r"C:\Users\andre\Documents\Outlook Files\backup.pst")

    def test_unicode_message_with_html_body(self):
        if not self.SAMPLE.is_file():
            self.skipTest("Unicode PST sample not present")
        binary = Path(__file__).resolve().parents[1] / "target" / "debug" / "mail2rust.exe"
        if not binary.is_file():
            self.skipTest("mail2rust CLI binary not built")
        import shutil
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            result = subprocess.run(
                [str(binary), "eml", str(self.SAMPLE), str(out)],
                capture_output=True,
                text=True,
                timeout=300,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(list(out.rglob("*.eml")))
            joined = "\n".join(p.read_text(errors="replace") for p in out.rglob("*.eml"))
            self.assertIn("Subject: Try the new Windows Search experience", joined)
            self.assertIn("Windows Insider Program", joined)
            self.assertIn("Content-Type: multipart/alternative", joined)
            self.assertIn("<!DOCTYPE HTML", joined)


if __name__ == "__main__":
    unittest.main()
