//! What the Flutter interface calls. Ids travel as hex strings, times as milliseconds since
//! the Unix epoch.

pub mod account_move;
pub mod app;
pub mod direct;
pub mod recovery;
pub mod tray;
pub mod types;
pub mod update;
pub mod voice;

#[cfg(test)]
mod tests {
    /// The calls the interface makes on its own thread (`#[frb(sync)]`), each one quick:
    /// no disk write waited for, no keychain that may wait for the person. A call added
    /// here is a choice to block the window while it runs; one that reads or writes the
    /// data folder (`write_prefs`, `has_profile`) goes through a worker instead.
    /// `prepare_data_dir` stays, since the window needs its folder before anything else.
    const ON_THE_WINDOWS_THREAD: &[&str] = &[
        "move_code_problem",
        "move_confirm",
        "move_cancel",
        "recovery_new_key",
        "recovery_forget_new_key",
        "recovery_key_problem",
        "recovery_copy_name",
        "recovery_in_data_folder",
        "events_taken",
        "default_data_dir",
        "prepare_data_dir",
        "qr",
        "set_sound_output",
        "play_sound",
        "play_ring",
        "noise_suppressor",
        "reveal_in_explorer",
        "note_input",
        "voice_recording",
        "voice_record_stop",
        "voice_record_cancel",
        "voice_pause",
        "voice_set_speed",
        "voice_stop",
        "voice_playback",
    ];

    /// Whether `line` is the mark of a call run on the window's thread: `#[frb(sync)]` or
    /// `#[flutter_rust_bridge::frb(sync)]`, alone or among other options.
    fn marks_sync(line: &str) -> bool {
        let Some(options) = line
            .strip_prefix("#[frb(")
            .or_else(|| line.strip_prefix("#[flutter_rust_bridge::frb("))
        else {
            return false;
        };
        let options = options.strip_suffix(")]").unwrap_or(options);
        options.split(',').any(|option| option.trim() == "sync")
    }

    /// The names of the functions marked to run on the window's thread in `source`: the
    /// first `pub fn` after the mark, past other attributes and comments.
    fn sync_calls(source: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut marked = false;
        for line in source.lines().map(str::trim) {
            if marks_sync(line) {
                marked = true;
            } else if marked && line.starts_with("pub fn ") {
                let name = &line["pub fn ".len()..];
                let end = name.find(['(', '<']).unwrap_or(name.len());
                found.push(name[..end].to_string());
                marked = false;
            } else if marked && !(line.starts_with("#[") || line.starts_with("//")) {
                marked = false;
            }
        }
        found
    }

    /// The source of every module of the API, as `mod.rs` lists them: one added later is
    /// read too, and one whose file is not found fails the test.
    fn api_sources() -> Vec<(String, String)> {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
        let listed = include_str!("mod.rs");
        let modules: Vec<&str> = listed
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub mod "))
            .filter_map(|rest| rest.strip_suffix(';'))
            .collect();
        assert!(!modules.is_empty(), "mod.rs lists no modules");
        modules
            .into_iter()
            .map(|module| {
                let file = folder.join(format!("{module}.rs"));
                let file = if file.exists() {
                    file
                } else {
                    folder.join(module).join("mod.rs")
                };
                let source = std::fs::read_to_string(&file)
                    .unwrap_or_else(|e| panic!("module {module} not read at {file:?}: {e}"));
                (module.to_string(), source)
            })
            .collect()
    }

    #[test]
    fn a_call_is_marked_whichever_way_it_is_written() {
        assert!(marks_sync("#[frb(sync)]"));
        assert!(marks_sync("#[flutter_rust_bridge::frb(sync)]"));
        assert!(marks_sync("#[frb(sync, name = \"x\")]"));
        assert!(!marks_sync("#[frb(init)]"));
        assert!(!marks_sync("#[frb(name = \"asyncish\")]"));
        let source = "#[flutter_rust_bridge::frb(sync)]\n/// Says.\npub fn quick() {}\n";
        assert_eq!(sync_calls(source), ["quick"]);
    }

    #[test]
    fn only_quick_calls_run_on_the_windows_thread() {
        let sources = api_sources();
        let mut found: Vec<String> = sources
            .iter()
            .flat_map(|(_, source)| sync_calls(source))
            .collect();
        found.sort();
        let mut allowed: Vec<String> = ON_THE_WINDOWS_THREAD
            .iter()
            .map(|s| s.to_string())
            .collect();
        allowed.sort();
        assert_eq!(found, allowed);
        for slow in ["write_prefs", "has_profile"] {
            assert!(!found.iter().any(|f| f == slow), "{slow} blocks the window");
        }
    }
}
