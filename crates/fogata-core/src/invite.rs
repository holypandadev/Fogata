//! Invite links. An invite carries a single-use ticket signed by a manager and a token from
//! which the device that uses it and the group's managers derive where to meet on the DHT; it
//! carries neither the group key nor the members' own rendezvous secret, so a leaked invite
//! cannot be used to find the members once it has been used or has expired. Still, only give
//! it to the person it is meant for: whoever uses it first joins.

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use fogata_crypto::{DeviceIdentity, GroupId, names};
use fogata_net::Multiaddr;
use fogata_state::{GroupLog, InviteToken, Ticket, invite_rendezvous};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "fogata:";

#[derive(Serialize, Deserialize)]
struct InviteData {
    v: u8,
    name: String,
    ticket: String,
    /// Absent in the formats before 3, which are refused by their version.
    #[serde(default)]
    token: String,
    #[serde(default)]
    addrs: Vec<String>,
}

#[derive(Debug)]
pub struct Invite {
    pub name: String,
    pub ticket: Ticket,
    pub token: InviteToken,
    pub addrs: Vec<Multiaddr>,
}

impl Invite {
    pub fn group_id(&self) -> GroupId {
        self.ticket.group_id
    }

    /// The rendezvous secret under which the invited device and the managers meet.
    pub fn rendezvous(&self) -> [u8; 32] {
        invite_rendezvous(&self.token)
    }
}

/// An invite for `log`'s group, valid for `days`, with `addrs` as hints (managers only). It
/// is noted in the log, which the caller saves and shares, so every manager announces itself
/// under it while it is open. A manager that cannot write it is told why.
pub fn make(
    log: &mut GroupLog,
    identity: &DeviceIdentity,
    days: u64,
    addrs: Vec<Multiaddr>,
    now_ms: u64,
) -> Result<String> {
    let expires = now_ms + days * 24 * 3600 * 1000;
    if !log.shared_state().is_manager(&identity.public_key()) {
        bail!("only managers can invite");
    }
    let (ticket, token) = log.invite(identity, expires, now_ms)?;
    Ok(encode(&Invite {
        name: log.state().name,
        ticket,
        token,
        addrs,
    }))
}

pub fn encode(invite: &Invite) -> String {
    let data = InviteData {
        v: 3,
        name: invite.name.clone(),
        ticket: hex::encode(invite.ticket.encode()),
        token: hex::encode(invite.token),
        addrs: invite.addrs.iter().map(ToString::to_string).collect(),
    };
    let json = serde_json::to_vec(&data).expect("invite data always serialises");
    format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
}

pub fn decode(text: &str) -> Result<Invite> {
    let text = text.trim();
    if text.starts_with(fogata_friends::LINK_PREFIX) {
        bail!("this is a friend link; add the friend with it instead");
    }
    let body = text
        .strip_prefix(PREFIX)
        .context("this is not a Fogata invite")?;
    let json = URL_SAFE_NO_PAD
        .decode(body)
        .context("the invite is damaged")?;
    let data: InviteData = serde_json::from_slice(&json).context("the invite is damaged")?;
    match data.v {
        3 => {}
        1 | 2 => bail!("this invite is from an older version; ask for a new one"),
        _ => bail!("this invite needs a newer version of Fogata"),
    }
    let ticket = hex::decode(&data.ticket)
        .ok()
        .and_then(|b| Ticket::decode(&b).ok())
        .context("the invite is damaged")?;
    let token = hex::decode(&data.token)
        .ok()
        .and_then(|b| InviteToken::try_from(b).ok())
        .context("the invite is damaged")?;
    // Shown before anything of the group can be checked: only what of it would show.
    let name: String = data
        .name
        .chars()
        .filter(|c| !c.is_control() && !fogata_state::hiding_mark(*c))
        .collect();
    let name = names::tidy(&name)
        .chars()
        .take(fogata_state::MAX_GROUP_NAME_CHARS)
        .collect();
    Ok(Invite {
        name,
        ticket,
        token,
        addrs: data.addrs.iter().filter_map(|a| a.parse().ok()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;
    use fogata_state::{Body, MAX_PICTURE_BYTES, PAST_RUN};

    use super::*;

    /// Only a manager invites; a manager that cannot write the invite into the log hears
    /// the reason, not that only managers invite.
    #[test]
    fn an_invite_that_cannot_be_made_says_why() {
        let (ana, bea) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        log.append(&ana, Body::SetManagers { first: Some(1) }, 3)
            .unwrap();
        let refused = make(&mut log, &bea, 7, Vec::new(), 4).unwrap_err();
        assert_eq!(refused.to_string(), "only managers can invite");
        // Ana fills her run with pictures: nobody else signed on any of it.
        let mut now = 5;
        while log
            .append(
                &ana,
                Body::SetPicture {
                    picture: Some(vec![now as u8; MAX_PICTURE_BYTES]),
                },
                now,
            )
            .is_ok()
        {
            now += 1;
        }
        // Then the smallest entry there is, until not even that fits.
        while log.append(&ana, Body::SetIcon { icon: 2 }, now).is_ok() {
            now += 1;
        }
        let refused = make(&mut log, &ana, 7, Vec::new(), now).unwrap_err();
        assert!(refused.to_string().contains(PAST_RUN), "{refused}");
        assert!(make(&mut log, &bea, 7, Vec::new(), now).is_err());
    }

    /// Damaged copies of `good`, as a hostile peer could send: bits flipped, bytes and
    /// lengths changed, cut short, grown, and plain noise. The same ones every run.
    fn damaged(good: &[u8], seed: u64) -> Vec<Vec<u8>> {
        let mut x = seed | 1;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as usize
        };
        (0..2000)
            .map(|_| {
                let mut bytes = good.to_vec();
                let at = next() % (bytes.len() + 1);
                match next() % 6 {
                    0 if at < bytes.len() => bytes[at] ^= 1 << (next() % 8),
                    1 if at < bytes.len() => bytes[at] = next() as u8,
                    2 => bytes.truncate(at),
                    3 => {
                        let extra: Vec<u8> = (0..next() % 64).map(|_| next() as u8).collect();
                        bytes.splice(at..at, extra);
                    }
                    4 if at + 4 <= bytes.len() => {
                        let big = [u32::MAX, 0x8000_0000, 0x0100_0000][next() % 3];
                        bytes[at..at + 4].copy_from_slice(&big.to_be_bytes());
                    }
                    _ => bytes = (0..next() % 300).map(|_| next() as u8).collect(),
                }
                bytes
            })
            .collect()
    }

    #[test]
    fn invite_round_trips() {
        let admin = DeviceIdentity::generate().unwrap();
        let ticket = Ticket::issue(&admin, GroupId::random().unwrap(), 5).unwrap();
        let addr: Multiaddr = "/ip4/203.0.113.5/udp/4001/quic-v1".parse().unwrap();
        let text = encode(&Invite {
            name: "friends".into(),
            ticket: ticket.clone(),
            token: [7u8; 16],
            addrs: vec![addr.clone()],
        });
        let invite = decode(&text).unwrap();
        assert_eq!(invite.name, "friends");
        assert_eq!(invite.ticket, ticket);
        assert_eq!(invite.token, [7u8; 16]);
        assert_eq!(invite.rendezvous(), invite_rendezvous(&[7u8; 16]));
        assert_eq!(invite.addrs, vec![addr]);
    }

    #[test]
    fn an_invite_shows_only_what_of_its_name_shows() {
        let admin = DeviceIdentity::generate().unwrap();
        let ticket = Ticket::issue(&admin, GroupId::random().unwrap(), 5).unwrap();
        let named = |name: &str| {
            decode(&encode(&Invite {
                name: name.into(),
                ticket: ticket.clone(),
                token: [7u8; 16],
                addrs: Vec::new(),
            }))
            .unwrap()
            .name
        };
        assert_eq!(named("\u{3164}\u{2800}"), "");
        assert_eq!(named("friends\u{3164}\u{3164}"), "friends");
        assert_eq!(named("a\u{202E}gnp.exe"), "agnp.exe");
        assert_eq!(named(&"g".repeat(100)), "g".repeat(60));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(decode("hello").is_err());
        assert!(decode("fogata:%%%").is_err());
        // An invite of the previous format, which carried the members' rendezvous secret.
        let old = URL_SAFE_NO_PAD.encode(br#"{"v":2,"name":"x","ticket":"00","rendezvous":"00"}"#);
        let error = decode(&format!("{PREFIX}{old}")).unwrap_err().to_string();
        assert!(error.contains("older version"));
    }

    #[test]
    fn making_an_invite_notes_it_in_the_log() {
        let admin = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&admin, "friends", 1_000).unwrap();
        let text = make(&mut log, &admin, 7, Vec::new(), 1_000).unwrap();
        let invite = decode(&text).unwrap();
        assert_eq!(invite.name, "friends");
        assert_eq!(log.state().open_invite_tokens(2_000), vec![invite.token]);
        assert_eq!(invite.ticket.expires_ms, 1_000 + 7 * 24 * 3600 * 1000);
    }

    #[test]
    fn damaged_invites_are_refused_without_a_panic() {
        let admin = DeviceIdentity::generate().unwrap();
        let ticket = Ticket::issue(&admin, GroupId::random().unwrap(), 5).unwrap();
        let text = encode(&Invite {
            name: "friends".into(),
            ticket,
            token: [7; 16],
            addrs: vec!["/ip4/203.0.113.5/udp/4001/quic-v1".parse().unwrap()],
        });
        let json = URL_SAFE_NO_PAD
            .decode(text.strip_prefix(PREFIX).unwrap())
            .unwrap();
        for bytes in damaged(&json, 1) {
            let _ = decode(&format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(&bytes)));
        }
        for bytes in damaged(text.as_bytes(), 2) {
            let _ = decode(&String::from_utf8_lossy(&bytes));
        }
    }
}
