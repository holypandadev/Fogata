//! The core's error messages the app puts in its own words (`explain()` in
//! app/lib/src/format.dart) are found there by a part of their words, listed in
//! app/test/core_errors.txt. Each part must still be written by the core, or said by the
//! system, as the list says: a message reworded here would reach people in the core's raw
//! English, and nothing else would notice. (The app's own test checks that format.dart looks
//! for exactly the parts listed.)

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The list's lines: where the words come from, the system's error number if any, the words.
fn listed() -> Vec<(String, Option<i32>, String)> {
    let file = root().join("app/test/core_errors.txt");
    let list = std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
    list.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let (from, rest) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("no words in {line:?}"));
            match from {
                "core" => (from.to_string(), None, rest.to_string()),
                "unix" | "windows" => {
                    let (number, words) = rest
                        .split_once(' ')
                        .unwrap_or_else(|| panic!("no number and words in {line:?}"));
                    let number = number
                        .parse()
                        .unwrap_or_else(|_| panic!("not an error number in {line:?}"));
                    (from.to_string(), Some(number), words.to_string())
                }
                _ => panic!("{line:?} does not start with core, unix or windows"),
            }
        })
        .collect()
}

/// The Rust files of the crates the application is built from, without the tests: those of
/// `tests/` folders, the unit tests at the end of a file (`#[cfg(test)] mod ... {` up to its
/// closing brace at the start of a line), the fuzzing entry points and comments. The command
/// line program (fogata-cli) and the frozen copy of fogata-state are not the application.
fn core_sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut folders = vec![root().join("crates")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                let left_out =
                    matches!(name.as_str(), "tests" | "before" | "target" | "fogata-cli")
                        || name.starts_with('.');
                if !left_out {
                    folders.push(path);
                }
            } else if name.ends_with(".rs") && name != "fuzzing.rs" {
                let source = std::fs::read_to_string(&path).unwrap();
                found.push((path, without_tests(&source)));
            }
        }
    }
    assert!(found.len() > 100, "the crates' sources were not found");
    found
}

fn without_tests(source: &str) -> String {
    let mut kept = String::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "#[cfg(test)]"
            && lines
                .peek()
                .is_some_and(|next| next.starts_with("mod ") && next.trim_end().ends_with('{'))
        {
            for line in lines.by_ref() {
                if line == "}" {
                    break;
                }
            }
            continue;
        }
        if !line.trim_start().starts_with("//") {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    kept
}

#[test]
fn the_core_still_writes_each_message_the_app_explains() {
    let sources = core_sources();
    let mut missing = Vec::new();
    let mut checked = 0;
    for (from, _, words) in listed().into_iter().filter(|(from, ..)| from == "core") {
        checked += 1;
        if !sources.iter().any(|(_, source)| source.contains(&words)) {
            missing.push(format!("{from} {words}"));
        }
    }
    assert!(checked > 30, "only {checked} messages listed");
    assert!(
        missing.is_empty(),
        "the core no longer writes these, which app/lib/src/format.dart explains: {missing:#?}"
    );
}

#[test]
fn the_system_still_says_each_message_the_app_explains() {
    let system = if cfg!(windows) { "windows" } else { "unix" };
    // Windows says it in the language it was installed in, and the list has its English
    // (the app explains only that): told by its words for a file not found (error 2).
    if cfg!(windows)
        && !std::io::Error::from_raw_os_error(2)
            .to_string()
            .contains("cannot find the file")
    {
        return;
    }
    let mut checked = 0;
    for (from, number, words) in listed() {
        if from != system {
            continue;
        }
        let said = std::io::Error::from_raw_os_error(number.unwrap()).to_string();
        assert!(said.contains(&words), "{said:?} does not say {words:?}");
        checked += 1;
    }
    assert!(checked > 0, "nothing listed for {system}");
}

#[test]
fn unit_tests_and_comments_are_left_out_of_the_sources() {
    let source = "fn a() { bail!(\"kept\") }\n// \"a comment\"\n#[cfg(test)]\nmod tests {\n    \
                  fn t() { \"in a test\" }\n}\nfn b() {}\n";
    let kept = without_tests(source);
    assert!(kept.contains("kept") && kept.contains("fn b()"));
    assert!(!kept.contains("a comment") && !kept.contains("in a test"));
}
