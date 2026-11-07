//! What a network report means for reaching friends, in plain words (`fogata network` and
//! the app's network diagnosis).

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use fogata_net::{Multiaddr, NetworkReport, UdpSeen, multiaddr::Protocol};

/// How a finding reads: good news, a note, or a problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Good,
    Note,
    Bad,
}

/// What a finding is about, for screens that word it in their own language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Offline,
    /// The number is the DHT peers known.
    Internet(usize),
    /// `text` carries the system's note.
    Dns,
    UdpWorks,
    UdpBlocked,
    CarrierNat,
    PublicIpv4,
    HardNat,
    EasyNat,
    UnknownRouter,
    NoIpv6,
    Ipv6Works,
    Ipv6Unreached,
    /// The number is the devices found.
    LanDevices(usize),
    /// This computer's own firewall lets nothing in for Fogata; `text` ends with the
    /// command that does.
    Firewall,
    Summary(Verdict),
}

/// What the findings mean for connecting to friends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    NoUdp,
    Public,
    EasyNat,
    HardNatWithIpv6,
    HardNat,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub level: Level,
    pub topic: Topic,
    pub text: String,
}

impl Finding {
    /// A short mark for text output.
    pub fn mark(&self) -> &'static str {
        match self.level {
            Level::Good => "[ok]",
            Level::Note => "[--]",
            Level::Bad => "[!!]",
        }
    }
}

fn ip_of(address: &Multiaddr) -> Option<IpAddr> {
    address.iter().find_map(|p| match p {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    })
}

fn is_global_v6(ip: &Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !ip.is_loopback()
        && !ip.is_unspecified()
        && (first & 0xfe00) != 0xfc00
        && (first & 0xffc0) != 0xfe80
}

fn is_carrier_nat(ip: &Ipv4Addr) -> bool {
    ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64
}

/// How recent a contact with the internet must be for this device's side to count as fine:
/// live connections answer pings every 15 s.
pub const FRESH_CONTACT: Duration = Duration::from_secs(60);

/// Dials to public addresses that must fail, with nothing answering, before the internet
/// counts as out of reach: a device that just had nothing to do dialled nobody.
pub const OFFLINE_AFTER: usize = 2;

/// Whether UDP gets out of this network now (the node saw a QUIC connection with a public
/// address answer lately).
fn udp_works(report: &NetworkReport) -> bool {
    report.udp == UdpSeen::Works
}

/// A peer at a public address connected or answered a moment ago.
fn fresh_contact(report: &NetworkReport) -> bool {
    report.contact.is_some_and(|age| age <= FRESH_CONTACT)
}

/// Nothing on the internet answers: the node has been on this network a while, nobody at a
/// public address answered lately, and its dials there failed.
fn offline(report: &NetworkReport) -> bool {
    report.settled && !fresh_contact(report) && report.failed_dials >= OFFLINE_AFTER
}

/// Whether IPv6 goes around a router here: this device has a public IPv6 address and peers
/// reached it there. Both the check and the causes use this one criterion.
fn ipv6_works(report: &NetworkReport) -> bool {
    let has = report
        .listening
        .iter()
        .filter_map(ip_of)
        .any(|ip| matches!(ip, IpAddr::V6(v6) if is_global_v6(&v6)));
    has && report
        .observed
        .iter()
        .any(|(a, _)| matches!(ip_of(a), Some(IpAddr::V6(_))))
}

/// Something on this device's side that keeps others from reaching it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Nothing on the internet answers: no connection, or a firewall lets nothing out.
    Offline,
    /// UDP does not get out (this network or this computer blocks it): no hole punching
    /// and no calls through routers.
    UdpBlocked,
    /// The router gives a new public port to every destination (a symmetric NAT): nobody
    /// behind another router can open a way to it.
    SymmetricNat,
    /// No working IPv6 either, which would have gone around that router.
    NoIpv6,
}

/// What this device's side tells about reaching friends and members, from what the node has
/// seen while running: nothing is guessed, so a cause is only there once the node saw it,
/// and `fine` only while it sees that nothing is in the way.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    /// The most telling first.
    pub causes: Vec<Cause>,
    /// A peer on the internet answered a moment ago, UDP works now, and the router, if any,
    /// can be opened from both sides (or there is none).
    pub fine: bool,
}

/// What keeps others from reaching this device, as far as `report` shows. This computer's
/// own firewall is not in it: the core reads that apart (`Core::firewall_command`).
pub fn reach(report: &NetworkReport) -> Reach {
    if offline(report) {
        return Reach {
            causes: vec![Cause::Offline],
            fine: false,
        };
    }
    let mut causes = Vec::new();
    let udp = udp_works(report);
    if !udp && report.udp == UdpSeen::Blocked {
        causes.push(Cause::UdpBlocked);
    }
    let local: Vec<IpAddr> = report.listening.iter().filter_map(ip_of).collect();
    let public_v4 = report
        .observed
        .iter()
        .any(|(a, _)| matches!(ip_of(a), Some(ip @ IpAddr::V4(_)) if local.contains(&ip)));
    if report.port_changes && !public_v4 {
        causes.push(Cause::SymmetricNat);
        // IPv6 matters here: with it, friends who have it too go around the router.
        if !ipv6_works(report) {
            causes.push(Cause::NoIpv6);
        }
    }
    let easy = !report.port_changes
        && report
            .confirmed
            .iter()
            .any(|a| matches!(ip_of(a), Some(IpAddr::V4(_))));
    Reach {
        fine: causes.is_empty() && fresh_contact(report) && udp && (public_v4 || easy),
        causes,
    }
}

/// Plain-words findings, ending with what they mean for connecting to friends.
pub fn diagnose(report: &NetworkReport) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut say = |level, topic, text: String| out.push(Finding { level, topic, text });

    // Only a peer that answered on this network counts: the DHT's table fills from the
    // bootstrap list alone.
    if report.contact.is_none() {
        if offline(report) {
            say(
                Level::Bad, Topic::Offline,
                "No peer of the public DHT answered: this device does not seem to reach the internet, or a firewall blocks Fogata.".into(),
            );
        } else {
            say(
                Level::Note,
                Topic::Summary(Verdict::Unknown),
                "Summary: not enough information; run the check again in a minute.".into(),
            );
        }
        return out;
    }
    say(
        Level::Good,
        Topic::Internet(report.dht_peers),
        format!(
            "Internet: reached the public DHT ({} peers known).",
            report.dht_peers
        ),
    );
    if let Some(note) = &report.dns_note {
        say(Level::Note, Topic::Dns, format!("DNS: {note}."));
    }

    let udp = udp_works(report);
    let blocked = !udp && report.udp == UdpSeen::Blocked;
    if udp {
        say(
            Level::Good,
            Topic::UdpWorks,
            "UDP works. Calls and getting through routers need it.".into(),
        );
    } else if blocked {
        say(
            Level::Bad, Topic::UdpBlocked,
            "UDP seems blocked by this network or this computer (peers that answered over TCP did not over QUIC). Chat can still go through members, but direct connections through routers and calls will mostly fail. Check the firewall or the network's rules.".into(),
        );
    }

    let local: Vec<IpAddr> = report.listening.iter().filter_map(ip_of).collect();
    let seen_v4: Vec<Ipv4Addr> = report
        .observed
        .iter()
        .filter_map(|(a, _)| match ip_of(a) {
            Some(IpAddr::V4(ip)) => Some(ip),
            _ => None,
        })
        .collect();
    let public_v4 = seen_v4.iter().any(|ip| local.contains(&IpAddr::V4(*ip)));
    let confirmed_v4 = report
        .confirmed
        .iter()
        .any(|a| matches!(ip_of(a), Some(IpAddr::V4(_))));
    if local
        .iter()
        .any(|ip| matches!(ip, IpAddr::V4(v4) if is_carrier_nat(v4)))
    {
        say(
            Level::Note, Topic::CarrierNat,
            "This device has an address of a carrier-grade NAT (100.64.0.0/10): the provider shares one public address among many customers.".into(),
        );
    }

    #[derive(PartialEq)]
    enum Router {
        None,
        Easy,
        Hard,
        Unknown,
    }
    let router = if public_v4 {
        say(
            Level::Good,
            Topic::PublicIpv4,
            "This device has a public IPv4 address: others can reach it directly.".into(),
        );
        Router::None
    } else if report.port_changes {
        say(
            Level::Note,
            Topic::HardNat,
            "Behind a router that gives a different public port for each destination (a hard NAT)."
                .into(),
        );
        Router::Hard
    } else if confirmed_v4 {
        say(
            Level::Good, Topic::EasyNat,
            "Behind a router that keeps the same public port for every destination (an easy NAT): it can be opened from both sides at once.".into(),
        );
        Router::Easy
    } else {
        say(
            Level::Note, Topic::UnknownRouter,
            "Too few peers reported this device's public address to tell what kind of router this is.".into(),
        );
        Router::Unknown
    };

    let v6: Vec<&Ipv6Addr> = local
        .iter()
        .filter_map(|ip| match ip {
            IpAddr::V6(v6) if is_global_v6(v6) => Some(v6),
            _ => None,
        })
        .collect();
    // The same criterion as the causes' (`reach`).
    let v6_seen = ipv6_works(report);
    if v6.is_empty() {
        say(
            Level::Note, Topic::NoIpv6,
            "No public IPv6 address. Fogata does not need it, but it makes direct connections easier.".into(),
        );
    } else if v6_seen {
        say(
            Level::Good,
            Topic::Ipv6Works,
            "Public IPv6 works: friends with IPv6 can connect without any router in the way."
                .into(),
        );
    } else {
        say(
            Level::Note,
            Topic::Ipv6Unreached,
            "This device has a public IPv6 address, but no peer reached it over IPv6 yet.".into(),
        );
    }

    if report.lan_devices > 0 {
        say(
            Level::Good,
            Topic::LanDevices(report.lan_devices),
            format!(
                "{} other device(s) found on this local network; friends here connect without going through the internet.",
                report.lan_devices
            ),
        );
    }

    let (verdict, text) = match router {
        _ if blocked => (
            Verdict::NoUdp,
            "Summary: expect trouble with direct connections until UDP is allowed.",
        ),
        Router::None => (
            Verdict::Public,
            "Summary: friends should be able to reach this device directly.",
        ),
        Router::Easy => (
            Verdict::EasyNat,
            "Summary: direct connections should work with most friends, through both routers at once; with a friend behind a hard NAT, a group member in between is needed.",
        ),
        Router::Hard if v6_seen => (
            Verdict::HardNatWithIpv6,
            "Summary: friends with IPv6 or a public address connect directly; with the others, a group member in between is needed.",
        ),
        Router::Hard => (
            Verdict::HardNat,
            "Summary: only friends with a public address connect directly; with the others, a group member in between is needed.",
        ),
        Router::Unknown => (
            Verdict::Unknown,
            "Summary: not enough information; run the check again in a minute.",
        ),
    };
    say(Level::Note, Topic::Summary(verdict), text.into());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(text: &str) -> Multiaddr {
        text.parse().unwrap()
    }

    fn texts(report: &NetworkReport) -> String {
        diagnose(report)
            .iter()
            .map(|f| f.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A peer at a public address answered a moment ago.
    fn answered() -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    #[test]
    fn offline() {
        let report = NetworkReport {
            settled: true,
            failed_dials: OFFLINE_AFTER,
            // The bootstrap list fills the table without anyone answering.
            dht_peers: 7,
            ..Default::default()
        };
        let text = texts(&report);
        assert!(
            text.contains("does not seem to reach the internet"),
            "{text}"
        );
        assert_eq!(reach(&report).causes, [Cause::Offline]);
    }

    #[test]
    fn easy_nat_without_ipv6() {
        let report = NetworkReport {
            listening: vec![addr("/ip4/192.168.1.20/udp/4001/quic-v1")],
            observed: vec![(addr("/ip4/203.0.113.5/udp/4001/quic-v1"), 7)],
            confirmed: vec![addr("/ip4/203.0.113.5/udp/4001/quic-v1")],
            dht_peers: 40,
            quic_connections: 5,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        let text = texts(&report);
        assert!(text.contains("UDP works"));
        assert!(text.contains("easy NAT"));
        assert!(text.contains("No public IPv6"));
        assert!(text.contains("should work with most friends"));
        let topics: Vec<Topic> = diagnose(&report).iter().map(|f| f.topic).collect();
        assert!(topics.contains(&Topic::Internet(40)));
        assert!(topics.contains(&Topic::EasyNat));
        assert_eq!(topics.last(), Some(&Topic::Summary(Verdict::EasyNat)));
    }

    #[test]
    fn hard_nat_with_carrier_nat_and_ipv6() {
        let report = NetworkReport {
            listening: vec![
                addr("/ip4/100.70.1.2/udp/4001/quic-v1"),
                addr("/ip6/2001:db8::5/udp/4001/quic-v1"),
            ],
            observed: vec![
                (addr("/ip4/203.0.113.5/udp/50001/quic-v1"), 1),
                (addr("/ip4/203.0.113.5/udp/50002/quic-v1"), 1),
                (addr("/ip6/2001:db8::5/udp/4001/quic-v1"), 2),
            ],
            port_changes: true,
            dht_peers: 30,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        let text = texts(&report);
        assert!(text.contains("carrier-grade NAT"));
        assert!(text.contains("hard NAT"));
        assert!(text.contains("Public IPv6 works"));
        assert!(text.contains("friends with IPv6"));
        // The causes say the same of IPv6: it goes around the router, so it is not missing.
        assert_eq!(reach(&report).causes, [Cause::SymmetricNat]);
    }

    #[test]
    fn public_address_and_blocked_udp() {
        let public = NetworkReport {
            listening: vec![addr("/ip4/203.0.113.9/udp/4001/quic-v1")],
            observed: vec![(addr("/ip4/203.0.113.9/udp/4001/quic-v1"), 3)],
            dht_peers: 10,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        assert!(texts(&public).contains("reach this device directly"));
        let blocked = NetworkReport {
            dht_peers: 10,
            tcp_connections: 4,
            udp: UdpSeen::Blocked,
            contact: answered(),
            ..Default::default()
        };
        let text = texts(&blocked);
        assert!(text.contains("UDP seems blocked by this network or this computer"));
        assert!(text.contains("until UDP is allowed"));
    }

    #[test]
    fn only_tcp_so_far_is_not_called_blocked_udp() {
        // A DHT node may answer only over TCP: without the node's check, nothing is said.
        let report = NetworkReport {
            dht_peers: 10,
            tcp_connections: 4,
            contact: answered(),
            ..Default::default()
        };
        let text = texts(&report);
        assert!(!text.contains("UDP"), "{text}");
        let topics: Vec<Topic> = diagnose(&report).iter().map(|f| f.topic).collect();
        assert_eq!(topics.last(), Some(&Topic::Summary(Verdict::Unknown)));
        assert_eq!(reach(&report), Reach::default());
    }

    #[test]
    fn nothing_answering_is_offline_only_once_settled_and_tried() {
        let tried = NetworkReport {
            failed_dials: 5,
            ..Default::default()
        };
        // Right after starting or moving: nothing yet.
        assert_eq!(reach(&tried), Reach::default());
        assert!(!texts(&tried).contains("internet"));
        // A while on the network, but nothing dialled: nothing known either.
        let idle = NetworkReport {
            settled: true,
            failed_dials: 1,
            contact: Some(Duration::from_secs(600)),
            ..Default::default()
        };
        assert_eq!(reach(&idle), Reach::default());
        let settled = NetworkReport {
            settled: true,
            ..tried.clone()
        };
        assert_eq!(reach(&settled).causes, [Cause::Offline]);
        assert!(!reach(&settled).fine);
        // An answer a moment ago says otherwise.
        let answered = NetworkReport {
            contact: answered(),
            ..settled
        };
        assert!(!reach(&answered).causes.contains(&Cause::Offline));
    }

    #[test]
    fn blocked_udp_is_a_cause() {
        let report = NetworkReport {
            dht_peers: 10,
            tcp_connections: 4,
            udp: UdpSeen::Blocked,
            settled: true,
            contact: answered(),
            ..Default::default()
        };
        assert_eq!(
            reach(&report),
            Reach {
                causes: vec![Cause::UdpBlocked],
                fine: false
            }
        );
    }

    #[test]
    fn symmetric_nat_names_missing_ipv6_only_then() {
        let hard = NetworkReport {
            listening: vec![addr("/ip4/192.168.1.20/udp/4001/quic-v1")],
            observed: vec![
                (addr("/ip4/203.0.113.5/udp/50001/quic-v1"), 2),
                (addr("/ip4/203.0.113.5/udp/50002/quic-v1"), 2),
            ],
            port_changes: true,
            dht_peers: 30,
            quic_connections: 3,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        assert_eq!(
            reach(&hard),
            Reach {
                causes: vec![Cause::SymmetricNat, Cause::NoIpv6],
                fine: false
            }
        );
        // A public IPv6 address nobody reached does not go around the router yet, as the
        // check says too.
        let unreached = NetworkReport {
            listening: vec![
                addr("/ip4/192.168.1.20/udp/4001/quic-v1"),
                addr("/ip6/2001:db8::5/udp/4001/quic-v1"),
            ],
            ..hard.clone()
        };
        assert_eq!(
            reach(&unreached).causes,
            [Cause::SymmetricNat, Cause::NoIpv6]
        );
        assert!(texts(&unreached).contains("no peer reached it over IPv6"));
        let mut observed = hard.observed.clone();
        observed.push((addr("/ip6/2001:db8::5/udp/4001/quic-v1"), 2));
        let reached = NetworkReport {
            observed,
            ..unreached
        };
        assert_eq!(reach(&reached).causes, [Cause::SymmetricNat]);
    }

    #[test]
    fn fine_takes_udp_working_now_and_an_answer_a_moment_ago() {
        let easy = NetworkReport {
            listening: vec![addr("/ip4/192.168.1.20/udp/4001/quic-v1")],
            observed: vec![(addr("/ip4/203.0.113.5/udp/4001/quic-v1"), 7)],
            confirmed: vec![addr("/ip4/203.0.113.5/udp/4001/quic-v1")],
            dht_peers: 40,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        assert_eq!(
            reach(&easy),
            Reach {
                causes: Vec::new(),
                fine: true
            }
        );
        let public = NetworkReport {
            listening: vec![addr("/ip4/203.0.113.9/udp/4001/quic-v1")],
            observed: vec![(addr("/ip4/203.0.113.9/udp/4001/quic-v1"), 1)],
            dht_peers: 10,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        assert!(reach(&public).fine);
        // The same addresses seen long ago, with nothing answering since (the line went
        // down, a VPN took every connection): not fine.
        let stale = NetworkReport {
            contact: Some(FRESH_CONTACT + Duration::from_secs(1)),
            ..easy.clone()
        };
        assert_eq!(reach(&stale), Reach::default());
        // UDP not seen working lately: not fine either.
        let no_udp_lately = NetworkReport {
            udp: UdpSeen::Unknown,
            ..easy
        };
        assert_eq!(reach(&no_udp_lately), Reach::default());
        // UDP works, but too few peers saw the address to tell the router: not fine yet.
        let unknown = NetworkReport {
            listening: vec![addr("/ip4/192.168.1.20/udp/4001/quic-v1")],
            dht_peers: 10,
            udp: UdpSeen::Works,
            contact: answered(),
            ..Default::default()
        };
        assert_eq!(reach(&unknown), Reach::default());
    }
}
