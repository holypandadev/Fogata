//! Being in a voice room: joining and leaving it, what is said and typed there, what the
//! screens show of it, and a call's signals and media bridge.

use std::{collections::HashMap, sync::atomic::Ordering};

use anyhow::{Result, bail};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_state::{Barrier, FIRST_ROOM, KeptOut, VoiceRoomId};

use crate::{Core, FileView, MediaBridge, RoomState, now_ms, shares};

/// A time to wait, in words: "58 min", "3 h", "2 days".
fn time_left(ms: u64) -> String {
    let minutes = ms.div_ceil(60_000).max(1);
    match minutes {
        ..60 => format!("{minutes} min"),
        60..1440 => format!("{} h", minutes.div_ceil(60)),
        _ => match minutes.div_ceil(1440) {
            1 => "1 day".into(),
            days => format!("{days} days"),
        },
    }
}

/// Where the media bridge listens: `127.0.0.1`, except on Windows, where a socket bound to a
/// network address (as WebRTC binds its sockets) cannot send to `127.0.0.1`. There it is the
/// address of the interface toward the internet, found by asking the system for a route to a
/// documentation address; nothing is sent.
fn bridge_host() -> std::net::IpAddr {
    use std::net::Ipv4Addr;
    if !cfg!(windows) {
        return Ipv4Addr::LOCALHOST.into();
    }
    fogata_media::bridge::default_route_ipv4()
        .unwrap_or(Ipv4Addr::LOCALHOST)
        .into()
}

impl Core {
    /// Enters a group's first voice room, starting a room there if nobody is in it. Only one
    /// room at a time: another room this device is in is left first.
    pub async fn join_room(&self, group: GroupId) -> Result<()> {
        self.join_voice_room(group, FIRST_ROOM, None).await
    }

    /// The same, with a nickname other than the profile name.
    pub async fn join_room_as(&self, group: GroupId, nick: Option<String>) -> Result<()> {
        self.join_voice_room(group, FIRST_ROOM, nick).await
    }

    /// Enters one of a group's voice rooms, unless it is full.
    pub async fn join_voice_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        nick: Option<String>,
    ) -> Result<()> {
        let me = self.me();
        let state = self.log(group).await?.state();
        let name = |voice: &VoiceRoomId| {
            state
                .rooms
                .iter()
                .find(|r| r.id == *voice)
                .map(|r| r.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    // The first room shows as "General" until someone names it.
                    if *voice == FIRST_ROOM {
                        "General".into()
                    } else {
                        "that voice room".into()
                    }
                })
        };
        match state.barrier(&voice, &me, now_ms()) {
            Some(Barrier::TimedOut(KeptOut {
                until_ms: Some(until),
                ..
            })) => bail!(
                "you can come back to {} in {}",
                name(&voice),
                time_left(until.saturating_sub(now_ms()))
            ),
            Some(_) => bail!("{} is not for you now", name(&voice)),
            None => {}
        }
        let max = state.max_in_room;
        // Someone not heard from for a while (away) keeps no place: a device that crashed
        // would otherwise hold it until it is dropped, two minutes later. Coming back, it is
        // still in the room.
        if let Some(room) = self.inner.node.room(group, voice).await?
            && !room.joined
            && room
                .participants
                .iter()
                .filter(|(d, _, away)| *d != me && !away)
                .count()
                >= usize::from(max)
        {
            bail!("that voice room is full: {max} people at most");
        }
        for other in self.inner.node.groups().await? {
            let other = other.group_id;
            if other != group
                && self
                    .inner
                    .node
                    .rooms(other)
                    .await?
                    .iter()
                    .any(|(_, r)| r.joined)
            {
                self.leave_room(other).await?;
            }
        }
        let nick = match nick {
            Some(nick) => nick,
            None => self
                .profile()
                .await?
                .map(|p| p.name)
                .unwrap_or_else(|| "anon".into()),
        };
        // Another voice room of the group this device is in is left: what it shared there
        // stops being served.
        let other_voice = self
            .inner
            .node
            .rooms(group)
            .await?
            .iter()
            .any(|(v, r)| r.joined && *v != voice);
        let joining = self.inner.node.join_room(group, voice, nick);
        let joined = match other_voice {
            true => shares::leaving(&self.inner, group, joining).await,
            false => joining.await,
        };
        Ok(joined?)
    }

    /// Leaves a room on purpose; the files this device shared there stop being served.
    pub async fn leave_room(&self, group: GroupId) -> Result<()> {
        let left = shares::leaving(&self.inner, group, self.inner.node.leave_room(group)).await;
        self.inner.lock().left_room.notify_waiters();
        Ok(left?)
    }

    pub async fn say(&self, group: GroupId, text: &str) -> Result<()> {
        anyhow::ensure!(
            text.chars().count() <= fogata_state::MAX_CHAT_CHARS,
            "a message holds at most {} characters",
            fogata_state::MAX_CHAT_CHARS
        );
        Ok(self.inner.node.room_chat(group, text.to_string()).await?)
    }

    /// Tells the others in the voice room of `group` this device is in, those connected
    /// now, that it is typing in the room's chat, or stopped. Nothing is kept.
    pub async fn typing(&self, group: GroupId, typing: bool) -> Result<()> {
        Ok(self.inner.node.typing(group, typing).await?)
    }

    /// A voice room of a group, if a room is open in it.
    pub async fn room(&self, group: GroupId, voice: VoiceRoomId) -> Result<Option<RoomState>> {
        let me = self.me();
        let Some(room) = self.inner.node.room(group, voice).await? else {
            return Ok(None);
        };
        let members = self.log(group).await?.shared_state();
        let book = self.inner.node.friends().await?;
        let state = self.inner.lock();
        let downloads = state.downloads.clone();
        let sent = state.sent.clone();
        drop(state);
        let nicks: HashMap<DevicePublicKey, String> = room
            .participants
            .iter()
            .map(|(d, n, _)| (*d, n.clone()))
            .collect();
        let who = |device: &DevicePublicKey| {
            self.person_with(
                *device,
                nicks.get(device).map(String::as_str),
                book.as_ref(),
                &members,
            )
        };
        let files = room
            .files
            .iter()
            .map(|file| FileView {
                info: file.info.clone(),
                shared_by: who(&file.shared_by),
                shared_ms: file.shared_ms,
                holders: file.holders.iter().map(who).collect(),
                mine: file.holders.contains(&me),
                downloading: downloads.get(&(group, file.info.id)).copied(),
                sent: sent.get(&(group, file.info.id)).copied().unwrap_or(0),
                path: self.inner.shared.get(group, &file.info.id),
            })
            .collect();
        Ok(Some(RoomState {
            started_ms: room.started_ms,
            joined: room.joined,
            participants: room
                .participants
                .iter()
                .map(|(device, _, away)| (who(device), *away))
                .collect(),
            chat: room.chat,
            files,
        }))
    }

    /// Sends a call signal to a connected member of `group`.
    pub async fn signal(
        &self,
        group: GroupId,
        to: DevicePublicKey,
        payload: Vec<u8>,
    ) -> Result<()> {
        Ok(self.inner.node.signal(group, to, payload).await?)
    }

    /// This device's media bridge, started on first use.
    pub async fn media_bridge(&self) -> Result<MediaBridge> {
        let mut bridge = self.inner.bridge.lock().await;
        if let Some((_, info)) = bridge.as_ref() {
            return Ok(info.clone());
        }
        // None starts once the core shuts down: nothing would stop it.
        anyhow::ensure!(
            !self.inner.stopping.load(Ordering::Relaxed),
            "the core is shutting down"
        );
        let secret = |len: usize| -> Result<String> {
            let mut bytes = vec![0u8; len];
            fogata_crypto::random_bytes(&mut bytes)?;
            Ok(hex::encode(bytes))
        };
        let (username, password) = (secret(8)?, secret(16)?);
        let running = fogata_media::bridge::start(
            self.inner.node.clone(),
            fogata_media::bridge::BridgeConfig {
                listen: (bridge_host(), 0).into(),
                username: username.clone(),
                password: password.clone(),
                impairment: None,
                // Test only (debug builds): media over streams even where datagrams would work.
                streams_only: cfg!(debug_assertions)
                    && std::env::var_os("FOGATA_MEDIA_STREAMS").is_some(),
                // Test only (debug builds): no answers to connectivity checks while this
                // file exists.
                unanswered: std::env::var_os("FOGATA_TEST_UNANSWERED")
                    .filter(|_| cfg!(debug_assertions))
                    .map(Into::into),
            },
        )
        .await?;
        let info = MediaBridge {
            host: running.local_addr().ip(),
            port: running.local_addr().port(),
            username,
            password,
        };
        *bridge = Some((running, info.clone()));
        Ok(info)
    }
}
