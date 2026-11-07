#!/usr/bin/env python3
"""Tests of app/linux/dev.holypanda.fogata.desktop, the entry the Flatpak installs: what
desktops read from it to show, find and group Fogata's window.

    python3 -I -B tools/test_desktop_entry.py
"""

import configparser
import pathlib
import re
import sys
import unittest

sys.dont_write_bytecode = True  # no __pycache__ in the repository
ROOT = pathlib.Path(__file__).resolve().parent.parent


def application_id():
    """The id the Linux runner gives its window (app/linux/CMakeLists.txt)."""
    cmake = (ROOT / "app/linux/CMakeLists.txt").read_text()
    return re.search(r'set\(APPLICATION_ID "([^"]+)"\)', cmake).group(1)


class DesktopEntry(unittest.TestCase):
    def setUp(self):
        self.app_id = application_id()
        path = ROOT / f"app/linux/{self.app_id}.desktop"
        entry = configparser.ConfigParser(interpolation=None, strict=True)
        entry.optionxform = str  # keys are case-sensitive
        entry.read_string(path.read_text(encoding="utf-8"))
        self.entry = entry["Desktop Entry"]

    def test_the_window_is_tied_to_its_entry(self):
        # The window's id is the entry's name, its icon and its class: the dock shows
        # Fogata's icon on its window, not a generic one beside a second launcher.
        self.assertEqual(self.entry["Icon"], self.app_id)
        self.assertEqual(self.entry["StartupWMClass"], self.app_id)
        self.assertEqual(self.entry["Type"], "Application")
        self.assertEqual(self.entry["Exec"], "fogata")
        self.assertEqual(self.entry["Terminal"], "false")

    def test_one_window_and_words_to_find_it(self):
        # One window: the desktop offers no "new window" that would start a second copy.
        self.assertEqual(self.entry["SingleMainWindow"], "true")
        for key in ("Keywords", "Categories"):
            with self.subTest(key=key):
                values = self.entry[key]
                self.assertTrue(values.endswith(";"), f"{key} must end with ;")
                self.assertTrue(all(values.split(";")[:-1]), f"{key} has an empty value")
        self.assertIn("chat", self.entry["Keywords"].split(";"))
        self.assertIn("Network", self.entry["Categories"].split(";"))


if __name__ == "__main__":
    unittest.main()
