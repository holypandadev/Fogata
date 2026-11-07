//! What is personal, taken out of a log line before it reaches the disk.
//!
//! The day files hold what the event formatter of [`super`] writes: the time, the level, the
//! target and the names of spans (all fixed in the code), then the message and the fields.
//! Those last two are the only parts that carry values, and they go through here. A field
//! keeps its value only when the value is plainly a number, a yes or no, a duration or one
//! word of the code (an enum's variant); an address or a peer keeps its shape with marks in
//! place of the values; anything else becomes `‹value›`, and an error's text is scrubbed
//! like a message. A message keeps its words and loses: text in quotes, the names of people,
//! groups, rooms, this computer and its user known here, the paths of the data folder and
//! the home folder, links and invitations, e-mail addresses, IP addresses and ports (plain
//! and in multiaddrs), host names of the local network, paths and file names (with the
//! words of the name before its last), codes (a move's too, without its dash), keys and long
//! runs of hex (with or without `0x`), base32, base58 or base64, and lists of bytes. Peer ids keep their last six
//! characters, device keys and other long hex their first six, fingerprints the short form
//! people see, so the steps of one device can still be followed. The application scrubs the
//! files again when it makes a report (`app/lib/src/scrub.dart`), with these rules and more:
//! what only it knows (names of people and groups, this computer's folders), tricks of
//! Unicode, and ids as numbers of that report.

/// What this computer knows that a log must not carry: names of people, groups and rooms,
/// of this computer and its user, and the paths of its folders.
#[derive(Debug, Default, Clone)]
pub(crate) struct Known {
    /// Each name as characters, the longest first; none shorter than two characters.
    names: Vec<Vec<char>>,
    /// Folders, the longest first.
    paths: Vec<String>,
}

impl Known {
    pub(crate) fn new(
        names: impl IntoIterator<Item = String>,
        paths: impl IntoIterator<Item = String>,
    ) -> Known {
        let mut known = Known::default();
        known.add_names(names);
        let mut paths: Vec<String> = paths
            .into_iter()
            .map(|p| p.trim_end_matches(['/', '\\']).to_owned())
            .filter(|p| p.chars().count() >= 2)
            .collect();
        paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
        paths.dedup();
        known.paths = paths;
        known
    }

    /// Adds names to those known; names are never forgotten while the process runs, so a
    /// renamed group's old name stays out too.
    pub(crate) fn add_names(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            let chars: Vec<char> = name.trim().chars().collect();
            if chars.len() >= 2 && !self.names.contains(&chars) {
                self.names.push(chars);
            }
        }
        self.names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.names.iter().map(|n| n.iter().collect())
    }

    pub(crate) fn has_name(&self, name: &str) -> bool {
        let chars: Vec<char> = name.trim().chars().collect();
        self.names.contains(&chars)
    }
}

/// The longest message scrubbed; the rest of a longer one is cut.
const LONGEST: usize = 4096;

/// Fields whose value is the text of an error, scrubbed as a message is.
const FREE_TEXT: &[&str] = &["error", "err", "e", "reason", "cause", "problem"];

/// A message without what is personal.
pub(crate) fn message(text: &str, known: &Known) -> String {
    let mut s: String = text.chars().take(LONGEST).collect();
    if s.len() < text.len() {
        s.push_str(" …(cut)");
    }
    for path in &known.paths {
        s = s.replace(path.as_str(), "‹path›");
    }
    s = apply(&s, quoted);
    s = apply(&s, |c, from| at_each(c, from, link));
    s = apply(&s, email);
    s = apply(&s, |c, from| at_each(c, from, |c, i| name(c, i, known)));
    s = said_with_names(&s);
    s = apply(&s, |c, from| at_each(c, from, multiaddr_part));
    s = apply(&s, |c, from| at_each(c, from, ipv4));
    s = apply(&s, |c, from| at_each(c, from, ipv6));
    s = apply(&s, |c, from| at_each(c, from, port_word));
    s = apply(&s, |c, from| at_each(c, from, port_after_host));
    s = apply(&s, |c, from| at_each(c, from, windows_path));
    s = apply(&s, |c, from| at_each(c, from, unix_path));
    s = apply(&s, host_name);
    s = apply(&s, file_name);
    s = apply(&s, |c, from| at_each(c, from, peer_id));
    s = apply(&s, |c, from| at_each(c, from, long_hex));
    s = apply(&s, |c, from| at_each(c, from, grouped_fingerprint));
    s = apply(&s, |c, from| at_each(c, from, fingerprint));
    s = apply(&s, |c, from| at_each(c, from, code));
    s = apply(&s, key);
    s = apply(&s, |c, from| at_each(c, from, bytes));
    slashed_words(&s)
}

/// A field's value as the day files keep it (see the module's text).
pub(crate) fn field(name: &str, value: &str, known: &Known) -> String {
    let v = value.trim();
    if v.is_empty() || plain(v) {
        return v.to_owned();
    }
    if let Some(shape) = multiaddr_shape(v) {
        return shape;
    }
    if let Some(short) = whole_peer(v) {
        return short;
    }
    if known.has_name(v) {
        return "‹name›".into();
    }
    let scrubbed = message(v, known);
    if is_identifier(v) && scrubbed == v {
        return scrubbed;
    }
    if FREE_TEXT.contains(&name) {
        return scrubbed;
    }
    // A variant with data: its name stays, and what it holds when that is plain.
    if let Some(open) = v.find(['(', '{']) {
        let (head, rest) = v.split_at(open);
        let head = head.trim_end();
        if is_identifier(head) && !known.has_name(head) {
            let inner = rest[1..].trim_end_matches([')', '}']).trim();
            return if rest.starts_with('(') {
                if plain(inner) || inner.is_empty() {
                    format!("{head}({inner})")
                } else {
                    format!("{head}(‹…›)")
                }
            } else {
                format!("{head} {{‹…›}}")
            };
        }
    }
    if plain_words(&scrubbed) {
        scrubbed
    } else {
        "‹value›".into()
    }
}

// ----- Matching -----

/// A match: where it starts and ends (character indices) and what takes its place.
type Found = Option<(usize, usize, String)>;

/// `s` with every match of `find` replaced, left to right, each search starting where the
/// last match ended (as a regular expression's replace-all does).
fn apply(s: &str, find: impl Fn(&[char], usize) -> Found) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut pos = 0;
    while pos < c.len() {
        let Some((start, end, with)) = find(&c, pos) else {
            break;
        };
        out.extend(&c[pos..start]);
        out.push_str(&with);
        pos = end.max(start + 1);
        if end <= start {
            out.push(c[start]);
        }
    }
    if pos < c.len() {
        out.extend(&c[pos..]);
    }
    out
}

/// The first match of a rule tried at each position from `from` on.
fn at_each(
    c: &[char],
    from: usize,
    rule: impl Fn(&[char], usize) -> Option<(usize, String)>,
) -> Found {
    (from..c.len()).find_map(|i| rule(c, i).map(|(end, with)| (i, end, with)))
}

/// A character of a word as regular expressions mean it (`\w`).
fn w(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// No word character just before `i` (`\b` at the start of a word).
fn starts_word(c: &[char], i: usize) -> bool {
    i == 0 || !w(c[i - 1])
}

/// No word character at `i` (`\b` at the end of a word).
fn ends_word(c: &[char], i: usize) -> bool {
    i >= c.len() || !w(c[i])
}

fn starts_with(c: &[char], i: usize, text: &str) -> bool {
    let mut rest = c.get(i..).unwrap_or_default().iter();
    text.chars().all(|t| rest.next() == Some(&t))
}

fn starts_with_ignoring_case(c: &[char], i: usize, text: &str) -> bool {
    let mut rest = c.get(i..).unwrap_or_default().iter();
    text.chars()
        .all(|t| rest.next().is_some_and(|x| x.eq_ignore_ascii_case(&t)))
}

/// The end of the run of characters from `i` that `class` takes.
fn run(c: &[char], i: usize, class: impl Fn(char) -> bool) -> usize {
    let mut k = i;
    while k < c.len() && class(c[k]) {
        k += 1;
    }
    k
}

// ----- Rules -----

/// Text in quotes of any kind: what a library printed of data, a message among it.
fn quoted(c: &[char], from: usize) -> Found {
    for i in from..c.len() {
        let close = match c[i] {
            '"' => '"',
            '“' => '”',
            '«' => '»',
            '‘' => '’',
            '`' => '`',
            // A single quote opens only where no letter is before it and something is
            // after it ("can't" has none).
            '\'' if (i == 0 || !c[i - 1].is_alphanumeric())
                && c.get(i + 1).is_some_and(|n| !n.is_whitespace()) =>
            {
                '\''
            }
            _ => continue,
        };
        let mut k = i + 1;
        while k < c.len() && c[k] != '\n' {
            if c[k] == '\\' && close == '"' {
                k += 2;
                continue;
            }
            if c[k] == close && (close != '\'' || k + 1 >= c.len() || !c[k + 1].is_alphanumeric()) {
                return Some((i, k + 1, format!("{}‹text›{close}", c[i])));
            }
            k += 1;
        }
        // Printed data cut before its end (a long message): the rest of the line goes.
        if !matches!(c[i], '\'' | '‘') {
            return Some((i, k, format!("{}‹text›", c[i])));
        }
    }
    None
}

/// Links (invitations carry their secret in them): `scheme://…`, `www.…` and Fogata's own
/// `fogata:…`.
fn link(c: &[char], i: usize) -> Option<(usize, String)> {
    if !c[i].is_ascii_alphabetic() || !starts_word(c, i) {
        return None;
    }
    let rest = |k: usize| {
        let end = run(c, k, |x| !x.is_whitespace());
        (end > k).then_some((end, "‹link›".to_owned()))
    };
    if starts_with_ignoring_case(c, i, "fogata:") {
        return rest(i + 7);
    }
    if starts_with_ignoring_case(c, i, "www.") {
        return rest(i + 4);
    }
    let scheme = run(c, i + 1, |x| x.is_ascii_alphanumeric() || "+.-".contains(x));
    if starts_with(c, scheme, "://") {
        return rest(scheme + 3);
    }
    None
}

/// E-mail addresses, which may hold a name.
fn email(c: &[char], from: usize) -> Found {
    let local = |x: char| x.is_alphanumeric() || "._+-".contains(x);
    let label = |x: char| x.is_alphanumeric() || "_-".contains(x);
    for at in from..c.len() {
        if c[at] != '@' {
            continue;
        }
        let mut start = at;
        while start > from && local(c[start - 1]) {
            start -= 1;
        }
        if start == at {
            continue;
        }
        let mut end = run(c, at + 1, label);
        if end == at + 1 {
            continue;
        }
        let mut dots = 0;
        while end < c.len() && c[end] == '.' {
            let next = run(c, end + 1, label);
            if next == end + 1 {
                break;
            }
            end = next;
            dots += 1;
        }
        if dots > 0 {
            return Some((start, end, "‹email›".into()));
        }
    }
    None
}

/// A name known here, in any case, as a whole word.
fn name(c: &[char], i: usize, known: &Known) -> Option<(usize, String)> {
    if i > 0 && c[i - 1].is_alphanumeric() {
        return None;
    }
    known.names.iter().find_map(|n| {
        let end = i + n.len();
        let same = end <= c.len()
            && c[i..end]
                .iter()
                .zip(n)
                .all(|(a, b)| a == b || a.to_lowercase().eq(b.to_lowercase()));
        (same && (end == c.len() || !c[end].is_alphanumeric())).then(|| (end, "‹name›".into()))
    })
}

/// What the core says with the name of a person or a group in it, for names not known here
/// (yet).
fn said_with_names(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (n, line) in s.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        out.push_str(&line_said_with_names(line));
    }
    out
}

fn line_said_with_names(line: &str) -> String {
    let mut line = line.to_owned();
    const ASKS: &str = " asks to join a group";
    if let Some(at) = line.find(ASKS) {
        let start = line[..at].rfind(": ").map_or(0, |p| p + 2);
        if start < at {
            line.replace_range(start..at, "‹name›");
        }
    }
    const JOINED: &str = "joined the group ";
    if let Some(at) = line.find(JOINED) {
        line.replace_range(at + JOINED.len().., "‹name›");
    }
    for word in ["group ", "chat "] {
        let mut from = 0;
        while let Some(found) = line[from..].find(word) {
            let at = from + found;
            let start = at + word.len();
            from = start;
            if line[..at].chars().next_back().is_some_and(w) {
                continue;
            }
            if let Some(end) = line[start..].find(" not loaded:").map(|e| start + e)
                && end > start
            {
                line.replace_range(start..end, "‹name›");
                from = start + "‹name›".len();
            }
        }
    }
    for verb in [
        "cannot save group ",
        "cannot read group ",
        "cannot run group ",
    ] {
        if let Some(at) = line.find(verb) {
            let start = at + verb.len();
            if let Some(colon) = line[start..].find(':').map(|e| start + e)
                && colon > start
            {
                let segment = &line[start..colon];
                let again = segment.len() > " again".len() && segment.ends_with(" again");
                let end = if again { colon - " again".len() } else { colon };
                line.replace_range(start..end, "‹name›");
            }
        }
    }
    line
}

/// The protocols of a multiaddr.
const PROTOCOLS: &[&str] = &[
    "ip4",
    "ip6",
    "dns",
    "dns4",
    "dns6",
    "dnsaddr",
    "sni",
    "udp",
    "tcp",
    "quic",
    "quic-v1",
    "p2p",
    "p2p-circuit",
    "webrtc",
    "webrtc-direct",
    "webtransport",
    "certhash",
    "tls",
    "noise",
    "ws",
    "wss",
    "http",
    "https",
];

/// Protocols whose value is an address.
const ADDRESSED: &[&str] = &["ip4", "ip6", "dns", "dns4", "dns6", "dnsaddr", "sni"];

/// A value in a multiaddr: an address, a port, a certificate hash; `/p2p/` ids are left to
/// [`peer_id`] (other kinds go as keys).
fn multiaddr_part(c: &[char], i: usize) -> Option<(usize, String)> {
    if c[i] != '/' {
        return None;
    }
    let value_end = |k: usize| run(c, k, |x| !x.is_whitespace() && !"/,)]".contains(x));
    for proto in ADDRESSED.iter().chain(&["udp", "tcp", "certhash", "p2p"]) {
        let k = i + 1 + proto.len();
        if !starts_with(c, i + 1, proto) || c.get(k) != Some(&'/') {
            continue;
        }
        let end = match *proto {
            "udp" | "tcp" => run(c, k + 1, |x| x.is_ascii_digit()),
            _ => value_end(k + 1),
        };
        if end == k + 1 {
            continue;
        }
        let mark = match *proto {
            "udp" | "tcp" => "‹port›",
            "certhash" => "‹key›",
            "p2p" if starts_with(c, k + 1, "12D3KooW") || starts_with(c, k + 1, "Qm") => {
                return None;
            }
            "p2p" => "‹key›",
            _ => "‹ip›",
        };
        return Some((end, format!("/{proto}/{mark}")));
    }
    None
}

/// IPv4 addresses, with their port.
fn ipv4(c: &[char], i: usize) -> Option<(usize, String)> {
    if i > 0 && (w(c[i - 1]) || c[i - 1] == '.') {
        return None;
    }
    let mut k = i;
    for part in 0..4 {
        let end = run(c, k, |x| x.is_ascii_digit());
        if !(1..=3).contains(&(end - k)) {
            return None;
        }
        k = end;
        if part < 3 {
            if c.get(k) != Some(&'.') {
                return None;
            }
            k += 1;
        }
    }
    let clear = |e: usize| e >= c.len() || !(w(c[e]) || c[e] == '.');
    if c.get(k) == Some(&':') {
        let end = run(c, k + 1, |x| x.is_ascii_digit());
        if (1..=5).contains(&(end - k - 1)) && clear(end) {
            return Some((end, "‹ip›".into()));
        }
    }
    clear(k).then(|| (k, "‹ip›".into()))
}

/// IPv6 addresses (and MAC addresses, which look alike), with their brackets, zone and port.
fn ipv6(c: &[char], i: usize) -> Option<(usize, String)> {
    if i > 0 && (w(c[i - 1]) || c[i - 1] == ':' || c[i - 1] == '‹') {
        return None;
    }
    let bracket = c[i] == '[';
    let start = if bracket { i + 1 } else { i };
    let end = run(c, start, |x| x.is_ascii_hexdigit() || x == ':');
    let text: String = c[start..end].iter().collect();
    let colons = text.matches(':').count();
    if !(2..=7).contains(&colons) || text.split(':').any(|p| p.len() > 4) || !is_ipv6(&text) {
        return None;
    }
    let mut k = end;
    if c.get(k) == Some(&'%') {
        let zone = run(c, k + 1, w);
        if zone > k + 1 {
            k = zone;
        }
    }
    let clear = |e: usize| e >= c.len() || !(w(c[e]) || c[e] == ':');
    let mut ends = Vec::new();
    let closed = if c.get(k) == Some(&']') { k + 1 } else { k };
    if c.get(closed) == Some(&':') {
        let port = run(c, closed + 1, |x| x.is_ascii_digit());
        if (1..=5).contains(&(port - closed - 1)) {
            ends.push(port);
        }
    }
    ends.extend([closed, k]);
    ends.into_iter()
        .find(|&e| clear(e))
        .map(|e| (e, "‹ip›".to_owned()))
}

fn is_ipv6(s: &str) -> bool {
    // A time of day (12:34:56) is no address.
    let time = !s.contains("::")
        && s.split(':')
            .all(|p| (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
        && (2..=3).contains(&s.split(':').count());
    if time {
        return false;
    }
    s.contains("::") || s.matches(':').count() >= 3 || s.bytes().any(|b| b.is_ascii_alphabetic())
}

/// `port 4001`, `port: 4001`, `port=4001`.
fn port_word(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) || !starts_with_ignoring_case(c, i, "port") {
        return None;
    }
    let mut k = i + 4;
    for optional in [':', '=', ' '] {
        if c.get(k) == Some(&optional) {
            k += 1;
        }
    }
    let end = run(c, k, |x| x.is_ascii_digit());
    if !(1..=5).contains(&(end - k)) || !ends_word(c, end) {
        return None;
    }
    let kept: String = c[i..k].iter().collect();
    Some((end, format!("{kept}‹port›")))
}

/// A port after a host's name or address: `localhost:4001`, `[…]:4001` (not a line of a
/// source file, `node.rs:120`).
fn port_after_host(c: &[char], i: usize) -> Option<(usize, String)> {
    if c[i] != ':' || i == 0 {
        return None;
    }
    let before = c[i - 1];
    if !(before.is_ascii_alphabetic() || before == ']' || before == '›') {
        return None;
    }
    let source = [".rs", ".dart"].iter().any(|ext| {
        let n = ext.chars().count();
        i >= n && c[i - n..i].iter().copied().eq(ext.chars())
    });
    if source {
        return None;
    }
    let end = run(c, i + 1, |x| x.is_ascii_digit());
    ((2..=5).contains(&(end - i - 1)) && ends_word(c, end)).then(|| (end, ":‹port›".into()))
}

/// The end of a path's characters from `k`.
fn path_end(c: &[char], k: usize) -> usize {
    run(c, k, |x| !x.is_whitespace() && !"\"<>,;)]".contains(x))
}

/// Windows paths: `C:\…`, `C:/…` and `\\server\…`.
fn windows_path(c: &[char], i: usize) -> Option<(usize, String)> {
    if c[i].is_ascii_alphabetic()
        && starts_word(c, i)
        && c.get(i + 1) == Some(&':')
        && matches!(c.get(i + 2), Some('\\' | '/'))
    {
        return Some((path_end(c, i + 3), "‹path›".into()));
    }
    if c[i] == '\\' && c.get(i + 1) == Some(&'\\') {
        let end = path_end(c, i + 2);
        if end > i + 2 {
            return Some((end, "‹path›".into()));
        }
    }
    None
}

/// Unix paths of two parts or more, and from the home folder (`~/…`); not multiaddrs.
fn unix_path(c: &[char], i: usize) -> Option<(usize, String)> {
    if i > 0 && (w(c[i - 1]) || "/.~‹›-".contains(c[i - 1])) {
        return None;
    }
    let part = |k: usize| run(c, k, |x| !x.is_whitespace() && !"/\"<>,;)]".contains(x));
    let mut k = match c[i] {
        '~' => i + 1,
        '/' => {
            let protocol = PROTOCOLS.iter().any(|p| {
                starts_with(c, i + 1, p)
                    && c.get(i + 1 + p.len())
                        .is_none_or(|x| *x == '/' || x.is_whitespace())
            });
            let end = part(i + 1);
            if protocol || end == i + 1 {
                return None;
            }
            end
        }
        _ => return None,
    };
    let mut parts = 0;
    while c.get(k) == Some(&'/') {
        k = part(k + 1);
        parts += 1;
    }
    (parts > 0).then(|| (k, "‹path›".into()))
}

/// Endings of host names on a local network, which are often a person's name
/// (`Martas-Laptop.local`).
const LOCAL_HOSTS: &[&str] = &[
    "local",
    "lan",
    "home",
    "internal",
    "localdomain",
    "home.arpa",
];

fn host_name(c: &[char], from: usize) -> Found {
    let host = |x: char| x.is_alphanumeric() || "-_.".contains(x);
    let mut i = from;
    while i < c.len() {
        if !host(c[i]) || (i > from && host(c[i - 1])) {
            i += 1;
            continue;
        }
        let end = run(c, i, host);
        let word: String = c[i..end].iter().collect();
        let word = word.trim_end_matches('.');
        let lower = word.to_lowercase();
        if LOCAL_HOSTS.iter().any(|h| {
            lower.len() > h.len() + 1
                && lower.ends_with(h)
                && lower.as_bytes()[lower.len() - h.len() - 1] == b'.'
        }) {
            return Some((i, i + word.chars().count(), "‹host›".into()));
        }
        i = end;
    }
    None
}

/// The endings of files people have, taken out wherever a name shows with one.
const FILE_ENDS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "avif", "bmp", "tif", "tiff", "svg",
    "ico", "raw", "dng", "cr2", "nef", "mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "mpg",
    "mpeg", "3gp", "mp3", "m4a", "wav", "flac", "ogg", "oga", "opus", "aac", "wma", "mid", "pdf",
    "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "md", "csv",
    "epub", "pages", "numbers", "json", "xml", "htm", "html", "yaml", "yml", "toml", "ini", "zip",
    "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "exe", "msi", "dmg", "pkg", "apk", "deb",
    "rpm", "appimage", "iso", "flatpak", "fogata", "log", "key", "pem", "ps1", "sh", "bat", "py",
    "js", "db", "sqlite", "bak", "tmp", "part", "ics", "vcf", "eml", "srt", "vtt", "torrent",
];

/// File names: what goes with an ending of [`FILE_ENDS`].
fn file_name(c: &[char], from: usize) -> Found {
    let class = |x: char| x.is_alphanumeric() || "_()+-.".contains(x);
    let mut i = from;
    while i < c.len() {
        if !class(c[i]) {
            i += 1;
            continue;
        }
        let end = run(c, i, class);
        // The last ending in the run makes the longest name, as a greedy match does.
        for dot in (i + 1..end).rev() {
            if c[dot] != '.' {
                continue;
            }
            let ext_end = run(c, dot + 1, char::is_alphanumeric);
            let ext: String = c[dot + 1..ext_end]
                .iter()
                .collect::<String>()
                .to_lowercase();
            if ext_end > dot + 1 && FILE_ENDS.contains(&ext.as_str()) {
                return Some((name_start(c, from, i), ext_end, "‹file›".into()));
            }
        }
        i = end;
    }
    None
}

/// Words a message says before a file's name, where the name cannot go on.
const BEFORE_NAMES: &[&str] = &[
    "a",
    "an",
    "and",
    "as",
    "at",
    "attach",
    "attached",
    "attachment",
    "big",
    "by",
    "cannot",
    "copied",
    "copy",
    "could",
    "decode",
    "decoding",
    "delete",
    "deleted",
    "download",
    "downloaded",
    "downloading",
    "drop",
    "dropped",
    "error",
    "failed",
    "fetch",
    "fetched",
    "fetching",
    "file",
    "for",
    "from",
    "got",
    "image",
    "in",
    "into",
    "is",
    "kept",
    "large",
    "message",
    "move",
    "moved",
    "named",
    "not",
    "of",
    "on",
    "open",
    "opened",
    "opening",
    "or",
    "photo",
    "picked",
    "picture",
    "read",
    "reading",
    "received",
    "remove",
    "removed",
    "save",
    "saved",
    "send",
    "sending",
    "sent",
    "share",
    "shared",
    "skipped",
    "the",
    "to",
    "too",
    "video",
    "voice",
    "was",
    "with",
    "write",
    "writing",
    "wrote",
];

/// Where the name of a file whose last word starts at `i` starts: names may hold spaces,
/// so the whole words before it (not before `from`) count, back to a word of the message
/// ([`BEFORE_NAMES`]) or anything else between them than one space.
fn name_start(c: &[char], from: usize, i: usize) -> usize {
    let class = |x: char| x.is_alphanumeric() || "_()+-.".contains(x);
    let mut start = i;
    while start >= from + 2 && c[start - 1] == ' ' && class(c[start - 2]) {
        let mut word = start - 1;
        while word > from && class(c[word - 1]) {
            word -= 1;
        }
        let whole = word == 0 || c[word - 1].is_whitespace();
        let text: String = c[word..start - 1].iter().collect::<String>().to_lowercase();
        if !whole || BEFORE_NAMES.contains(&text.as_str()) {
            break;
        }
        start = word;
    }
    start
}

fn base58(x: char) -> bool {
    x.is_ascii_alphanumeric() && !"0OIl".contains(x)
}

/// Full peer ids keep their last six characters.
fn peer_id(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) {
        return None;
    }
    let prefix = if starts_with(c, i, "12D3KooW") {
        8
    } else if starts_with(c, i, "Qm") {
        2
    } else {
        return None;
    };
    let end = run(c, i + prefix, base58);
    if !(38..=50).contains(&(end - i - prefix)) || !ends_word(c, end) {
        return None;
    }
    let tail: String = c[end - 6..end].iter().collect();
    Some((end, format!("…{tail}")))
}

/// A whole value that is a peer id, in its short form.
fn whole_peer(v: &str) -> Option<String> {
    let c: Vec<char> = v.chars().collect();
    peer_id(&c, 0).and_then(|(end, short)| (end == c.len()).then_some(short))
}

/// Device keys and other long hex keep their first six characters (after `0x`, when they
/// are written with it).
fn long_hex(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) {
        return None;
    }
    let prefix = if starts_with(c, i, "0x") || starts_with(c, i, "0X") {
        2
    } else {
        0
    };
    let digits = i + prefix;
    let end = run(c, digits, |x| x.is_ascii_hexdigit());
    if end - digits < 16 || !ends_word(c, end) {
        return None;
    }
    let head: String = c[i..digits + 6].iter().collect();
    Some((end, format!("{head}…")))
}

fn crockford(x: char) -> bool {
    x.is_ascii_digit() || (x.is_ascii_uppercase() && !"ILOU".contains(x))
}

/// A fingerprint in groups, `7Q4M K2XR 9PLD 3VHC`, in its short form.
fn grouped_fingerprint(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) {
        return None;
    }
    let mut k = i;
    for group in 0..4 {
        if run(c, k, crockford) - k < 4 {
            return None;
        }
        k += 4;
        if group < 3 {
            if c.get(k) != Some(&' ') {
                return None;
            }
            k += 1;
        }
    }
    if !ends_word(c, k) {
        return None;
    }
    let first: String = c[i..i + 4].iter().collect();
    let second: String = c[i + 5..i + 9].iter().collect();
    Some((k, format!("{first}·{second}")))
}

/// A fingerprint, `7Q4MK2XR9PLD3VHC`, in its short form.
fn fingerprint(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) {
        return None;
    }
    let end = run(c, i, crockford);
    if end - i != 16 || !ends_word(c, end) {
        return None;
    }
    let f = &c[i..end];
    let mixed = f.iter().any(char::is_ascii_digit) && f.iter().any(char::is_ascii_uppercase);
    if !mixed {
        return None;
    }
    let first: String = f[..4].iter().collect();
    let second: String = f[4..8].iter().collect();
    Some((end, format!("{first}·{second}")))
}

/// Codes: `FGT-7Q4MK-2XR9P` and the like, in any case for friend codes.
fn code(c: &[char], i: usize) -> Option<(usize, String)> {
    if !starts_word(c, i) {
        return None;
    }
    if starts_with_ignoring_case(c, i, "fgt-") {
        let end = run(c, i + 4, |x| x.is_alphanumeric() || x == '-');
        return Some((end, "‹code›".into()));
    }
    // A move's code typed without its dash: ten characters of Crockford base32, digits and
    // letters of one case.
    let end = run(c, i, |x| crockford(x.to_ascii_uppercase()));
    if end - i == 10 && ends_word(c, end) {
        let t = &c[i..end];
        let digit = t.iter().any(char::is_ascii_digit);
        let one_case =
            t.iter().all(|x| !x.is_ascii_lowercase()) || t.iter().all(|x| !x.is_ascii_uppercase());
        if digit && one_case && t.iter().any(char::is_ascii_alphabetic) {
            return Some((end, "‹code›".into()));
        }
    }
    let upper = |x: char| x.is_ascii_digit() || x.is_ascii_uppercase();
    let first = run(c, i, upper);
    if first - i < 3 {
        return None;
    }
    let mut k = first;
    let mut best = None;
    while c.get(k) == Some(&'-') {
        let end = run(c, k + 1, upper);
        if end - k - 1 < 4 {
            break;
        }
        k = end;
        if ends_word(c, k) {
            best = Some(k);
        }
    }
    best.map(|end| (end, "‹code›".into()))
}

/// Keys and other long runs of base64, base58 or base32: a run of 24 characters or more
/// with digits, capitals and small letters; or of digits and letters of one case, with at
/// least four of each; or 16 bytes in base64 (22 characters).
fn key(c: &[char], from: usize) -> Found {
    let class = |x: char| x.is_ascii_alphanumeric() || "+/_-".contains(x);
    let mut i = from;
    while i < c.len() {
        if !class(c[i]) {
            i += 1;
            continue;
        }
        let mut end = run(c, i, class);
        let t = &c[i..end];
        let digits = t.iter().filter(|x| x.is_ascii_digit()).count();
        let upper = t.iter().filter(|x| x.is_ascii_uppercase()).count();
        let lower = t.iter().filter(|x| x.is_ascii_lowercase()).count();
        let padded = c.get(end) == Some(&'=');
        let random = (t.len() >= 24
            && digits > 0
            && ((upper > 0 && lower > 0)
                || (digits >= 4 && (upper >= 4 || lower >= 4) && !t.contains(&'_'))))
            // 16 bytes in base64 (22 characters, and its padding when it has one).
            || ((22..24).contains(&t.len())
                && digits > 0
                && upper >= 2
                && lower >= 2
                && (padded || !t.contains(&'_')));
        if random {
            while end < c.len() && end - i < t.len() + 2 && c[end] == '=' {
                end += 1;
            }
            return Some((i, end, "‹key›".into()));
        }
        i = end;
    }
    None
}

/// Lists of bytes as Rust prints them (`[12, 200, 7, …]`), which may be a key.
fn bytes(c: &[char], i: usize) -> Option<(usize, String)> {
    if c[i] != '[' {
        return None;
    }
    let mut k = i + 1;
    let mut count = 0;
    loop {
        k = run(c, k, |x| x == ' ');
        let end = run(c, k, |x| x.is_ascii_digit());
        if !(1..=3).contains(&(end - k)) {
            return None;
        }
        count += 1;
        k = run(c, end, |x| x == ' ');
        match c.get(k) {
            Some(',') => k += 1,
            Some(']') if count >= 8 => return Some((k + 1, "[‹bytes›]".into())),
            _ => return None,
        }
    }
}

/// Last, any word still holding a slash or a backslash becomes a path, unless it is a
/// fraction or a date (`3/4`) or a multiaddr left with marks only.
fn slashed_words(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(&slashed_word(word));
        word.clear();
    };
    for ch in s.chars() {
        if ch.is_whitespace() {
            flush(&mut word, &mut out);
            out.push(ch);
        } else {
            word.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn slashed_word(word: &str) -> String {
    let core = word
        .trim_start_matches(['(', '[', '{', '<', '\'', '"'])
        .trim_end_matches([')', ']', '}', '>', ',', ';', ':', '.', '!', '?', '\'', '"']);
    if !core.contains(['/', '\\']) || core == "/" {
        return word.to_owned();
    }
    let numbers = core
        .chars()
        .all(|x| x.is_ascii_digit() || "/.-".contains(x));
    let marked_multiaddr = core.starts_with('/')
        && core.split('/').all(|p| {
            p.is_empty()
                || PROTOCOLS.contains(&p)
                || p.chars().all(|x| x.is_ascii_digit())
                || (p.starts_with('‹') && p.ends_with('›'))
                || (p.starts_with('…') && p.chars().count() == 7)
        });
    if numbers || marked_multiaddr {
        return word.to_owned();
    }
    let at = word.find(core).unwrap_or(0);
    format!("{}‹path›{}", &word[..at], &word[at + core.len()..])
}

// ----- Field values -----

/// A number (with a unit of time, size or share), a yes or no.
fn plain(v: &str) -> bool {
    if v == "true" || v == "false" {
        return true;
    }
    let number = v.trim_start_matches('-');
    let digits = number.trim_end_matches(|x: char| x.is_alphabetic() || x == '%');
    let unit = &number[digits.len()..];
    let mut parts = digits.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
        && parts.next().is_none()
        && [
            "", "ns", "µs", "us", "ms", "s", "m", "h", "d", "B", "KiB", "MiB", "GiB", "kB", "MB",
            "GB", "%",
        ]
        .contains(&unit)
}

/// One word of the code: an enum's variant, a state.
fn is_identifier(v: &str) -> bool {
    let mut chars = v.chars();
    chars
        .next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && chars.all(|x| x.is_ascii_alphanumeric() || x == '_')
        && v.len() <= 32
}

/// A multiaddr as its protocols, with marks in place of every value.
fn multiaddr_shape(v: &str) -> Option<String> {
    let mut parts = v.strip_prefix('/')?.split('/');
    let first = parts.clone().next()?;
    if !ADDRESSED.contains(&first) && first != "p2p" && first != "p2p-circuit" {
        return None;
    }
    let mut out = String::new();
    while let Some(proto) = parts.next() {
        if !PROTOCOLS.contains(&proto) {
            out.push_str("/‹…›");
            continue;
        }
        out.push('/');
        out.push_str(proto);
        let takes_value =
            ADDRESSED.contains(&proto) || ["udp", "tcp", "p2p", "certhash"].contains(&proto);
        if takes_value && let Some(value) = parts.next() {
            out.push('/');
            out.push_str(&match proto {
                "udp" | "tcp" => "‹port›".to_owned(),
                "certhash" => "‹key›".to_owned(),
                "p2p" => whole_peer(value).unwrap_or_else(|| "‹key›".into()),
                _ => "‹ip›".to_owned(),
            });
        }
    }
    Some(out)
}

/// Words a field may keep after scrubbing, four at most: small words of the code or of a
/// sentence, numbers, marks and short ids.
fn plain_words(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    words.len() <= 4
        && words.iter().all(|word| {
            let word = word.trim_end_matches([',', ';', ':', '.']);
            plain(word)
                || word
                    .chars()
                    .all(|x| x.is_ascii_lowercase() || x == '-' || x == '_')
                || (word.starts_with('‹') && word.ends_with('›'))
                || ((6..=8).contains(&word.chars().count())
                    && word.chars().all(base58)
                    && word
                        .chars()
                        .any(|x| x.is_ascii_digit() || x.is_ascii_uppercase()))
                || (word.starts_with('…') && word.chars().count() == 7)
                || (word.ends_with('…') && word.chars().count() == 7)
                || (word.chars().count() == 9 && word.chars().nth(4) == Some('·'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Known {
        Known::new(
            ["Leo", "Marta", "The usual crew", "Gaming", "X"].map(String::from),
            [
                "/home/marta/.local/share/fogata".into(),
                "/home/marta".into(),
            ],
        )
    }

    fn m(text: &str) -> String {
        message(text, &known())
    }

    #[test]
    fn quotes_links_and_mail() {
        assert_eq!(
            m(r#"said "hi, \"Leo\"" and 'x y'"#),
            "said \"‹text›\" and '‹text›'"
        );
        assert_eq!(m("can't stop it’s fine"), "can't stop it’s fine");
        assert_eq!(m("cut \"hi Leo, coming\nnext"), "cut \"‹text›\nnext");
        assert_eq!(m("chat “coming?” failed"), "chat “‹text›” failed");
        assert_eq!(
            m("open https://x.example/a?b=c and fogata:friend:AbC9 now"),
            "open ‹link› and ‹link› now"
        );
        assert_eq!(m("from josé.r+1@mail.example.org."), "from ‹email›.");
    }

    #[test]
    fn names() {
        assert_eq!(
            m("Leo and LEO and Leonardo"),
            "‹name› and ‹name› and Leonardo"
        );
        assert_eq!(m("the usual crew: 3"), "‹name›: 3");
        assert_eq!(
            m("Bea Ruiz asks to join a group"),
            "‹name› asks to join a group"
        );
        assert_eq!(m("joined the group The crew"), "joined the group ‹name›");
        assert_eq!(
            m("group crew (2) not loaded: bad"),
            "group ‹name› not loaded: bad"
        );
        assert_eq!(
            m("cannot run group Café again: x"),
            "cannot run group ‹name› again: x"
        );
    }

    #[test]
    fn addresses() {
        assert_eq!(
            m(
                "/ip4/203.0.113.7/udp/40123/quic-v1/p2p/12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A"
            ),
            "/ip4/‹ip›/udp/‹port›/quic-v1/p2p/…VtXk3A"
        );
        assert_eq!(
            m("dial 192.168.1.20:4001, [fe80::1c2b:3cff:fe4d:5e6f%eth0]:4001 and ::1 failed"),
            "dial ‹ip›, ‹ip› and ‹ip› failed"
        );
        assert_eq!(m("/dns4/a1b2.fogata/tcp/80"), "/dns4/‹ip›/tcp/‹port›");
        assert_eq!(
            m("port 51820 and port: 7 and localhost:8080"),
            "port ‹port› and port: ‹port› and localhost:‹port›"
        );
        assert_eq!(
            m("took 12:30:45 at src/node.rs:120"),
            "took 12:30:45 at ‹path›"
        );
        assert_eq!(m("ether aa:bb:cc:dd:ee:ff"), "ether ‹ip›");
        assert_eq!(m("seen from Martas-Laptop.local."), "seen from ‹host›.");
    }

    #[test]
    fn paths_and_files() {
        assert_eq!(
            m("cannot remove /home/marta/.local/share/fogata/direct/x: busy"),
            "cannot remove ‹path›: busy"
        );
        assert_eq!(
            m("cannot read /home/bea perez/notes/a: x"),
            "cannot read ‹path› ‹path›: x"
        );
        assert_eq!(
            m(r"C:\Users\Bea Pérez\Downloads\final report.pdf in use"),
            "‹path› ‹path› ‹file› in use"
        );
        assert_eq!(
            m("shared birthday-photo.PNG and beach holiday.jpg"),
            "shared ‹file› and ‹file›"
        );
        assert_eq!(m("~/photos/a and and/or 3/4"), "‹path› and ‹path› 3/4");
    }

    /// What slipped through before: names of files with spaces, 16-byte keys in base64,
    /// hex written with 0x, move codes typed without their dash.
    #[test]
    fn short_keys_spaced_names_and_bare_codes() {
        assert_eq!(
            m("shared beach holiday.jpg and Screenshot 2.png"),
            "shared ‹file› and ‹file›"
        );
        assert_eq!(
            m("cannot open final trip report.pdf: busy"),
            "cannot open ‹file›: busy"
        );
        assert_eq!(m("token q83vEjRWeJCrze8SNFZ4mA== end"), "token ‹key› end");
        assert_eq!(m("token q83vEjRWeJCrze8SNFZ4mA end"), "token ‹key› end");
        assert_eq!(
            m("secret 0x9f86d081884c7d659a2feaa0c55ad015 ok"),
            "secret 0x9f86d0… ok"
        );
        assert_eq!(
            m("code 7Q4MK2XR9P and 7q4mk2xr9p"),
            "code ‹code› and ‹code›"
        );
        // Words of the code stay.
        assert_eq!(
            m("ConnectionHandler upgrade at 0x10 took 1234567890 ms"),
            "ConnectionHandler upgrade at 0x10 took 1234567890 ms"
        );
        assert_eq!(
            m("Ed25519PublicKey from libp2p_swarm"),
            "Ed25519PublicKey from libp2p_swarm"
        );
    }

    #[test]
    fn ids_codes_and_keys() {
        assert_eq!(
            m("device b1946ac92492d2347c6235b4d2611184e2b6a1bda1c0e2c8a1f9d5c3b4a2e1f0 ok"),
            "device b1946a… ok"
        );
        assert_eq!(
            m("friend 7Q4MK2XR9PMD3VHC and 7Q4M K2XR 9PMD 3VHC"),
            "friend 7Q4M·K2XR and 7Q4M·K2XR"
        );
        assert_eq!(
            m("code FGT-7Q4MK-2XR9P or fgt-7q4mk-2xr9p"),
            "code ‹code› or ‹code›"
        );
        assert_eq!(
            m("key kQ7vR2xN9pL4mW8sT1zY6bC3dF5gH0jK2nP7qR9sU4w= end"),
            "key ‹key› end"
        );
        assert_eq!(
            m("cid bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"),
            "cid ‹key›"
        );
        assert_eq!(m("log [1, 2, 3, 4, 5, 6, 7, 200]"), "log [‹bytes›]");
        assert_eq!(
            m("libp2p_swarm said build 0.1.0-beta.3"),
            "libp2p_swarm said build 0.1.0-beta.3"
        );
    }

    #[test]
    fn fields() {
        let k = known();
        assert_eq!(field("n", "42", &k), "42");
        assert_eq!(field("took", "1.25s", &k), "1.25s");
        assert_eq!(field("hidden", "false", &k), "false");
        assert_eq!(field("kind", "HolePunched", &k), "HolePunched");
        assert_eq!(field("who", "Leo", &k), "‹name›");
        assert_eq!(field("who", "Bea Ruiz", &k), "‹value›");
        assert_eq!(
            field("path", "through member Ab3dEfGh", &k),
            "through member Ab3dEfGh"
        );
        assert_eq!(field("path", "local network", &k), "local network");
        assert_eq!(
            field(
                "peer",
                "12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A",
                &k
            ),
            "…VtXk3A"
        );
        assert_eq!(
            field(
                "address",
                "/ip6/2001:db8::42/udp/4001/quic-v1/p2p/12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A",
                &k
            ),
            "/ip6/‹ip›/udp/‹port›/quic-v1/p2p/…VtXk3A"
        );
        assert_eq!(field("signal", "Ring { log: [[1, 2]] }", &k), "Ring {‹…›}");
        assert_eq!(
            field("signal", "Offer { sdp: \"v=0 IN IP4 203.0.113.7\" }", &k),
            "Offer {‹…›}"
        );
        assert_eq!(field("n", "Some(3)", &k), "Some(3)");
        assert_eq!(field("id", "PeerId(\"x\")", &k), "PeerId(‹…›)");
        assert_eq!(
            field("error", "connection to 192.168.1.20:4001 refused", &k),
            "connection to ‹ip› refused"
        );
        assert_eq!(field("text", "see you at 8, Leo", &k), "‹value›");
    }
}
