//! What a person does in the application, through the calls the screens make (`api`), as
//! the screens make them, with no Flutter: a profile, a group with an invite and a member let
//! in, a friend made from the group, a direct chat with a file each way, a voice room with
//! its chat, call signals and files, a rename, leaving the room, a restart on the same data
//! folder, and leaving the group. What the screens get is checked as they get it: the
//! bridge's own types, the events of its stream (`UiEvent`), and the words of its errors as
//! `explain()` in the app reads them (app/test/core_errors.txt).
//!
//! The bridge runs one core per process, so the other device is a core of its own (Bea),
//! driven as fogata-core's tests drive one. Both use only the loopback interface: no DHT,
//! no mDNS, no releases page, no idle time of the desktop, and keys kept in memory rather
//! than the system's keychain. A wait that fails says what it had seen so far: the events of
//! both, and how each sees the other.

use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fogata_core::{
    Core, CoreEvent, CoreEvents, NetOptions,
    direct::{Outgoing, model::Local},
    vault::{Keychain, MemoryKeyStore},
};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_ffi::{
    api::{
        app, direct,
        direct::{UiDirectOutgoing, UiFileState},
        types::{UiAtRest, UiEvent, UiEventKind, UiPresence, UiRoomChangeKind},
    },
    testing,
};
use fogata_net::{Multiaddr, RoomChange};
use tokio::time::timeout;

/// Longest a step waits for what another device does.
const PATIENCE: Duration = Duration::from_secs(60);

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn alone(port: u16, peers: Vec<Multiaddr>) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers,
        ..Default::default()
    }
}

fn hex_of(device: DevicePublicKey) -> String {
    hex::encode(device.as_bytes())
}

/// What the screens get of a failed call: the bridge sends the error as `{:?}` puts it, and
/// `explain()` reads its first line.
fn said(error: anyhow::Error) -> String {
    format!("{error:?}")
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

/// The parts of the core's messages the app explains in its own words (see
/// tests/error_texts.rs for the file's form).
fn explained() -> Vec<String> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/test/core_errors.txt");
    std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()))
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("core "))
        .map(str::to_string)
        .collect()
}

/// Checks that `text`, as the screens get it, holds `part` and that the app explains it.
fn explained_as(text: &str, part: &str) {
    assert!(text.contains(part), "{text:?} does not say {part:?}");
    assert!(
        explained().iter().any(|known| known == part),
        "the app does not explain {part:?} (app/test/core_errors.txt)"
    );
}

/// Shown of an id in a report: its first characters.
fn short(id: Option<&str>) -> &str {
    id.map_or("-", |id| &id[..id.len().min(8)])
}

/// What the test has seen so far, told by any wait that fails: every event the screens were
/// given and Bea was told, and each core's view of the other. A wait fails at once, with
/// all of it, when the screens are told of a failure nobody waits for (a friend request
/// that did not go, a download that failed, the core stopped).
struct Scene {
    started: Instant,
    bea: Core,
    bea_id: String,
    group: Mutex<Option<GroupId>>,
    /// The screens' events taken and not looked for yet, oldest first.
    kept: Mutex<VecDeque<UiEvent>>,
    screens: Mutex<Vec<String>>,
    told_bea: Mutex<Vec<String>>,
}

/// Events of the screens that say something did not work.
fn alarming(e: &UiEvent) -> bool {
    matches!(
        e.kind,
        UiEventKind::FriendRequestFailed
            | UiEventKind::DownloadFailed
            | UiEventKind::DirectDownloadFailed
            | UiEventKind::Stopped
    )
}

impl Scene {
    fn new(bea: Core) -> Self {
        Scene {
            started: Instant::now(),
            bea_id: hex_of(bea.me()),
            bea,
            group: Mutex::new(None),
            kept: Mutex::new(VecDeque::new()),
            screens: Mutex::new(Vec::new()),
            told_bea: Mutex::new(Vec::new()),
        }
    }

    fn at(&self) -> String {
        format!("{:>6.2}s", self.started.elapsed().as_secs_f64())
    }

    /// Takes what the screens were given since; the new events that are alarming and not
    /// `expected`.
    fn take(&self, expected: &mut dyn FnMut(&UiEvent) -> bool) -> Vec<UiEvent> {
        let mut alarms = Vec::new();
        for e in testing::take_events() {
            self.screens.lock().unwrap().push(format!(
                "{} {:?} group {} device {} name {:?} text {:?} room {:?} file {}",
                self.at(),
                e.kind,
                short(e.group.as_deref()),
                short(e.device.as_deref()),
                e.name,
                e.text,
                e.room
                    .as_ref()
                    .map(|r| (r.kind, short(r.device.as_deref()).to_string())),
                short(e.file.as_deref()),
            ));
            if alarming(&e) && !expected(&e) {
                alarms.push(e.clone());
            }
            self.kept.lock().unwrap().push_back(e);
        }
        alarms
    }

    /// Everything seen, after why the test fails.
    async fn report(&self, why: String) -> String {
        let last = |lines: &[String]| -> String {
            let from = lines.len().saturating_sub(60);
            lines[from..].iter().map(|l| format!("\n    {l}")).collect()
        };
        let screens = last(&self.screens.lock().unwrap());
        let told_bea = last(&self.told_bea.lock().unwrap());
        let group = *self.group.lock().unwrap();
        format!(
            "{why} (at {})\nthe screens were given:{screens}\nBea was told:{told_bea}\n\
             Ana sees:{}\nBea sees:{}",
            self.at(),
            self.ana_view(group).await,
            self.bea_view(group).await
        )
    }

    async fn ana_view(&self, group: Option<GroupId>) -> String {
        let ask = Duration::from_secs(5);
        let mut out = String::new();
        match timeout(ask, app::friends()).await {
            Ok(Ok(friends)) => {
                for f in friends {
                    let line = format!("\n    friend {} {:?}", f.person.name, f.presence);
                    out.push_str(&line);
                }
            }
            Ok(Err(e)) => out.push_str(&format!("\n    friends: {e:#}")),
            Err(_) => out.push_str("\n    friends: no answer in 5 s"),
        }
        match timeout(ask, app::requests()).await {
            Ok(Ok(r)) => out.push_str(&format!(
                "\n    requests: {} incoming, {} outgoing",
                r.incoming.len(),
                r.outgoing.len()
            )),
            Ok(Err(e)) => out.push_str(&format!("\n    requests: {e:#}")),
            Err(_) => out.push_str("\n    requests: no answer in 5 s"),
        }
        if let Some(group) = group {
            match timeout(ask, app::members(hex::encode(group.as_bytes()))).await {
                Ok(Ok(members)) => {
                    for m in members.iter().filter(|m| !m.me) {
                        let line =
                            format!("\n    member {} connected by {:?}", m.person.name, m.path);
                        out.push_str(&line);
                    }
                }
                Ok(Err(e)) => out.push_str(&format!("\n    members: {e:#}")),
                Err(_) => out.push_str("\n    members: no answer in 5 s"),
            }
        }
        out
    }

    async fn bea_view(&self, group: Option<GroupId>) -> String {
        let ask = Duration::from_secs(5);
        let mut out = String::new();
        match timeout(ask, self.bea.friends()).await {
            Ok(Ok(friends)) => {
                for f in friends {
                    let line = format!("\n    friend {} {:?}", f.person.name, f.presence);
                    out.push_str(&line);
                }
            }
            Ok(Err(e)) => out.push_str(&format!("\n    friends: {e:#}")),
            Err(_) => out.push_str("\n    friends: no answer in 5 s"),
        }
        match timeout(ask, self.bea.requests()).await {
            Ok(Ok((incoming, outgoing))) => out.push_str(&format!(
                "\n    requests: {} incoming, {} outgoing",
                incoming.len(),
                outgoing.len()
            )),
            Ok(Err(e)) => out.push_str(&format!("\n    requests: {e:#}")),
            Err(_) => out.push_str("\n    requests: no answer in 5 s"),
        }
        if let Some(group) = group {
            match timeout(ask, self.bea.members(group)).await {
                Ok(Ok(members)) => {
                    for m in members.iter().filter(|m| !m.me) {
                        let line =
                            format!("\n    member {} connected by {:?}", m.person.name, m.online);
                        out.push_str(&line);
                    }
                }
                Ok(Err(e)) => out.push_str(&format!("\n    members: {e:#}")),
                Err(_) => out.push_str("\n    members: no answer in 5 s"),
            }
        }
        out
    }

    /// Waits for an event the screens would be given, leaving out those before it.
    async fn screens_get(&self, what: &str, mut wanted: impl FnMut(&UiEvent) -> bool) -> UiEvent {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let alarms = self.take(&mut wanted);
            {
                let mut kept = self.kept.lock().unwrap();
                if let Some(at) = kept.iter().position(&mut wanted) {
                    let found = kept.remove(at).unwrap();
                    kept.drain(..at);
                    return found;
                }
            }
            if let Some(alarm) = alarms.first() {
                panic!(
                    "{}",
                    self.report(format!(
                        "waiting for {what}, the screens were told {:?}: {:?}",
                        alarm.kind, alarm.text
                    ))
                    .await
                );
            }
            if Instant::now() > deadline {
                panic!(
                    "{}",
                    self.report(format!(
                        "the screens were not told of {what} in {PATIENCE:?}"
                    ))
                    .await
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Fails at once when the screens were told of a failure meanwhile.
    async fn no_alarm(&self, what: &str) {
        if let Some(alarm) = self.take(&mut |_| false).first() {
            panic!(
                "{}",
                self.report(format!(
                    "waiting for {what}, the screens were told {:?}: {:?}",
                    alarm.kind, alarm.text
                ))
                .await
            );
        }
    }

    async fn bea_gets(
        &self,
        events: &mut CoreEvents,
        what: &str,
        mut done: impl FnMut(&CoreEvent) -> bool,
    ) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            match timeout(Duration::from_millis(50), events.recv()).await {
                Ok(Some(event)) => {
                    let mut said = format!("{event:?}");
                    if said.len() > 240 {
                        let cut = (0..=240).rev().find(|&i| said.is_char_boundary(i)).unwrap();
                        said.truncate(cut);
                        said.push('…');
                    }
                    self.told_bea
                        .lock()
                        .unwrap()
                        .push(format!("{} {said}", self.at()));
                    if done(&event) {
                        return;
                    }
                }
                Ok(None) => panic!(
                    "{}",
                    self.report(format!("Bea's core stopped before {what}"))
                        .await
                ),
                Err(_) => {}
            }
            self.no_alarm(what).await;
            if Instant::now() > deadline {
                panic!(
                    "{}",
                    self.report(format!("Bea was not told of {what} in {PATIENCE:?}"))
                        .await
                );
            }
        }
    }

    /// Asks again until `check` gives something: what a device shows of another also
    /// changes with heartbeats and lists read again, which no event marks.
    async fn until<T, F, Fut>(&self, what: &str, mut check: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Option<T>>,
    {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(found) = check().await {
                return found;
            }
            self.no_alarm(what).await;
            if Instant::now() > deadline {
                panic!(
                    "{}",
                    self.report(format!("timed out waiting for {what} in {PATIENCE:?}"))
                        .await
                );
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Asks again until `check` holds (see [`Scene::until`]).
    async fn eventually<F, Fut>(&self, what: &str, mut check: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        self.until(what, || {
            let held = check();
            async move { held.await.then_some(()) }
        })
        .await
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_person_does_the_main_things_through_the_bridge() {
    let base = std::env::temp_dir().join(format!("fogata-ffi-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b, files) = (base.join("ana"), base.join("bea"), base.join("files"));
    std::fs::create_dir_all(&files).unwrap();
    testing::use_keychain(Keychain::Seal(Arc::new(MemoryKeyStore::default())));

    // Before the core runs, as the welcome screen calls.
    assert!(!app::has_profile(text(&dir_a)).unwrap());
    explained_as(
        &said(app::my_profile().await.unwrap_err()),
        "Fogata is not running yet",
    );
    let made = app::create_profile(text(&dir_a), "Ana".into(), 3, None).unwrap();
    assert_eq!(
        (made.person.name.as_str(), made.person.avatar.preset),
        ("Ana", 3)
    );
    assert!(app::has_profile(text(&dir_a)).unwrap());

    // Bea, on her own core, and Ana each listen on a loopback port of their own and dial
    // the other's: dials the test names, which a device makes whatever its networks (an
    // address a peer names is never dialled on the loopback interface), so the two find
    // each other the same way on every computer, again after Ana starts anew.
    let (port_a, port_b) = loop {
        let ports = (free_port(), free_port());
        if ports.0 != ports.1 {
            break ports;
        }
    };
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let (bea, mut events_b) =
        Core::start(dir_b.clone(), alone(port_b, vec![loopback(port_a)])).unwrap();
    let scene = Scene::new(bea.clone());
    let bea_id = scene.bea_id.clone();
    testing::use_network(alone(port_a, vec![loopback(port_b)]));
    let me = app::start(text(&dir_a)).await.unwrap();
    assert_eq!(me.person.id, made.person.id);
    assert_eq!(app::at_rest().await.unwrap(), UiAtRest::Sealed);
    let ana = DevicePublicKey::from_bytes(&hex::decode(&me.person.id).unwrap()).unwrap();
    // The folder is held while the core runs.
    let held = Core::start(dir_a.clone(), alone(0, Vec::new())).unwrap_err();
    explained_as(&said(held), "Fogata is already running with the data");

    // What the profile takes is checked, in words the app explains.
    explained_as(
        &said(app::set_profile(String::new(), 0, None).await.unwrap_err()),
        "a name needs",
    );
    explained_as(
        &said(app::set_bio("x".repeat(400)).await.unwrap_err()),
        "a bio has at most",
    );
    for (typed, part) in [
        ("hello", "a friend code looks like"),
        ("fogata:abc", "this is a group invite"),
        ("fogata:friend:!!", "friend link is damaged"),
        ("fogata:friend:Yw", "needs a newer version"),
    ] {
        explained_as(
            &said(app::add_friend(typed.into()).await.unwrap_err()),
            part,
        );
    }
    let own = app::friend_link().await.unwrap();
    app::add_friend(own.text).await.unwrap();
    let refused = scene
        .screens_get("the own link refused", |e| {
            e.kind == UiEventKind::FriendRequestFailed
        })
        .await;
    explained_as(
        refused.text.as_deref().unwrap(),
        "this link was made on this device",
    );

    // A group, an invite, and Bea let in.
    let group = app::create_group("crew".into(), 4, Vec::new())
        .await
        .unwrap();
    let groups = app::groups().await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        (groups[0].id.as_str(), groups[0].name.as_str()),
        (group.as_str(), "crew")
    );
    assert!(groups[0].manager);
    let voice = groups[0].voice_rooms[0].id.clone();
    let gid = GroupId::from_bytes(&hex::decode(&group).unwrap()).unwrap();
    *scene.group.lock().unwrap() = Some(gid);
    let invite = app::invite(group.clone(), 1).await.unwrap();
    explained_as(
        &said(app::join_group(invite.clone()).await.unwrap_err()),
        "this device is already in that group",
    );
    bea.join_group(&invite, None, None).await.unwrap();
    let asked = scene
        .screens_get("Bea's ask to join", |e| {
            e.kind == UiEventKind::JoinAsked && e.device.as_deref() == Some(bea_id.as_str())
        })
        .await;
    assert_eq!(
        (asked.group.as_deref(), asked.name.as_deref()),
        (Some(group.as_str()), Some("Bea"))
    );
    let asks = app::join_asks().await.unwrap();
    assert_eq!(asks.len(), 1);
    assert_eq!(
        (asks[0].device_id.as_str(), asks[0].group_name.as_str()),
        (bea_id.as_str(), "crew")
    );
    app::answer_join(group.clone(), bea_id.clone(), true)
        .await
        .unwrap();
    scene
        .bea_gets(&mut events_b, "the join", |e| {
            matches!(e, CoreEvent::Joined { .. })
        })
        .await;
    scene
        .eventually("Bea among the members", || async {
            let members = app::members(group.clone()).await.unwrap();
            members.len() == 2 && members.iter().any(|m| m.person.id == bea_id && !m.me)
        })
        .await;

    // Friends from the group: no link or code. The request goes over a connection where
    // each proved to the other to be a member, so only once each shows the other connected,
    // as the screens offer it only then: before, it fails at once ("not connected") or comes
    // back refused ("did not reach the member").
    scene
        .eventually(
            "Ana and Bea connected as members, each in the other's view",
            || async {
                let ana_sees = app::members(group.clone())
                    .await
                    .unwrap()
                    .iter()
                    .any(|m| m.person.id == bea_id && m.path.is_some());
                let bea_sees = bea
                    .members(gid)
                    .await
                    .unwrap()
                    .iter()
                    .any(|m| m.person.device == ana && m.online.is_some());
                ana_sees && bea_sees
            },
        )
        .await;
    app::ask_member(bea_id.clone(), "Bea".into()).await.unwrap();
    scene
        .bea_gets(
            &mut events_b,
            "Ana's friend request",
            |e| matches!(e, CoreEvent::FriendRequest { profile } if profile.device == ana),
        )
        .await;
    bea.answer_request(ana, true).await.unwrap();
    let added = scene
        .screens_get("Bea a friend", |e| {
            e.kind == UiEventKind::FriendAdded && e.device.as_deref() == Some(bea_id.as_str())
        })
        .await;
    assert_eq!(added.name.as_deref(), Some("Bea"));
    scene
        .eventually("Bea online as a friend", || async {
            app::friends()
                .await
                .unwrap()
                .iter()
                .any(|f| f.person.id == bea_id && !matches!(f.presence, UiPresence::Offline))
        })
        .await;
    scene
        .eventually("Ana online at Bea's", || async {
            bea.friends().await.unwrap().iter().any(|f| {
                f.person.device == ana
                    && !matches!(f.presence, fogata_core::Presence::Offline { .. })
            })
        })
        .await;
    // A friend's link, used again, says so.
    app::add_friend(bea.friend_link().await.unwrap().text)
        .await
        .unwrap();
    let again = scene
        .screens_get("a friend's link refused", |e| {
            e.kind == UiEventKind::FriendRequestFailed
        })
        .await;
    explained_as(again.text.as_deref().unwrap(), "you are already friends");

    // A direct chat, with a small file each way.
    let notes = files.join("notes.txt");
    std::fs::write(&notes, b"bring the tent\n").unwrap();
    direct::send_direct(
        bea_id.clone(),
        "the notes".into(),
        vec![UiDirectOutgoing {
            path: text(&notes),
            video: None,
        }],
    )
    .await
    .unwrap();
    scene.bea_gets(&mut events_b, "Ana's message", |e| {
        matches!(e, CoreEvent::DirectMessage { friend, text } if *friend == ana && text == "the notes")
    })
    .await;
    let chat = bea.direct_chat(ana, 10).await.unwrap();
    let sent = chat.items.last().unwrap();
    assert_eq!(sent.files.len(), 1);
    let notes_id = sent.files[0].file.id;
    bea.fetch_direct_file(ana, notes_id).await.unwrap();
    scene
        .eventually("the notes at Bea's", || async {
            let chat = bea.direct_chat(ana, 10).await.unwrap();
            chat.items.last().unwrap().files[0].file.local == Local::Here
        })
        .await;
    // Kept sealed, as everything of the chat's: read through the core.
    std::fs::create_dir_all(files.join("at-bea")).unwrap();
    let copy = bea
        .save_direct_file(ana, notes_id, &files.join("at-bea"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(copy).unwrap(), b"bring the tent\n");

    let map = files.join("map.bin");
    let map_bytes: Vec<u8> = (0..30_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(&map, &map_bytes).unwrap();
    bea.send_direct(
        ana,
        "got it\nhere is the map",
        vec![Outgoing {
            path: map.clone(),
            video: None,
        }],
    )
    .await
    .unwrap();
    let message = scene
        .screens_get("Bea's answer", |e| {
            e.kind == UiEventKind::DirectMessage && e.device.as_deref() == Some(bea_id.as_str())
        })
        .await;
    assert_eq!(message.text.as_deref(), Some("got it"));
    let page = direct::direct_chat(bea_id.clone(), 10).await.unwrap();
    assert_eq!(page.friend.id, bea_id);
    let texts: Vec<&str> = page.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["the notes", "got it\nhere is the map"]);
    assert!(page.items[0].mine && !page.items[1].mine);
    let map_file = page.items[1].files[0].clone();
    assert_eq!(
        (map_file.name.as_str(), map_file.size, map_file.state),
        ("map.bin", 30_000, UiFileState::Missing)
    );
    direct::fetch_direct_file(bea_id.clone(), map_file.id.clone())
        .await
        .unwrap();
    scene
        .eventually("the map here", || async {
            let page = direct::direct_chat(bea_id.clone(), 10).await.unwrap();
            page.items[1].files[0].state == UiFileState::Here
        })
        .await;
    let saved_dir = base.join("saved");
    std::fs::create_dir_all(&saved_dir).unwrap();
    let saved = direct::save_direct_file(bea_id.clone(), map_file.id.clone(), text(&saved_dir))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&saved).unwrap(), map_bytes);

    // The voice room: its people, its chat, call signals and files. No media.
    explained_as(
        &said(
            app::say(group.clone(), "too early".into())
                .await
                .unwrap_err(),
        ),
        "join the room first",
    );
    app::join_room(group.clone(), voice.clone()).await.unwrap();
    bea.join_room(gid).await.unwrap();
    let joined = scene
        .screens_get("Bea in the room", |e| {
            e.kind == UiEventKind::Room
                && e.room.as_ref().is_some_and(|r| {
                    r.kind == UiRoomChangeKind::Joined
                        && r.device.as_deref() == Some(bea_id.as_str())
                })
        })
        .await;
    assert_eq!(
        (joined.group.as_deref(), joined.voice.as_deref()),
        (Some(group.as_str()), Some(voice.as_str()))
    );
    app::say(group.clone(), "hello room".into()).await.unwrap();
    scene.bea_gets(&mut events_b, "Ana's line", |e| {
        matches!(e, CoreEvent::Room { change: RoomChange::Chat(line), .. } if line.text == "hello room" && line.sender == ana)
    })
    .await;
    app::send_signal(group.clone(), bea_id.clone(), "offer 1".into())
        .await
        .unwrap();
    scene.bea_gets(&mut events_b, "Ana's signal", |e| {
        matches!(e, CoreEvent::Signal { from, payload, .. } if *from == ana && payload == b"offer 1")
    })
    .await;
    bea.signal(gid, ana, b"answer 1".to_vec()).await.unwrap();
    let signal = scene
        .screens_get("Bea's signal", |e| {
            e.kind == UiEventKind::Signal && e.device.as_deref() == Some(bea_id.as_str())
        })
        .await;
    assert_eq!(
        (signal.group.as_deref(), signal.text.as_deref()),
        (Some(group.as_str()), Some("answer 1"))
    );

    let photo = files.join("lake.jpg");
    let photo_bytes: Vec<u8> = (0..40_000u32).map(|i| (i % 249) as u8).collect();
    std::fs::write(&photo, &photo_bytes).unwrap();
    let shared = bea.share_file(gid, photo, None).await.unwrap();
    let shared_id = hex::encode(shared.id);
    let listed = scene
        .screens_get("Bea's file in the room", |e| {
            e.kind == UiEventKind::Room
                && e.room.as_ref().is_some_and(|r| {
                    r.kind == UiRoomChangeKind::FileShared
                        && r.file_id.as_deref() == Some(shared_id.as_str())
                })
        })
        .await;
    let change = listed.room.unwrap();
    assert_eq!(
        (change.file_name.as_deref(), change.file_size),
        (Some("lake.jpg"), 40_000)
    );
    let downloads = base.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let dest = app::download(group.clone(), shared_id.clone(), text(&downloads))
        .await
        .unwrap();
    let done = scene
        .screens_get("the download done", |e| {
            e.kind == UiEventKind::DownloadDone && e.file.as_deref() == Some(shared_id.as_str())
        })
        .await;
    assert_eq!(done.text.as_deref(), Some(dest.as_str()));
    assert_eq!(std::fs::read(&dest).unwrap(), photo_bytes);

    let plan = files.join("plan.txt");
    std::fs::write(&plan, b"north ridge at dawn").unwrap();
    let plan_id = app::share_file(group.clone(), text(&plan), None)
        .await
        .unwrap();
    scene
        .eventually("Ana's file at Bea's", || async {
            bea.room(gid, fogata_core::FIRST_ROOM)
                .await
                .unwrap()
                .is_some_and(|r| r.files.iter().any(|f| hex::encode(f.info.id) == plan_id))
        })
        .await;
    let room = app::room(group.clone(), voice.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(room.joined);
    assert_eq!(room.participants.len(), 2);
    assert!(
        room.participants
            .iter()
            .any(|p| p.me && p.person.name == "Ana")
    );
    assert!(room.chat.iter().any(|l| l.text == "hello room" && l.mine));
    let names: Vec<&str> = room.files.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"lake.jpg") && names.contains(&"plan.txt"),
        "{names:?}"
    );

    // A new name for the group and for Ana, seen at Bea's.
    app::rename_group(group.clone(), "crew two".into())
        .await
        .unwrap();
    app::set_profile("Ana B".into(), 3, None).await.unwrap();
    scene
        .eventually("the new names at Bea's", || async {
            let renamed = bea
                .groups()
                .await
                .unwrap()
                .iter()
                .any(|g| g.id == gid && g.name == "crew two");
            let named = bea
                .friends()
                .await
                .unwrap()
                .iter()
                .any(|f| f.person.device == ana && f.person.name == "Ana B");
            renamed && named
        })
        .await;

    app::leave_room(group.clone()).await.unwrap();
    scene.bea_gets(&mut events_b, "Ana gone from the room", |e| {
        matches!(e, CoreEvent::Room { change: RoomChange::Left { device, .. }, .. } if *device == ana)
    })
    .await;

    // Stopped and started again on the same folder: everything is there, and the screens
    // hear of the new run.
    app::stop_core(text(&dir_a)).await.unwrap();
    explained_as(
        &said(app::groups().await.unwrap_err()),
        "Fogata is not running yet",
    );
    let again = app::start(text(&dir_a)).await.unwrap();
    assert_eq!(
        (again.person.id.as_str(), again.person.name.as_str()),
        (me.person.id.as_str(), "Ana B")
    );
    assert_eq!(app::at_rest().await.unwrap(), UiAtRest::Sealed);
    let groups = app::groups().await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        (groups[0].id.as_str(), groups[0].name.as_str()),
        (group.as_str(), "crew two")
    );
    assert_eq!(groups[0].members, 2);
    assert!(
        app::friends()
            .await
            .unwrap()
            .iter()
            .any(|f| f.person.id == bea_id)
    );
    let page = direct::direct_chat(bea_id.clone(), 10).await.unwrap();
    let texts: Vec<&str> = page.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["the notes", "got it\nhere is the map"]);
    assert_eq!(page.items[1].files[0].state, UiFileState::Here);
    assert!(Path::new(&dest).is_file());
    scene
        .screens_get("Bea back after the restart", |e| {
            matches!(
                e.kind,
                UiEventKind::FriendOnline | UiEventKind::MemberConnected
            ) && e.device.as_deref() == Some(bea_id.as_str())
        })
        .await;

    // Leaving the group.
    app::leave_group(group.clone()).await.unwrap();
    assert!(app::groups().await.unwrap().is_empty());
    scene
        .eventually("Ana gone from the group at Bea's", || async {
            bea.members(gid).await.unwrap().len() == 1
        })
        .await;

    app::stop_core(text(&dir_a)).await.unwrap();
    bea.shutdown().await.unwrap();
    drop(bea);
    let _ = std::fs::remove_dir_all(&base);
}
