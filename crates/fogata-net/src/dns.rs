//! DNS settings used to resolve bootstrap addresses.
//!
//! Some systems publish name servers that the resolver library cannot read as a whole. For
//! example, a phone hotspot on macOS announces a link-local IPv6 server with a zone
//! (`fe80::1%en1`), which makes reading the system settings fail. A bad entry must never stop
//! the node: the readable servers are used, and without any the node still starts and uses
//! bootstrap peers given as plain IP addresses.

use std::net::IpAddr;

use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};

pub(crate) struct DnsSettings {
    pub(crate) config: ResolverConfig,
    pub(crate) opts: ResolverOpts,
    /// Explains a fallback, for the user.
    pub(crate) note: Option<String>,
}

pub(crate) fn system_settings() -> DnsSettings {
    settings_from(
        hickory_resolver::system_conf::read_system_conf().map_err(|e| e.to_string()),
        || std::fs::read_to_string("/etc/resolv.conf").ok(),
    )
}

fn settings_from(
    system: Result<(ResolverConfig, ResolverOpts), String>,
    resolv_conf: impl FnOnce() -> Option<String>,
) -> DnsSettings {
    match system {
        Ok((config, opts)) => DnsSettings {
            config,
            opts,
            note: None,
        },
        Err(error) => {
            let servers = resolv_conf()
                .map(|text| nameservers(&text))
                .unwrap_or_default();
            let note = if servers.is_empty() {
                format!(
                    "the system DNS settings could not be read ({error}); only bootstrap peers with plain IP addresses will be used"
                )
            } else {
                format!(
                    "the system DNS settings could not be read as a whole ({error}); using {} name server(s) from /etc/resolv.conf",
                    servers.len()
                )
            };
            DnsSettings {
                config: ResolverConfig::from_name_servers(
                    servers
                        .into_iter()
                        .map(NameServerConfig::udp_and_tcp)
                        .collect(),
                ),
                opts: ResolverOpts::default(),
                note: Some(note),
            }
        }
    }
}

/// Name servers listed in resolv.conf syntax. Entries the resolver cannot use, such as
/// addresses with a zone (`%en1`), are skipped.
fn nameservers(text: &str) -> Vec<IpAddr> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? != "nameserver" {
                return None;
            }
            words.next()?.parse().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOTSPOT: &str = "#\n# macOS Notice\n#\nnameserver fe80::1%en1\nnameserver 192.0.2.53\n";

    #[test]
    fn zone_and_garbage_entries_are_skipped() {
        let text = format!(
            "{HOTSPOT}nameserver not-an-address\noptions edns0\nnameserver 2001:db8::53 # comment\n"
        );
        let servers = nameservers(&text);
        assert_eq!(
            servers,
            vec![
                "192.0.2.53".parse::<IpAddr>().unwrap(),
                "2001:db8::53".parse().unwrap()
            ]
        );
    }

    #[test]
    fn unreadable_system_settings_fall_back_to_resolv_conf() {
        let settings = settings_from(Err("bad entry".into()), || Some(HOTSPOT.into()));
        let ips: Vec<IpAddr> = settings
            .config
            .name_servers()
            .iter()
            .map(|s| s.ip)
            .collect();
        assert_eq!(ips, vec!["192.0.2.53".parse::<IpAddr>().unwrap()]);
        assert!(settings.note.unwrap().contains("1 name server"));
    }

    #[test]
    fn without_any_server_the_node_still_gets_a_config() {
        let settings = settings_from(Err("bad entry".into()), || None);
        assert!(settings.config.name_servers().is_empty());
        assert!(settings.note.is_some());
    }

    #[test]
    fn readable_system_settings_are_used_as_they_are() {
        let system = ResolverConfig::from_name_servers(vec![NameServerConfig::udp_and_tcp(
            "192.0.2.53".parse().unwrap(),
        )]);
        let settings = settings_from(Ok((system, ResolverOpts::default())), || {
            panic!("resolv.conf must not be read when the system settings work")
        });
        assert_eq!(settings.config.name_servers().len(), 1);
        assert!(settings.note.is_none());
    }
}
