#!/usr/bin/env python3
"""Writes app/assets/emoji/<language>.tsv from Unicode CLDR's emoji annotations.

Usage: tools/emoji_names.py <folder with cldr-annotations-full and
       cldr-annotations-derived-full> [output folder, default app/assets/emoji]

The folder is the unpacked CLDR JSON release (cldr-<version>-json-full.zip from
https://github.com/unicode-org/cldr-json/releases); it is only read as data. For each
language Fogata has, a file holds the name and the search keywords of each emoji of
app/lib/src/emoji_table.dart (the picker's table, so the files stay small), one per line,
in the table's order:

    <emoji> TAB <name> TAB <keyword>|<keyword>|...

Skin tone forms are not listed: the table has only the base forms. The names go to the
picker as they are; the app folds case, accents and kana when it loads them, the same
way it folds what is typed, so what is written here stays readable.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "app/lib/src/emoji_table.dart"
# The app's language files and the CLDR locale each is made from (CLDR's "pt" is
# Brazilian Portuguese, its "zh" Simplified Chinese).
LANGUAGES = ["en", "es", "pt", "fr", "de", "it", "ru", "ja", "zh"]
VS16 = "️"


def table_emoji():
    """The emoji of the table, in its order."""
    found = []
    for line in TABLE.read_text(encoding="utf-8").splitlines():
        m = re.match(r"\s+'(.+?)\\t.*?(?:\\n)?'[,;]?$", line)
        if m:
            found.append(m.group(1).rstrip("~"))
    return found


def annotations(cldr, language):
    """CLDR's name and keywords of each emoji in [language], derived ones included."""
    found = {}
    for pack, folder, key in [
        ("cldr-annotations-full", "annotations", "annotations"),
        ("cldr-annotations-derived-full", "annotationsDerived", "annotationsDerived"),
    ]:
        path = cldr / pack / folder / language / "annotations.json"
        data = json.loads(path.read_text(encoding="utf-8"))[key]["annotations"]
        found.update(data)
    return found


def entry(found, text):
    for key in (text, text.replace(VS16, ""), text + VS16):
        if key in found:
            return found[key]
    return None


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    cldr = Path(args[0])
    out = Path(args[1]) if len(args) > 1 else ROOT / "app/assets/emoji"
    out.mkdir(parents=True, exist_ok=True)
    emoji = table_emoji()
    assert len(emoji) > 1500, len(emoji)
    for language in LANGUAGES:
        found = annotations(cldr, language)
        lines = []
        for text in emoji:
            e = entry(found, text)
            if e is None:
                sys.exit(f"{language}: no annotation for {text!r}")
            name = (e.get("tts") or e["default"][:1] or [""])[0]
            words = []
            for word in e["default"]:
                if word != name and word not in words:
                    words.append(word)
            for field in [name] + words:
                assert not re.search(r"[\t\n|]", field), (language, text, field)
            lines.append(f"{text}\t{name}\t{'|'.join(words)}")
        (out / f"{language}.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"{language}: {len(lines)} emoji")


if __name__ == "__main__":
    main()
