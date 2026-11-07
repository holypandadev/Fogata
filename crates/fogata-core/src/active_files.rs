//! Files that act on their own: a file manager follows what they say as soon as it shows
//! them (to draw their icon it may reach a network share named inside, giving away the
//! person's sign-in to whoever runs it), or they run something when opened, whatever marks
//! say where they came from. A file of one of these kinds that came from someone else is
//! saved with [`INERT_ENDING`] added, so it opens as the plain text it is, and what it was
//! called still shows in its name.

/// Endings of files that act on their own, in lowercase; a name's ending is matched
/// whatever its case.
pub(crate) const ACTIVE_ENDINGS: &[&str] = &[
    // Windows. Shortcuts, and folders, searches and libraries that can lie on another
    // computer: Explorer reads them to draw their icon.
    "url",
    "lnk",
    "pif",
    "scf",
    "website",
    "library-ms",
    "searchconnector-ms",
    "search-ms",
    // Run when opened, past the question asked for a downloaded program.
    "settingcontent-ms",
    "appref-ms",
    "application",
    "diagcab",
    "msc",
    // Themes: their wallpaper, icons and sounds can lie on another computer, and Explorer
    // reads them to show the theme.
    "theme",
    "themepack",
    "deskthemepack",
    // Linux: launchers, which file managers show with the icon and name written inside.
    "desktop",
    "kdelnk",
    // macOS: run in the terminal when opened, or links to anywhere, files of other
    // computers included.
    "command",
    "tool",
    "terminal",
    "webloc",
    "fileloc",
    "inetloc",
    "afploc",
    "ftploc",
];

/// Whole names a file manager reads for the folder they are in (its icon, its name, what
/// runs when it is opened), in lowercase.
const ACTIVE_NAMES: &[&str] = &["desktop.ini", "autorun.inf"];

/// What is added to the name of a file that acts on its own, so it opens as text.
pub(crate) const INERT_ENDING: &str = ".txt";

/// Whether `ending` (without its dot) is one of [`ACTIVE_ENDINGS`], whatever its case.
pub(crate) fn active_ending(ending: &str) -> bool {
    ACTIVE_ENDINGS
        .iter()
        .any(|active| active.eq_ignore_ascii_case(ending))
}

/// Whether a file named `name` acts on its own. Dots and spaces at the end are not looked
/// at, as Windows drops them: `link.url. ` is `link.url` there.
pub(crate) fn acts_on_its_own(name: &str) -> bool {
    let name = name.trim_end_matches(['.', ' ']);
    ACTIVE_NAMES
        .iter()
        .any(|active| active.eq_ignore_ascii_case(name))
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ending)| active_ending(ending))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_kind_acts_on_its_own_whatever_its_case() {
        for ending in ACTIVE_ENDINGS {
            assert_eq!(*ending, ending.to_ascii_lowercase());
            let upper = ending.to_ascii_uppercase();
            let mixed: String = ending
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 2 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect();
            for name in [
                format!("a.{ending}"),
                format!("report.pdf.{upper}"),
                format!("x.{mixed}"),
                format!("x.{ending}."),
                format!("x.{ending} "),
                format!("x.{ending} . ."),
            ] {
                assert!(acts_on_its_own(&name), "{name}");
            }
        }
        for name in ["desktop.ini", "Desktop.INI", "AUTORUN.inf", "autorun.inf. "] {
            assert!(acts_on_its_own(name), "{name}");
        }
    }

    #[test]
    fn ordinary_files_do_not() {
        for name in [
            "photo.jpg",
            "IMG_2041.HEIC",
            "plan.pdf",
            "backup.zip",
            "report.docx",
            "notes.txt",
            "song.mp3",
            "clip.mp4",
            "page.html",
            "setup.exe",
            "url",
            "lnk",
            "desktop",
            "my.url.txt",
            "shortcut.lnk.jpg",
            "desktop.ini.txt",
            "my desktop.ini",
            "theme park.png",
            "",
        ] {
            assert!(!acts_on_its_own(name), "{name}");
        }
    }
}
