//! A last line said right before its sender leaves the room, or quits: whether the others
//! in the room get it, and what brings it to one that missed it. Three members (Ana, Bea,
//! Cai), with Ana reached through UDP forwarders that can slow or cut her links to each of
//! the other two; Bea and Cai are connected directly. Nodes in one process, over the
//! loopback interface.
//!
//! What these tests show (rounds and rates measured on one computer, see each test):
//! - A line goes to each member as its own request, the leave as another. One that fails
//!   on its way, or is dropped from a full queue, goes again once while the member is still
//!   connected; quitting waits up to 2 s for those still on their way.
//! - A member that missed a line gets it when a participant's presence (every 20 s) names
//!   it within two minutes, from the line's author while connected, else from the
//!   participant that named it: Cai's presence brings Bea a line Ana's link failed to.
//! - Nothing brings back a line no member got once its author's node stopped: nobody has
//!   it, and its next session does not either.
//! - Ana's own room shows the line said whether or not anyone got it: nothing tells her.

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, MembershipProof, SealedMessage};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvents, NodeHandle};
use fogata_proto::{
    GroupPayload, MEMBER_PROTOCOL, MemberRequest, MemberResponse, WireMessage, WireProof,
};
use fogata_state::{FIRST_ROOM, GroupLog};
use futures::StreamExt;
use libp2p::{
    StreamProtocol, Swarm,
    request_response::{self, ProtocolSupport, ResponseChannel},
    swarm::{
        ConnectionId, SwarmEvent,
        dial_opts::{DialOpts, PeerCondition},
    },
};
use tokio::{net::UdpSocket, time::timeout};

/// One test at a time: each starts devices that talk over this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// How long a member is given to get a line, repairs included.
const WITHIN: Duration = Duration::from_secs(30);

/// A way between Ana and one member: each packet waits `delay_ms` on its way, either way,
/// and every packet is dropped while `cut` is set.
#[derive(Default)]
struct Link {
    cut: AtomicBool,
    delay_ms: AtomicU64,
}

impl Link {
    fn cut(&self, cut: bool) {
        self.cut.store(cut, Ordering::SeqCst);
    }
}

/// Sends `buf` to `to` from `socket` after the link's delay, unless the link is cut.
fn pass(link: &Arc<Link>, socket: &Arc<UdpSocket>, buf: &[u8], to: SocketAddr) {
    if link.cut.load(Ordering::SeqCst) {
        return;
    }
    let delay = Duration::from_millis(link.delay_ms.load(Ordering::SeqCst));
    let (socket, buf) = (socket.clone(), buf.to_vec());
    tokio::spawn(async move {
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let _ = socket.send_to(&buf, to).await;
    });
}

/// A UDP forwarder between the latest device to send to the address it returns and the
/// device at `back`, through `link`.
async fn forwarder(back: SocketAddr, link: Arc<Link>) -> SocketAddr {
    let front = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let behind = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let address = front.local_addr().unwrap();
    let client: Arc<std::sync::Mutex<Option<SocketAddr>>> = Default::default();
    {
        let (front, behind, link, client) =
            (front.clone(), behind.clone(), link.clone(), client.clone());
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            while let Ok((n, from)) = front.recv_from(&mut buf).await {
                *client.lock().unwrap() = Some(from);
                pass(&link, &behind, &buf[..n], back);
            }
        });
    }
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        while let Ok((n, _)) = behind.recv_from(&mut buf).await {
            let to = *client.lock().unwrap();
            if let Some(to) = to {
                pass(&link, &front, &buf[..n], to);
            }
        }
    });
    address
}

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

fn quic(address: SocketAddr) -> Multiaddr {
    loopback(address.port())
}

/// A node of `log`'s group whose events are read and dropped as they come: the application
/// is never behind (see events.rs), so nothing is refused for that.
fn node(
    identity: Arc<DeviceIdentity>,
    log: GroupLog,
    port: u16,
    peers: Vec<Multiaddr>,
) -> NodeHandle {
    let (handle, events) = fogata_net::start(NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers,
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap();
    drain(events);
    handle
}

fn drain(mut events: NodeEvents) {
    tokio::spawn(async move { while events.recv().await.is_some() {} });
}

/// Polls `check` until it holds, for up to `seconds`.
async fn eventually<F, Fut>(what: &str, seconds: u64, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let held = timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
}

/// Who `node` sees in the first voice room of `group`.
async fn in_room(node: &NodeHandle, group: GroupId) -> Vec<DevicePublicKey> {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .map(|r| r.participants.iter().map(|(d, _, _)| *d).collect())
        .unwrap_or_default()
}

/// Whether `node`'s first voice room of `group` has the line `text`.
async fn has_line(node: &NodeHandle, group: GroupId, text: &str) -> bool {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .is_some_and(|r| r.chat.iter().any(|l| l.text == text))
}

/// Whether `node` is connected to each of `devices` in `group`.
async fn connected(node: &NodeHandle, group: GroupId, devices: &[DevicePublicKey]) -> bool {
    let members = node.members(group).await.unwrap();
    devices.iter().all(|d| members.iter().any(|(m, _)| m == d))
}

/// The three members, Bea and Cai started and connected to each other.
struct Three {
    ana: Arc<DeviceIdentity>,
    log: GroupLog,
    group: GroupId,
    bea: NodeHandle,
    cai: NodeHandle,
    b: DevicePublicKey,
    c: DevicePublicKey,
    /// Ana's ways to Bea and to Cai, and where she dials them.
    to_bea: Arc<Link>,
    to_cai: Arc<Link>,
    peers: Vec<Multiaddr>,
}

impl Three {
    async fn new() -> Self {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let cai = Arc::new(DeviceIdentity::generate().unwrap());
        let (b, c) = (bea.public_key(), cai.public_key());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, b, "bea", 2).unwrap();
        log.add(&ana, c, "cai", 3).unwrap();
        let group = log.group_id();
        let (port_b, port_c) = (free_port(), free_port());
        let cai_node = node(cai, log.clone(), port_c, Vec::new());
        let bea_node = node(bea, log.clone(), port_b, vec![loopback(port_c)]);
        let to_bea = Arc::new(Link::default());
        let to_cai = Arc::new(Link::default());
        let via_b = forwarder(([127, 0, 0, 1], port_b).into(), to_bea.clone()).await;
        let via_c = forwarder(([127, 0, 0, 1], port_c).into(), to_cai.clone()).await;
        eventually("Bea and Cai connected", 60, || async {
            connected(&bea_node, group, &[c]).await
        })
        .await;
        for (n, handle) in [&bea_node, &cai_node].into_iter().enumerate() {
            handle
                .join_room(group, FIRST_ROOM, ["bea", "cai"][n].into())
                .await
                .unwrap();
        }
        Self {
            ana,
            log,
            group,
            bea: bea_node,
            cai: cai_node,
            b,
            c,
            to_bea,
            to_cai,
            peers: vec![quic(via_b), quic(via_c)],
        }
    }

    fn a(&self) -> DevicePublicKey {
        self.ana.public_key()
    }

    /// Starts a session of Ana's, dialling Bea and Cai through her links, and waits until
    /// she is connected to both.
    async fn start_ana(&self) -> NodeHandle {
        self.start_ana_with(&[self.b, self.c]).await
    }

    /// [`Self::start_ana`], waiting until she is connected to `devices`.
    async fn start_ana_with(&self, devices: &[DevicePublicKey]) -> NodeHandle {
        let ana = node(self.ana.clone(), self.log.clone(), 0, self.peers.clone());
        eventually("Ana connected", 60, || async {
            connected(&ana, self.group, devices).await
        })
        .await;
        ana
    }

    /// Ana enters the room, and waits until the three see each other there.
    async fn ana_enters(&self, ana: &NodeHandle) {
        let group = self.group;
        ana.join_room(group, FIRST_ROOM, "ana".into())
            .await
            .unwrap();
        let all = [self.a(), self.b, self.c];
        eventually("the three in the room everywhere", 60, || async {
            for handle in [ana, &self.bea, &self.cai] {
                let seen = in_room(handle, group).await;
                if !all.iter().all(|d| seen.contains(d)) {
                    return false;
                }
            }
            true
        })
        .await;
    }

    /// Waits until Bea and Cai no longer list Ana in the room, for up to `WITHIN`.
    async fn ana_gone(&self) -> bool {
        let a = self.a();
        timeout(WITHIN, async {
            while in_room(&self.bea, self.group).await.contains(&a)
                || in_room(&self.cai, self.group).await.contains(&a)
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .is_ok()
    }

    /// When Bea and Cai got `text`, from `since`, waiting up to `WITHIN`; none for a member
    /// that did not. With `give_up_early`, the wait ends 5 s in when neither has it: then
    /// nobody but Ana ever had it, and her node is gone.
    async fn arrivals(
        &self,
        text: &str,
        since: Instant,
        give_up_early: bool,
    ) -> [Option<Duration>; 2] {
        let mut got = [None, None];
        while since.elapsed() < WITHIN {
            for (n, handle) in [&self.bea, &self.cai].into_iter().enumerate() {
                if got[n].is_none() && has_line(handle, self.group, text).await {
                    got[n] = Some(since.elapsed());
                }
            }
            if got.iter().all(Option::is_some)
                || (give_up_early
                    && got.iter().all(Option::is_none)
                    && since.elapsed() > Duration::from_secs(5))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        got
    }
}

/// What each round left: lines that never came, and lines that came late (after a repair).
#[derive(Default, Debug)]
struct Tally {
    rounds: usize,
    lost: [usize; 2],
    lost_by_both: usize,
    late: [usize; 2],
    slowest: Duration,
}

impl Tally {
    fn add(&mut self, got: [Option<Duration>; 2]) {
        self.rounds += 1;
        for (n, at) in got.iter().enumerate() {
            match at {
                None => self.lost[n] += 1,
                Some(at) if *at > Duration::from_secs(2) => self.late[n] += 1,
                Some(_) => {}
            }
            if let Some(at) = at {
                self.slowest = self.slowest.max(*at);
            }
        }
        if got.iter().all(Option::is_none) {
            self.lost_by_both += 1;
        }
    }
}

/// Scenario (i): Ana says a last line and leaves at once, her node staying up; Bea and Cai
/// stay in the room. Measured on loopback: 40 of 40 rounds, both got every line, each within
/// a fraction of a second.
#[tokio::test(flavor = "multi_thread")]
async fn a_last_line_before_leaving_reaches_everyone() {
    const ROUNDS: usize = 40;
    let _one = ONE_AT_A_TIME.lock().await;
    let three = Three::new().await;
    let ana = three.start_ana().await;
    let mut tally = Tally::default();
    for round in 0..ROUNDS {
        three.ana_enters(&ana).await;
        let text = format!("last word {round}");
        let said = Instant::now();
        ana.room_chat(three.group, text.clone()).await.unwrap();
        ana.leave_room(three.group).await.unwrap();
        tally.add(three.arrivals(&text, said, false).await);
        assert!(
            three.ana_gone().await,
            "Ana still listed after round {round}"
        );
    }
    eprintln!("{tally:?}");
    assert_eq!(tally.lost, [0, 0], "{tally:?}");
}

/// Scenario (ii): as (i), and Ana's node stops 400 ms after she left, as the application
/// does when it quits; her next session starts for the next round. `delay_ms` is each way
/// of her links to the others.
async fn quit_after_the_last_line(delay_ms: u64, rounds: usize) -> Tally {
    let three = Three::new().await;
    three.to_bea.delay_ms.store(delay_ms, Ordering::SeqCst);
    three.to_cai.delay_ms.store(delay_ms, Ordering::SeqCst);
    let mut tally = Tally::default();
    for round in 0..rounds {
        let ana = three.start_ana().await;
        three.ana_enters(&ana).await;
        let text = format!("last word {round}");
        let said = Instant::now();
        ana.room_chat(three.group, text.clone()).await.unwrap();
        ana.leave_room(three.group).await.unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        ana.shutdown().await.unwrap();
        drop(ana);
        let got = three.arrivals(&text, said, true).await;
        tally.add(got);
        eprintln!("round {round}: {got:?}");
        // Without her leave, the others let her go after the grace period: her next session
        // enters all the same.
        let _ = three.ana_gone().await;
    }
    eprintln!("{tally:?}");
    tally
}

/// Scenario (ii) on loopback, where each packet takes well under a millisecond: the 400 ms
/// are plenty. Measured: 30 of 30 rounds, both got every line before the first look.
#[tokio::test(flavor = "multi_thread")]
async fn a_last_line_before_quitting_reaches_everyone_on_a_fast_link() {
    let _one = ONE_AT_A_TIME.lock().await;
    let tally = quit_after_the_last_line(0, 5).await;
    assert_eq!(tally.lost, [0, 0], "{tally:?}");
}

/// Scenario (ii) over links of 250 ms each way (a relayed or long-distance connection):
/// quitting waits for the line and the leave to be answered. Measured before that wait,
/// 10 rounds: in 6 neither Bea nor Cai ever got the line, nor did Ana's next session bring
/// it; in 3 one of them missed it and had it 11-19 s later from the other's presence; in 1
/// both got it. With it: 10 of 10 rounds, both had it about 1.2 s after it was said.
#[tokio::test(flavor = "multi_thread")]
async fn a_last_line_before_quitting_reaches_everyone_on_a_slow_link() {
    let _one = ONE_AT_A_TIME.lock().await;
    let tally = quit_after_the_last_line(250, 2).await;
    assert_eq!(tally.lost, [0, 0], "{tally:?}");
}

/// Scenario (iii): Ana's link to Bea is cut as she says a last line and leaves, so that the
/// line to Bea fails on its way (after the 20 s a request is given, and so does the one
/// sent again on the same cut link) while Cai gets it. Cai's presence, every 20 s, names the
/// line, and Bea asks for it: from Ana while she still counts as connected (the cut link: no
/// answer), then from Cai. Measured: 3 of 3 rounds, Bea had it 19.8-20.0 s after it was
/// said; with lines sent again once, 16.3-20.0 s. About 70 s.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow (about 70 s): pins that a member gets from another member a line that failed on its way to it; run by hand"]
async fn a_line_that_failed_on_its_way_comes_from_another_member() {
    const ROUNDS: usize = 3;
    let _one = ONE_AT_A_TIME.lock().await;
    let three = Three::new().await;
    let ana = three.start_ana().await;
    let mut tally = Tally::default();
    for round in 0..ROUNDS {
        three.ana_enters(&ana).await;
        let text = format!("last word {round}");
        three.to_bea.cut(true);
        let said = Instant::now();
        ana.room_chat(three.group, text.clone()).await.unwrap();
        ana.leave_room(three.group).await.unwrap();
        // Long enough for the line to Bea to fail on its way.
        let got = timeout(Duration::from_secs(45), async {
            while !has_line(&three.bea, three.group, &text).await {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            said.elapsed()
        })
        .await
        .ok();
        let at_cai = has_line(&three.cai, three.group, &text).await;
        three.to_bea.cut(false);
        eprintln!("round {round}: Bea got it after {got:?}, Cai has it: {at_cai}");
        tally.add([got, at_cai.then_some(Duration::ZERO)]);
        eventually("Ana connected to Bea again", 60, || async {
            connected(&ana, three.group, &[three.b]).await
        })
        .await;
        assert!(
            three.ana_gone().await,
            "Ana still listed after round {round}"
        );
    }
    eprintln!("{tally:?}");
    assert_eq!(tally.lost, [0, 0], "{tally:?}");
}

/// Scenarios (iv) and (v) with two in the room: Ana's link to Bea is cut while she says a
/// last line and leaves, long enough for both to fail on their way, with nobody else in the
/// room to name it; the connection closes. Ana's own room shows the line all the same. Then:
/// - `node_up`: Ana's node, up all along, connects to Bea again; if that does not bring the
///   line, Ana enters the room again, within the two minutes her presence names her latest
///   events for: Bea asks her for the line.
/// - otherwise Ana's node stops and her next session connects and enters: it has nothing of
///   the line.
async fn back_after_a_line_lost_to_the_only_other(node_up: bool) -> (bool, bool, bool) {
    let three = Three::new().await;
    // Cai goes: Bea is the only other in the room, and nobody carries Ana's way to her.
    three.cai.shutdown().await.unwrap();
    let group = three.group;
    let mut ana = three.start_ana_with(&[three.b]).await;
    ana.join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    let both = [three.a(), three.b];
    eventually("Ana and Bea in the room", 60, || async {
        let (at_a, at_b) = (in_room(&ana, group).await, in_room(&three.bea, group).await);
        both.iter().all(|d| at_a.contains(d) && at_b.contains(d))
    })
    .await;
    three.to_bea.cut(true);
    let text = "last word";
    ana.room_chat(group, text.into()).await.unwrap();
    ana.leave_room(group).await.unwrap();
    // The line and the leave fail on their way after 20 s, and the connection closes after
    // 30 s without a packet: what it still held (a stream given up is finished, not reset,
    // and goes once the way opens) is gone with it.
    let b = three.b;
    eventually("Ana's connection to Bea closed", 60, || async {
        !connected(&ana, group, &[b]).await
    })
    .await;
    three.to_bea.cut(false);
    let shown_at_ana = has_line(&ana, group, text).await;
    let missed = !has_line(&three.bea, group, text).await;
    if !node_up {
        ana.shutdown().await.unwrap();
        drop(ana);
        ana = three.start_ana_with(&[three.b]).await;
    }
    eventually("Ana connected to Bea again", 60, || async {
        connected(&ana, group, &[three.b]).await
    })
    .await;
    let line_at_bea = || async {
        timeout(WITHIN, async {
            while !has_line(&three.bea, group, text).await {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .is_ok()
    };
    let at = Instant::now();
    let mut back = line_at_bea().await;
    eprintln!(
        "back at Bea on connecting again: {back} ({:?})",
        at.elapsed()
    );
    if !back {
        // Ana enters the room again: her presence names her latest events.
        ana.join_room(group, FIRST_ROOM, "ana".into())
            .await
            .unwrap();
        let at = Instant::now();
        back = line_at_bea().await;
        eprintln!(
            "back at Bea on Ana entering again: {back} ({:?})",
            at.elapsed()
        );
    }
    eprintln!("shown at Ana: {shown_at_ana}, missed by Bea: {missed}, back at Bea: {back}");
    (shown_at_ana, missed, back)
}

/// Scenario (iv) with Ana's node up: once connected again, Bea asks her for the room and gets
/// the line only now and then; Ana entering the room again does. Scenario (v): her own room
/// showed it said all along. Measured, 4 runs: Bea always missed the line; on connecting
/// again she got it in 1 run, and in the 3 others within 11 s of Ana entering again (in the
/// first of those the timing was not split, it may have been on connecting). With a line
/// that failed on its way sent again once: 1 run, Bea had it 0.05 s after connecting again.
/// About 50-90 s (the cut waits for the connection to close).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow (about 50-90 s): pins that a line lost to the only other member comes when its author, node up, enters the room again; run by hand"]
async fn a_line_lost_to_the_only_other_comes_when_its_author_enters_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let (shown_at_ana, missed, back) = back_after_a_line_lost_to_the_only_other(true).await;
    assert!(shown_at_ana, "Ana's room does not show her line");
    assert!(missed, "Bea got the line through the cut link");
    assert!(back, "Bea never got the line");
}

/// Scenario (iv) after Ana's node stopped: her next session has nothing of the line, and Bea
/// never gets it. Measured: 3 of 3 runs lost it (nothing within 30 s of the new session
/// connecting, nor within 30 s of it entering the room).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a line that reached nobody before its author's node stopped is lost for good, its author's room having shown it said; fix pending"]
async fn a_line_lost_to_the_only_other_comes_when_its_author_starts_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let (shown_at_ana, missed, back) = back_after_a_line_lost_to_the_only_other(false).await;
    assert!(shown_at_ana, "Ana's room does not show her line");
    assert!(missed, "Bea got the line through the cut link");
    assert!(back, "Bea never got the line");
}

type Hand = Swarm<request_response::cbor::Behaviour<MemberRequest, MemberResponse>>;

/// Bea played by hand, over TCP and QUIC: she proves herself when challenged, and answers
/// every other request but Ana's lines while `hold` is set, which wait for their answer.
/// Each line that comes is noted with the connection it came on.
struct Bea {
    swarm: Hand,
    identity: DeviceIdentity,
    log: GroupLog,
    hold: bool,
    held: Vec<ResponseChannel<MemberResponse>>,
    lines: Vec<([u8; 16], ConnectionId)>,
}

impl Bea {
    fn new(identity: DeviceIdentity, log: GroupLog) -> Self {
        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .unwrap()
            .with_quic()
            .with_behaviour(|_| {
                request_response::cbor::Behaviour::<MemberRequest, MemberResponse>::new(
                    [(StreamProtocol::new(MEMBER_PROTOCOL), ProtocolSupport::Full)],
                    request_response::Config::default(),
                )
            })
            .unwrap()
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
            .build();
        Self {
            swarm,
            identity,
            log,
            hold: false,
            held: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// The id of the line `wire` is, if it is one.
    fn line(&self, wire: &WireMessage) -> Option<[u8; 16]> {
        let sealed = SealedMessage::try_from(wire).ok()?;
        let keys = self.log.current_keys(&self.identity)?;
        let payload = GroupPayload::decode(&sealed.open(&keys).ok()?).ok()?;
        matches!(payload, GroupPayload::RoomChat { .. }).then_some(sealed.header.message_id)
    }

    /// Handles the next event of Bea's side; returns it unless it was a request.
    async fn step(
        &mut self,
    ) -> Option<SwarmEvent<request_response::Event<MemberRequest, MemberResponse>>> {
        let event = self.swarm.select_next_some().await;
        let SwarmEvent::Behaviour(request_response::Event::Message {
            peer,
            connection_id,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
        }) = event
        else {
            return Some(event);
        };
        let answer = match request {
            MemberRequest::Challenge { challenge, .. } => {
                let challenge: [u8; 32] = challenge.try_into().unwrap();
                let keys = self.log.current_keys(&self.identity).unwrap();
                let bea_peer = self.swarm.local_peer_id().to_bytes();
                let ana_peer = peer.to_bytes();
                let proof = MembershipProof::create(
                    &keys,
                    &self.identity,
                    &bea_peer,
                    &ana_peer,
                    &challenge,
                );
                let sealed = proof.seal(&keys, &bea_peer, &ana_peer, &challenge).unwrap();
                MemberResponse::Proof(WireProof::from(&sealed))
            }
            MemberRequest::Deliver(message) | MemberRequest::Spread { message, .. } => {
                if let Some(id) = self.line(&message) {
                    self.lines.push((id, connection_id));
                    if self.hold {
                        self.held.push(channel);
                        return None;
                    }
                }
                MemberResponse::Ack
            }
            _ => MemberResponse::Ack,
        };
        let _ = self.swarm.behaviour_mut().send_response(channel, answer);
        None
    }

    /// Dials Ana at `address` on a connection of its own, and waits for it.
    async fn connect(&mut self, ana: libp2p::PeerId, address: Multiaddr) -> ConnectionId {
        let dial = DialOpts::peer_id(ana)
            .addresses(vec![address])
            .condition(PeerCondition::Always)
            .build();
        let wanted = dial.connection_id();
        self.swarm.dial(dial).unwrap();
        timeout(Duration::from_secs(20), async {
            loop {
                if let Some(SwarmEvent::ConnectionEstablished { connection_id, .. }) =
                    self.step().await
                    && connection_id == wanted
                {
                    return;
                }
            }
        })
        .await
        .expect("connected");
        wanted
    }

    /// Keeps Bea's side going until `done` holds, for up to `seconds`.
    async fn until(&mut self, seconds: u64, done: impl Fn(&Self) -> bool) -> bool {
        timeout(Duration::from_secs(seconds), async {
            while !done(self) {
                self.step().await;
            }
        })
        .await
        .is_ok()
    }
}

/// Scenario (iii) on its own: the line to Bea is lost on its way, its connection closed
/// while another one stays and Bea stays a member, and Ana sends it again at once on the
/// one left, rather than leaving it to a presence of someone else's 20 s on (when there is
/// anyone else). Measured: it came again on the other connection within a second.
#[tokio::test(flavor = "multi_thread")]
async fn a_line_lost_with_its_connection_goes_again_on_another() {
    let _one = ONE_AT_A_TIME.lock().await;
    if let Ok(filter) = std::env::var("FOGATA_TRACE") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_test_writer()
            .try_init();
    }
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let identity = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, identity.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let node_a = node(ana, log.clone(), port_a, Vec::new());
    let ana_peer = node_a.local_peer();
    let mut bea = Bea::new(identity, log);
    let device = bea.identity.public_key();
    let tcp = format!("/ip4/127.0.0.1/tcp/{port_a}").parse().unwrap();
    let first = bea.connect(ana_peer, tcp).await;
    let second = bea.connect(ana_peer, loopback(port_a)).await;
    let counted = timeout(Duration::from_secs(20), async {
        while !connected(&node_a, group, &[device]).await {
            bea.until(1, |_| false).await;
        }
    })
    .await;
    assert!(counted.is_ok(), "Bea is not a member at Ana's");
    // Ana's side counts a connection open a moment after Bea's side does: the line must
    // find the other one there when its own closes.
    bea.until(1, |_| false).await;
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    bea.hold = true;
    node_a.room_chat(group, "last word".into()).await.unwrap();
    assert!(bea.until(10, |b| !b.lines.is_empty()).await, "no line came");
    let (id, on) = bea.lines[0];
    let other = if on == first { second } else { first };
    // The connection the line came on closes before Bea answers it.
    bea.hold = false;
    bea.held.clear();
    assert!(bea.swarm.close_connection(on));
    let closed = Instant::now();
    let again = bea.until(10, |b| b.lines.len() > 1).await;
    let took = closed.elapsed();
    eprintln!("lines: {:?}, again after {took:?}", bea.lines);
    assert!(again, "the line did not come again");
    assert_eq!(bea.lines[1], (id, other), "the line sent again");
    assert!(took < Duration::from_secs(5), "came again after {took:?}");
    // Once, however long Bea's side keeps going.
    bea.until(3, |_| false).await;
    assert_eq!(bea.lines.len(), 2, "{:?}", bea.lines);
}
