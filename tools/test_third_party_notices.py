#!/usr/bin/env python3
"""Tests of tools/third_party_notices.py: libwebrtc's licence and notice are read from the
copy kept in the repository, so THIRD-PARTY-NOTICES is made and checked on a computer that
never unpacked libwebrtc (a Mac), and a copy that no longer matches the archive unpacked
beside it stops the script.

    python3 -I -B tools/test_third_party_notices.py
"""

import importlib.util
import pathlib
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True  # no __pycache__ in the repository
TOOLS = pathlib.Path(__file__).resolve().parent

spec = importlib.util.spec_from_file_location("notices", TOOLS / "third_party_notices.py")
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class Libwebrtc(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(self.tmp.name)
        self.saved = notices.ROOT, notices.LIBWEBRTC_COPY, notices.LIBWEBRTC_UNPACKED
        notices.ROOT = root
        notices.LIBWEBRTC_COPY = root / "kept"
        notices.LIBWEBRTC_UNPACKED = root / "unpacked"
        notices.LIBWEBRTC_COPY.mkdir()
        (notices.LIBWEBRTC_COPY / "NOTICE.md").write_text("# libwebrtc\nnotices\n")

    def tearDown(self):
        notices.ROOT, notices.LIBWEBRTC_COPY, notices.LIBWEBRTC_UNPACKED = self.saved
        self.tmp.cleanup()

    def test_the_copy_is_read_where_libwebrtc_was_never_unpacked(self):
        self.assertEqual(notices.libwebrtc_file("NOTICE.md"),
                         notices.LIBWEBRTC_COPY / "NOTICE.md")

    def test_the_copy_is_read_when_the_archive_unpacked_says_the_same(self):
        notices.LIBWEBRTC_UNPACKED.mkdir()
        (notices.LIBWEBRTC_UNPACKED / "NOTICE.md").write_text("# libwebrtc\nnotices\n")
        self.assertEqual(notices.libwebrtc_file("NOTICE.md"),
                         notices.LIBWEBRTC_COPY / "NOTICE.md")

    def test_a_copy_that_differs_from_the_archive_stops_it(self):
        notices.LIBWEBRTC_UNPACKED.mkdir()
        (notices.LIBWEBRTC_UNPACKED / "NOTICE.md").write_text("# libwebrtc\nnew notices\n")
        with self.assertRaises(SystemExit):
            notices.libwebrtc_file("NOTICE.md")

    def test_a_missing_copy_stops_it(self):
        with self.assertRaises(SystemExit):
            notices.libwebrtc_file("LICENSE")


class Kept(unittest.TestCase):
    def test_the_repository_keeps_libwebrtcs_licence_and_notice(self):
        for name in ("LICENSE", "NOTICE.md"):
            self.assertTrue((notices.LIBWEBRTC_COPY / name).is_file(), name)


if __name__ == "__main__":
    unittest.main()
