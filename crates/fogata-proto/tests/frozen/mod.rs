//! Frozen encodings, shared by the `golden` tests of the crates whose bytes other versions
//! read: each crate includes this file (`#[path]`) and freezes its own in `tests/data`.
//!
//! Each case must still encode to the bytes frozen under its name, and those bytes must still
//! read back as it: what this version writes is what versions already handed out read, and
//! what they wrote is still read here. A fixture that changes is a change of the wire or of
//! the log, which needs a capability level (a marker peers advertise, a field older versions
//! skip, a `Level` of the log), not just a new fixture. Write the fixtures anew only on
//! purpose, once that is done, with `FOGATA_WRITE_FIXTURES=1 cargo test -p <crate> --test
//! golden`, and review the difference.

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    path::Path,
};

/// The variable that has the fixtures written anew instead of checked.
pub const WRITE: &str = "FOGATA_WRITE_FIXTURES";

const HEADER: &str = "\
# Frozen encodings, checked by this crate's tests/golden.rs: read its text before changing
# a line here. A line that changes is a change of what goes over the wire or into the
# group log, which needs a capability level first.
";

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "odd hex");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Reads bytes back, printed, or `None` when they do not read.
type Read = Box<dyn Fn(&[u8]) -> Option<String>>;

/// A message, its bytes as this version writes them, and how this version reads bytes back.
pub struct Case {
    name: String,
    bytes: Vec<u8>,
    printed: String,
    read: Read,
}

/// `value` under `name`, written with `encode` and read with `decode`. Not every message
/// has an equality: two that print the same hold the same.
pub fn case<T: Debug + 'static>(
    name: String,
    value: &T,
    encode: impl Fn(&T) -> Vec<u8>,
    decode: impl Fn(&[u8]) -> Option<T> + 'static,
) -> Case {
    Case {
        name,
        bytes: encode(value),
        printed: format!("{value:?}"),
        read: Box::new(move |bytes| decode(bytes).map(|v| format!("{v:?}"))),
    }
}

/// Checks that each of `cases` encodes to the bytes frozen under its name in `file` (in
/// `tests/data`), and that those bytes read back as it. With [`WRITE`]`=1` the file is
/// written anew from `cases` instead.
pub fn frozen(file: &str, cases: &[Case]) {
    let names: BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names.len(), cases.len(), "two cases of one name");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(file);
    if std::env::var_os(WRITE).is_some_and(|v| v == "1") {
        let lines: String = cases
            .iter()
            .map(|c| format!("{} {}\n", c.name, hex(&c.bytes)))
            .collect();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{HEADER}{lines}")).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (see this file's text)", path.display()));
    let fixture: BTreeMap<&str, &str> = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split_once(' ').expect("a name and its bytes"))
        .collect();
    assert_eq!(
        fixture.keys().copied().collect::<BTreeSet<_>>(),
        names,
        "the cases are not those frozen in {file}"
    );
    for c in cases {
        let frozen = fixture[c.name.as_str()];
        assert_eq!(
            hex(&c.bytes),
            frozen,
            "{} no longer encodes as frozen in {file}: a change of the wire or the log",
            c.name
        );
        assert_eq!(
            (c.read)(&unhex(frozen)).as_ref(),
            Some(&c.printed),
            "{} as frozen in {file} no longer reads the same",
            c.name
        );
    }
}
