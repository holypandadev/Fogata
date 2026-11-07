//! Whether this computer's own firewall keeps everyone from reaching Fogata.
//!
//! Most connections start from both ends at once, which a firewall lets through as answers.
//! A friend or member at the same place behind another router (two routers at home, one
//! public address) can only reach this device by coming in, which a firewall that lets
//! nothing in unasked stops. The person is then told the one command that lets Fogata in:
//! - Linux with ufw: the rules are read without privileges; the command lets Fogata's own
//!   port in.
//! - Windows: answering "Cancel" when the firewall asks about Fogata the first time it
//!   listens leaves rules that block it. The local rules are read from the registry with
//!   `reg query`, which shows them the same in every language (`netsh` translates its
//!   output); the command turns Fogata's blocking rules into allowing ones.
//! - macOS: the application firewall may block Fogata, or every incoming connection; its
//!   own tool (`socketfilterfw`) tells, and the command unblocks Fogata or turns that off.
//!
//! What cannot be read counts as nothing in the way: the notice never shows on a guess.

/// On Linux, the command that lets `port` in, if ufw is on, lets nothing in unasked and has
/// no rule for that port. `None` when nothing can be read.
#[cfg(target_os = "linux")]
pub fn blocking_command(port: u16) -> Option<String> {
    ufw::blocking_command_in(std::path::Path::new("/etc"), port)
}

/// On Windows, the command that lets Fogata in, if an active rule of the firewall blocks it
/// on the kind of network (private, public, domain) this computer is on. It starts `reg`
/// (and PowerShell when the rules differ between kinds of network), so it may take a
/// second: not on an async task's thread.
#[cfg(windows)]
pub fn blocking_command(port: u16) -> Option<String> {
    defender::blocking_command(port)
}

/// On macOS, the command that lets Fogata in, if the application firewall is on and blocks
/// it, or blocks every incoming connection. It starts the firewall's tool: not on an async
/// task's thread.
#[cfg(target_os = "macos")]
pub fn blocking_command(_port: u16) -> Option<String> {
    alf::blocking_command()
}

/// Elsewhere, nothing is known.
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub fn blocking_command(_port: u16) -> Option<String> {
    None
}

/// Runs a system tool with no window and no input and returns its exit status and what it
/// printed, or `None` if it could not start or took more than a few seconds (it is then
/// stopped).
#[cfg(any(windows, target_os = "macos"))]
fn output(mut command: std::process::Command) -> Option<(std::process::ExitStatus, String)> {
    use std::io::Read as _;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW: a console tool started by a windowed program would flash one.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Read on the side: a long answer fills the pipe before the tool ends.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let bytes = reader.join().ok()?;
    Some((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[cfg(any(target_os = "linux", test))]
mod ufw {
    use std::path::Path;

    pub(super) fn blocking_command_in(etc: &Path, port: u16) -> Option<String> {
        let enabled = std::fs::read_to_string(etc.join("ufw/ufw.conf")).ok()?;
        if !setting(&enabled, "ENABLED").is_some_and(|v| v.eq_ignore_ascii_case("yes")) {
            return None;
        }
        let defaults = std::fs::read_to_string(etc.join("default/ufw")).unwrap_or_default();
        let policy = setting(&defaults, "DEFAULT_INPUT_POLICY").unwrap_or_else(|| "DROP".into());
        if policy.eq_ignore_ascii_case("ACCEPT") {
            return None;
        }
        let mut rules = String::new();
        for file in ["ufw/user.rules", "ufw/user6.rules"] {
            rules.push_str(&std::fs::read_to_string(etc.join(file)).unwrap_or_default());
        }
        if rules.lines().any(|line| lets_in(line, port)) {
            return None;
        }
        Some(format!("sudo ufw allow {port} comment 'Fogata'"))
    }

    /// `KEY=value` or `KEY="value"` from a shell-style settings file.
    fn setting(text: &str, key: &str) -> Option<String> {
        text.lines().find_map(|line| {
            let (k, v) = line.trim().split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
        })
    }

    /// Whether one of ufw's rule records (`### tuple ### allow udp 5353 0.0.0.0/0 any
    /// 0.0.0.0/0 in`) lets UDP in to `port` from anywhere, or lets everything in.
    fn lets_in(line: &str, port: u16) -> bool {
        let Some(tuple) = line.strip_prefix("### tuple ### ") else {
            return false;
        };
        let fields: Vec<&str> = tuple.split_whitespace().collect();
        let [action, proto, dport, _dst, _sport, src, direction, ..] = fields[..] else {
            return false;
        };
        if !matches!(action, "allow" | "limit") || direction != "in" {
            return false;
        }
        if !matches!(proto, "udp" | "any") {
            return false;
        }
        if dport == "any" {
            return matches!(src, "0.0.0.0/0" | "::/0");
        }
        dport.split(',').any(|part| match part.split_once(':') {
            Some((low, high)) => matches!(
                (low.parse::<u16>(), high.parse::<u16>()),
                (Ok(low), Ok(high)) if (low..=high).contains(&port)
            ),
            None => part.parse::<u16>() == Ok(port),
        })
    }
}

/// Windows' own firewall. Its local rules are registry values that read
/// `v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|Profile=Public|App=C:\…\fogata.exe|Name=…|`,
/// the same in every language. A blocking rule wins over any allowing one. Only a rule that
/// blocks UDP into Fogata from anywhere counts: one limited to some addresses, ports or
/// network interfaces leaves the way open to others. Having no rule at all does not count
/// either: Windows then asks the person itself, and other rules (for the port, for every
/// program) or another firewall may let Fogata in.
#[cfg(any(windows, test))]
mod defender {
    /// Where the local rules and each kind of network's switch are kept.
    #[cfg(windows)]
    const POLICY: &str =
        r"HKLM\SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy";

    /// Kinds of network (the firewall's profiles), as bits.
    pub(super) const DOMAIN: u8 = 1;
    pub(super) const PRIVATE: u8 = 2;
    pub(super) const PUBLIC: u8 = 4;
    pub(super) const ALL: u8 = DOMAIN | PRIVATE | PUBLIC;

    /// Fields that limit a rule to some addresses, ports, interfaces or a service.
    const LIMITS: [&str; 6] = ["RA", "LA", "RPort", "Svc", "IF", "RmtIntf"];

    /// The kinds of network on which an active rule blocks UDP into `exe` (on `port`) from
    /// anywhere, from `reg query` of the rules' key: lines `    {name}    REG_SZ    v2.31|…`.
    /// `env` gives the environment variables a rule's path may hold (`%LOCALAPPDATA%\…`).
    pub(super) fn blocked(
        rules: &str,
        exe: &str,
        port: u16,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> u8 {
        let exe = program_key(exe, env);
        let mut kinds = 0;
        for line in rules.lines() {
            let Some((_, data)) = line.split_once("REG_SZ") else {
                continue;
            };
            let fields: Vec<(&str, &str)> = data
                .trim()
                .split('|')
                .filter_map(|field| field.split_once('='))
                .collect();
            let first = |key: &str| fields.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
            if !first("Action").is_some_and(|v| v.eq_ignore_ascii_case("Block"))
                || !first("Active").is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
                || !first("Dir").is_some_and(|v| v.eq_ignore_ascii_case("In"))
                || !first("Protocol").is_none_or(|p| p == "17")
                || !first("App").is_some_and(|app| program_key(app, env) == exe)
            {
                continue;
            }
            let ports: Vec<&str> = fields
                .iter()
                .filter(|(k, _)| k.starts_with("LPort"))
                .map(|(_, v)| *v)
                .collect();
            let limited = fields
                .iter()
                .any(|(k, _)| LIMITS.iter().any(|limit| k.starts_with(limit)));
            if limited || !(ports.is_empty() || ports.iter().any(|p| covers(p, port))) {
                continue;
            }
            let mut on = 0;
            for (k, v) in &fields {
                if *k == "Profile" {
                    on |= match *v {
                        "Domain" => DOMAIN,
                        "Private" => PRIVATE,
                        "Public" => PUBLIC,
                        _ => 0,
                    };
                }
            }
            // No profile named: every kind of network.
            kinds |= if fields.iter().any(|(k, _)| *k == "Profile") {
                on
            } else {
                ALL
            };
        }
        kinds
    }

    /// Whether a rule's local ports (`31234`, `30000-32000`, or a word such as `RPC`) take
    /// `port` in.
    fn covers(ports: &str, port: u16) -> bool {
        ports.split(',').any(|part| match part.split_once('-') {
            Some((low, high)) => matches!(
                (low.parse::<u16>(), high.parse::<u16>()),
                (Ok(low), Ok(high)) if (low..=high).contains(&port)
            ),
            None => part.parse::<u16>() == Ok(port),
        })
    }

    /// A program's path in a form two spellings of it share: variables filled in, without
    /// `\\?\`, backslashes, ASCII in lower case (Windows' own rules keep the path in lower
    /// case), and every run of other characters as one `?`, which no path holds. `reg` prints
    /// in the console's code page, so a name like Sofía may not come back as written.
    pub(super) fn program_key(path: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
        let mut filled = String::new();
        let mut rest = path;
        while let Some(start) = rest.find('%') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('%') else {
                break;
            };
            filled.push_str(&rest[..start]);
            match env(&after[..end]) {
                Some(value) => filled.push_str(&value),
                None => filled.push_str(&rest[start..start + end + 2]),
            }
            rest = &after[end + 1..];
        }
        filled.push_str(rest);
        let filled = filled.strip_prefix(r"\\?\").unwrap_or(&filled);
        let mut key = String::new();
        let mut other = false;
        for c in filled.chars() {
            if c.is_ascii() {
                key.push(if c == '/' {
                    '\\'
                } else {
                    c.to_ascii_lowercase()
                });
                other = false;
            } else if !other {
                key.push('?');
                other = true;
            }
        }
        key
    }

    /// Whether a kind of network's firewall is on, from `reg query <key> /v EnableFirewall`
    /// (`    EnableFirewall    REG_DWORD    0x1`).
    pub(super) fn enabled(text: &str) -> Option<bool> {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once("REG_DWORD")?;
            (name.trim() == "EnableFirewall").then_some(value.trim() != "0x0")
        })
    }

    /// The kinds of network this computer is on, from lines `Private Internet NoTraffic`
    /// (category, IPv4 and IPv6 reach) printed by `NETWORKS`. Networks that reach the
    /// internet count; with none, every one listed (a virtual machine's network or a cable
    /// to another computer would otherwise decide).
    pub(super) fn current(text: &str) -> u8 {
        let (mut online, mut any) = (0, 0);
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let kind = match words.next() {
                Some("DomainAuthenticated") => DOMAIN,
                Some("Private") => PRIVATE,
                Some("Public") => PUBLIC,
                _ => continue,
            };
            any |= kind;
            if words.any(|reach| reach == "Internet") {
                online |= kind;
            }
        }
        if online != 0 { online } else { any }
    }

    /// PowerShell that lists the networks this computer is on (no rights needed).
    #[cfg(windows)]
    const NETWORKS: &str = "Get-NetConnectionProfile | ForEach-Object { \
        [string]$_.NetworkCategory + ' ' + [string]$_.IPv4Connectivity + ' ' + \
        [string]$_.IPv6Connectivity }";

    /// Whether Fogata is kept out, given the kinds of network where a rule blocks it and the
    /// firewall is on. Which kind this computer is on is asked only when that matters.
    pub(super) fn kept_out(blocked: u8, current: impl FnOnce() -> Option<u8>) -> bool {
        match blocked & ALL {
            0 => false,
            ALL => true,
            some => current().is_some_and(|on| on & some != 0),
        }
    }

    /// The PowerShell command, run as administrator, that turns the rules blocking `exe`
    /// from coming in into rules that let it in. Windows keeps the path as it wants (in
    /// lower case), and `-eq` compares without case.
    pub(super) fn command(exe: &str) -> String {
        let exe = exe.replace('\'', "''");
        format!(
            "Get-NetFirewallApplicationFilter | Where-Object Program -eq '{exe}' | \
             Get-NetFirewallRule | Where-Object {{ $_.Direction -eq 'Inbound' -and \
             $_.Action -eq 'Block' }} | Set-NetFirewallRule -Action Allow"
        )
    }

    #[cfg(windows)]
    pub(super) fn blocking_command(port: u16) -> Option<String> {
        use base64::Engine as _;
        use std::path::PathBuf;
        use std::process::Command;
        let exe = std::env::current_exe().ok()?;
        let exe = exe.to_str()?;
        // From the Windows folder: by its name alone, the program's own folder is looked in
        // first.
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("System32");
        let reg = |args: &[&str]| {
            let mut command = Command::new(system.join("reg.exe"));
            command.args(args);
            let (status, text) = super::output(command)?;
            status.success().then_some(text)
        };
        let rules = reg(&["query", &format!(r"{POLICY}\FirewallRules")])?;
        let env = |name: &str| std::env::var(name).ok();
        let blocked = blocked(&rules, exe, port, &env);
        if blocked == 0 {
            return None;
        }
        let mut on = 0;
        for (kind, key) in [
            (DOMAIN, "DomainProfile"),
            (PRIVATE, "StandardProfile"),
            (PUBLIC, "PublicProfile"),
        ] {
            let switch = reg(&["query", &format!(r"{POLICY}\{key}"), "/v", "EnableFirewall"]);
            if switch.as_deref().and_then(enabled) != Some(false) {
                on |= kind;
            }
        }
        let networks = || {
            let script: Vec<u8> = NETWORKS.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let mut command = Command::new(system.join(r"WindowsPowerShell\v1.0\powershell.exe"));
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-EncodedCommand",
                &base64::engine::general_purpose::STANDARD.encode(script),
            ]);
            let (status, text) = super::output(command)?;
            let on = current(&text);
            (status.success() && on != 0).then_some(on)
        };
        kept_out(blocked & on, networks).then(|| command(exe))
    }
}

/// macOS's application firewall, through its own tool. A sandboxed copy of Fogata may not be
/// allowed to run it: the tool then fails or answers that nothing is on, and nothing is shown.
#[cfg(any(target_os = "macos", test))]
mod alf {
    pub(super) const TOOL: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";

    /// The firewall's state from `--getglobalstate` (`Firewall is enabled. (State = 1)`):
    /// 0 off, 1 on, 2 blocking every incoming connection.
    pub(super) fn state(text: &str) -> Option<u8> {
        if let Some((_, after)) = text.split_once("State = ") {
            return after.trim_start().get(..1)?.parse().ok();
        }
        let text = text.to_ascii_lowercase();
        if text.contains("disabled") {
            Some(0)
        } else if text.contains("block") {
            Some(2)
        } else if text.contains("enabled") {
            Some(1)
        } else {
            None
        }
    }

    /// Whether `--getblockall` says every incoming connection is blocked (`Firewall has
    /// block all state set to enabled.`, or `Block all ENABLED!` on older systems).
    pub(super) fn blocks_all(text: &str) -> Option<bool> {
        let text = text.to_ascii_lowercase();
        if text.contains("disabled") {
            Some(false)
        } else if text.contains("enabled") {
            Some(true)
        } else {
            None
        }
    }

    /// Whether `--getappblocked <app>` says the app is blocked from accepting incoming
    /// connections (not when it is permitted, or not in the firewall's list).
    pub(super) fn app_blocked(text: &str) -> bool {
        let text = text.to_ascii_lowercase();
        text.contains("block")
            && !["permit", "allow", "not part", "not found", "unblock"]
                .iter()
                .any(|word| text.contains(word))
    }

    /// The command that lets `app` in, or turns off blocking every incoming connection.
    pub(super) fn command(all: bool, app: &str) -> String {
        if all {
            format!("sudo {TOOL} --setblockall off")
        } else {
            format!("sudo {TOOL} --unblockapp '{}'", app.replace('\'', r"'\''"))
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn blocking_command() -> Option<String> {
        let exe = std::env::current_exe().ok()?;
        // The firewall knows the app by its bundle.
        let app = exe
            .ancestors()
            .find(|path| path.extension().is_some_and(|e| e == "app"))
            .unwrap_or(&exe)
            .to_str()?;
        let ask = |args: &[&str]| {
            let mut command = std::process::Command::new(TOOL);
            command.args(args);
            super::output(command).map(|(_, text)| text)
        };
        let state = state(&ask(&["--getglobalstate"])?)?;
        if state == 0 {
            return None;
        }
        let all =
            state == 2 || ask(&["--getblockall"]).as_deref().and_then(blocks_all) == Some(true);
        if all {
            return Some(command(true, app));
        }
        app_blocked(&ask(&["--getappblocked", app])?).then(|| command(false, app))
    }
}

#[cfg(test)]
mod tests {
    use super::ufw::blocking_command_in;
    use super::*;

    fn etc(enabled: &str, policy: &str, rules: &str) -> tempfile_dir::Dir {
        let dir = tempfile_dir::Dir::new();
        std::fs::create_dir_all(dir.path().join("ufw")).unwrap();
        std::fs::create_dir_all(dir.path().join("default")).unwrap();
        std::fs::write(
            dir.path().join("ufw/ufw.conf"),
            format!("ENABLED={enabled}\n"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("default/ufw"),
            format!("DEFAULT_INPUT_POLICY=\"{policy}\"\n"),
        )
        .unwrap();
        std::fs::write(dir.path().join("ufw/user.rules"), rules).unwrap();
        dir
    }

    const MDNS_ONLY: &str = "### tuple ### allow udp 5353 0.0.0.0/0 any 0.0.0.0/0 in\n\
        -A ufw-user-input -p udp --dport 5353 -j ACCEPT\n";

    #[test]
    fn a_closed_firewall_gets_the_command() {
        let dir = etc("yes", "DROP", MDNS_ONLY);
        assert_eq!(
            blocking_command_in(dir.path(), 31234).as_deref(),
            Some("sudo ufw allow 31234 comment 'Fogata'")
        );
    }

    #[test]
    fn an_open_port_an_open_policy_or_no_ufw_get_nothing() {
        let rules = format!(
            "{MDNS_ONLY}### tuple ### allow any 31234 0.0.0.0/0 any 0.0.0.0/0 in comment=466f67617461\n"
        );
        assert!(blocking_command_in(etc("yes", "DROP", &rules).path(), 31234).is_none());
        let range = "### tuple ### allow udp 30000:32000 0.0.0.0/0 any 0.0.0.0/0 in\n";
        assert!(blocking_command_in(etc("yes", "DROP", range).path(), 31234).is_none());
        assert!(blocking_command_in(etc("yes", "ACCEPT", MDNS_ONLY).path(), 31234).is_none());
        assert!(blocking_command_in(etc("no", "DROP", MDNS_ONLY).path(), 31234).is_none());
        let empty = tempfile_dir::Dir::new();
        assert!(blocking_command_in(empty.path(), 31234).is_none());
    }

    #[test]
    fn rules_for_tcp_only_another_port_or_one_source_do_not_count() {
        let rules = "### tuple ### allow tcp 31234 0.0.0.0/0 any 0.0.0.0/0 in\n\
            ### tuple ### allow udp 31235 0.0.0.0/0 any 0.0.0.0/0 in\n\
            ### tuple ### allow any any 0.0.0.0/0 any 192.168.0.1 in\n\
            ### tuple ### deny udp 31234 0.0.0.0/0 any 0.0.0.0/0 in\n";
        assert!(blocking_command_in(etc("yes", "DROP", rules).path(), 31234).is_some());
    }

    const EXE: &str = r"C:\Users\Sofía\AppData\Local\Fogata\fogata.exe";

    /// `reg query` of the rules' key, as Windows prints it: the two rules "Cancel" leaves
    /// (TCP and UDP, in Windows' lower case, `í` in another code page), and rules of other
    /// programs.
    fn reg_rules(fogata: &[&str]) -> String {
        let mut text = String::from(
            "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\SharedAccess\\Parameters\\FirewallPolicy\\FirewallRules\r\n\
             \x20   CoreNet-DHCP-In    REG_SZ    v2.10|Action=Allow|Active=TRUE|Dir=In|Protocol=17|LPort=68|RPort=67|App=%SystemRoot%\\system32\\svchost.exe|Svc=dhcp|Name=@FirewallAPI.dll,-25301|\r\n\
             \x20   {7B1A0D52-0000-4000-8000-000000000001}    REG_SZ    v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|Profile=Public|App=C:\\program files\\other\\other.exe|Name=other.exe|\r\n",
        );
        for (i, rule) in fogata.iter().enumerate() {
            text.push_str(&format!(
                "    {{7B1A0D52-0000-4000-8000-00000000010{i}}}    REG_SZ    {rule}\r\n"
            ));
        }
        text
    }

    fn env(name: &str) -> Option<String> {
        name.eq_ignore_ascii_case("LOCALAPPDATA")
            .then(|| r"C:\Users\Sofía\AppData\Local".to_string())
    }

    #[test]
    fn windows_rules_left_by_cancel_block_fogata() {
        use defender::*;
        let cancel = reg_rules(&[
            "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=6|Profile=Public|App=C:\\users\\sof\u{fffd}a\\appdata\\local\\fogata\\fogata.exe|Name=fogata.exe|Desc=fogata.exe|Defer=User|",
            "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|Profile=Public|App=C:\\users\\sof\u{fffd}a\\appdata\\local\\fogata\\fogata.exe|Name=fogata.exe|Desc=fogata.exe|Defer=User|",
        ]);
        assert_eq!(blocked(&cancel, EXE, 31234, &env), PUBLIC);
        // Several profiles, none named (every one), or a path through a variable.
        let both = reg_rules(&[
            "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|Profile=Private|Profile=Domain|App=c:/users/sofía/appdata/local/fogata/fogata.exe|Name=Fogata|",
        ]);
        assert_eq!(blocked(&both, EXE, 31234, &env), PRIVATE | DOMAIN);
        let any = reg_rules(&[
            "v2.31|Action=Block|Active=TRUE|Dir=In|App=%LOCALAPPDATA%\\Fogata\\fogata.exe|Name=Fogata|",
        ]);
        assert_eq!(blocked(&any, EXE, 31234, &env), ALL);
        let port = reg_rules(&[
            "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|LPort=30000-32000|App=C:\\Users\\Sofía\\AppData\\Local\\Fogata\\fogata.exe|Name=Fogata|",
        ]);
        assert_eq!(blocked(&port, EXE, 31234, &env), ALL);
        assert_eq!(
            program_key(r"\\?\C:\Users\Sofía\AppData\Local\Fogata\fogata.exe", &env),
            r"c:\users\sof?a\appdata\local\fogata\fogata.exe"
        );
    }

    #[test]
    fn windows_rules_that_let_fogata_in_or_only_block_some_do_not_count() {
        use defender::*;
        let app = r"App=C:\users\sofía\appdata\local\fogata\fogata.exe";
        let rules = reg_rules(&[
            &format!("v2.31|Action=Allow|Active=TRUE|Dir=In|Protocol=17|{app}|Name=fogata.exe|"),
            &format!("v2.31|Action=Block|Active=FALSE|Dir=In|Protocol=17|{app}|Name=off|"),
            &format!("v2.31|Action=Block|Active=TRUE|Dir=Out|Protocol=17|{app}|Name=out|"),
            &format!("v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=6|{app}|Name=tcp|"),
            &format!(
                "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|RA4=203.0.113.7|{app}|Name=one address|"
            ),
            &format!(
                "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|LPort=5353|{app}|Name=another port|"
            ),
            "v2.31|Action=Block|Active=TRUE|Dir=In|Protocol=17|App=C:\\users\\sofía\\appdata\\local\\fogata-old\\fogata.exe|Name=another copy|",
        ]);
        assert_eq!(blocked(&rules, EXE, 31234, &env), 0);
        assert_eq!(blocked("ERROR: Access is denied.\r\n", EXE, 31234, &env), 0);
    }

    #[test]
    fn windows_asks_for_the_network_only_when_it_matters() {
        use defender::*;
        let switch = "\r\nHKEY_LOCAL_MACHINE\\…\\PublicProfile\r\n    EnableFirewall    REG_DWORD    0x0\r\n";
        assert_eq!(enabled(switch), Some(false));
        assert_eq!(enabled(&switch.replace("0x0", "0x1")), Some(true));
        assert_eq!(
            enabled(
                "ERROR: The system was unable to find the specified registry key or value.\r\n"
            ),
            None
        );
        assert!(!kept_out(0, || panic!("not asked")));
        assert!(kept_out(ALL, || panic!("not asked")));
        assert!(kept_out(PUBLIC, || Some(PUBLIC)));
        assert!(!kept_out(PUBLIC, || Some(PRIVATE)));
        assert!(!kept_out(PUBLIC, || None));
        // The network that reaches the internet decides; a virtual machine's does not.
        let networks = "Public NoTraffic NoTraffic\r\nPrivate Internet NoTraffic\r\n";
        assert_eq!(current(networks), PRIVATE);
        assert_eq!(current("Public LocalNetwork NoTraffic\r\n"), PUBLIC);
        assert_eq!(
            current("DomainAuthenticated NoTraffic Internet\r\n"),
            DOMAIN
        );
        assert_eq!(current(""), 0);
        assert_eq!(
            command(r"C:\Users\Bea O'Neil\AppData\Local\Fogata\fogata.exe"),
            "Get-NetFirewallApplicationFilter | Where-Object Program -eq \
             'C:\\Users\\Bea O''Neil\\AppData\\Local\\Fogata\\fogata.exe' | Get-NetFirewallRule | \
             Where-Object { $_.Direction -eq 'Inbound' -and $_.Action -eq 'Block' } | \
             Set-NetFirewallRule -Action Allow"
        );
    }

    #[test]
    fn macos_firewall_answers() {
        use alf::*;
        assert_eq!(state("Firewall is enabled. (State = 1)\n"), Some(1));
        assert_eq!(state("Firewall is disabled. (State = 0)\n"), Some(0));
        assert_eq!(
            state("Firewall is blocking all non-essential incoming connections. (State = 2)\n"),
            Some(2)
        );
        assert_eq!(state("Firewall is disabled.\n"), Some(0));
        assert_eq!(state(""), None);
        assert_eq!(
            blocks_all("Firewall has block all state set to enabled.\n"),
            Some(true)
        );
        assert_eq!(
            blocks_all("Firewall has block all state set to disabled.\n"),
            Some(false)
        );
        assert_eq!(blocks_all("Block all ENABLED!\n"), Some(true));
        assert_eq!(blocks_all("Block all DISABLED!\n"), Some(false));
        assert_eq!(blocks_all("sandbox: deny\n"), None);
        assert!(app_blocked(
            "The application /Applications/Fogata.app is blocked from accepting incoming connections.\n"
        ));
        assert!(!app_blocked(
            "The application /Applications/Fogata.app is permitted to accept incoming connections.\n"
        ));
        assert!(!app_blocked(
            "The application is not part of the firewall\n"
        ));
        // The wording of macOS 26.
        assert!(!app_blocked(
            "Incoming connection to /Applications/Fogata.app is permitted.\n"
        ));
        assert!(app_blocked(
            "Incoming connection to /Applications/Fogata.app is blocked.\n"
        ));
        assert!(!app_blocked(""));
        assert_eq!(
            command(false, "/Users/ana/Apps/Ana's Fogata.app"),
            "sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp '/Users/ana/Apps/Ana'\\''s Fogata.app'"
        );
        assert_eq!(
            command(true, "/Applications/Fogata.app"),
            "sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setblockall off"
        );
    }

    /// A folder removed when dropped.
    mod tempfile_dir {
        use std::path::{Path, PathBuf};

        pub struct Dir(PathBuf);

        impl Dir {
            pub fn new() -> Self {
                let mut bytes = [0u8; 8];
                fogata_crypto::random_bytes(&mut bytes).unwrap();
                let path = std::env::temp_dir().join(format!("fogata-ufw-{}", hex::encode(bytes)));
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
