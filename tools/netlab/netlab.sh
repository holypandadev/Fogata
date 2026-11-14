#!/usr/bin/env bash
# Simulated internet with home NATs, built in unprivileged Linux network namespaces.
#
# Every scenario gets fresh namespaces: three lab DHT nodes on a "public" network, each in a
# network of its own (203.0.113.0/24, 198.51.100.0/24 and 192.0.2.0/24, as a device takes an
# address for its own from strangers only when peers on three networks agree, one of them
# dialled by the device), which write every announcement they take to
# <logs>/dht<n>.records, and Fogata devices behind NAT routers of different kinds:
#   cone       Linux masquerade: keeps the source port, filters by address and port.
#   symmetric  masquerade fully-random: a new random public port for every destination.
#   public     no NAT at all.
#
# Usage: tools/netlab/netlab.sh [scenario...]     (default: every connection scenario)
#
# Media transport probes (not run by default):
#   probe-direct   cone + cone, packets go directly
#   probe-member   symmetric + symmetric + public member, packets go through the member
# Environment: PROBE_MODE (single, striped4, per-packet), NETEM (netem arguments applied to
# every router's public side, for example "delay 30ms 5ms loss 1%"), PROBE_PACKETS, and
# PROBE_EXTRA for more probe options (for example "--size 1200 --interval-ms 4").
#
# Updates: update-chain (a release spreads from friend to friend, a stranger gets nothing)
# and update-refusals (what a modified copy offers is refused).
#
# Long history (not run by default):
#   group-join-long  as group-join, with a log of about LONG_KB kilobytes (3072 when not
#                    set) made of members added and removed; with NETEM="rate 1mbit" it
#                    joins over a slow link
#
# Mixed versions: OLD_A or OLD_B, the path of an older `fogata`, runs device A or B of the
# friends-link, friends-code, friends-later(-code) and (A only) friends-ex-mutual scenarios
# with it. An older device
# that is given a code of the newer format must refuse it as not a code.
# Needs: unshare, ip, nft. Does not need root.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
# The lab's attack tools (lab-silent, lab-mdns-flood) are behind the CLI's `lab` feature, so
# this builds its own copy, in a target folder of its own: the release build that ships is
# never the one with them. FOGATA_BIN names another program to use as it is.
LAB_TARGET=$ROOT/target/netlab
BIN=${FOGATA_BIN:-$LAB_TARGET/release/fogata}
BIN_A=${OLD_A:-$BIN}
BIN_B=${OLD_B:-$BIN}
SCENARIOS=(selftest cone-cone cone-cone-member cone-symmetric cone-symmetric-member
    symmetric-symmetric-member member-lost-direct home-two-routers home-two-routers-firewall
    home-two-routers-closed group-join friends-link friends-code friends-later friends-later-code
    friends-remove friends-ex-mutual room-files network-check network-check-negative fake-observed inbound-flood
    silent-flood tcp-known tcp-slow mdns-flood mdns-stale dht-liars lan-sealed direct-apart transfer-cone-cone transfer-lan transfer-cone-symmetric
    transfer-two-routers transfer-two-routers-back update-chain update-refusals)

if [[ -z "${NETLAB_INSIDE:-}" ]]; then
    if [[ -z "${FOGATA_BIN:-}" ]]; then
        (cd "$ROOT" && cargo build --release -p fogata --features lab --target-dir "$LAB_TARGET") ||
            { echo "netlab: could not build the CLI with its lab feature" >&2; exit 1; }
    fi
    [[ -x "$BIN" ]] || { echo "netlab: no $BIN" >&2; exit 1; }
    "$BIN" lab-silent --help > /dev/null 2>&1 ||
        { echo "netlab: $BIN has no lab tools: build it with cargo build --release -p fogata --features lab" >&2; exit 1; }
    list=("$@")
    [[ ${#list[@]} -eq 0 ]] && list=("${SCENARIOS[@]}")
    failed=0
    for scenario in "${list[@]}"; do
        NETLAB_INSIDE=1 unshare --user --map-root-user --net --mount "$0" "$scenario" || failed=1
    done
    exit $failed
fi

SCENARIO=$1
WORK=$(mktemp -d "${TMPDIR:-/tmp}/fogata-netlab.XXXXXX")
mount -t tmpfs none /run
mkdir -p /run/netns
PIDS=()
cleanup() {
    for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    wait 2>/dev/null || true
}
trap cleanup EXIT

in_ns() { local ns=$1; shift; ip netns exec "$ns" "$@"; }

# The "internet": one bridge that every public interface plugs into.
ip netns add net
in_ns net ip link add name br0 type bridge
in_ns net ip link set br0 up
LINK=0

# Gives namespace $1 an interface "wan" on the internet with address $2. Every address
# of every network is reached straight on the bridge.
attach() {
    local ns=$1 address=$2
    LINK=$((LINK + 1))
    ip link add name "w$LINK" type veth peer name "p$LINK"
    ip link set "w$LINK" netns "$ns"
    ip link set "p$LINK" netns net
    in_ns "$ns" ip link set "w$LINK" name wan
    in_ns "$ns" ip addr add "$address/24" dev wan
    in_ns "$ns" ip link set wan up
    in_ns "$ns" ip route add default dev wan
    in_ns "$ns" ip link set lo up
    in_ns net ip link set "p$LINK" master br0
    in_ns net ip link set "p$LINK" up
}

public_host() {
    ip netns add "$1"
    attach "$1" "$2"
}

# Device $1 behind a NAT router of kind $3 whose public address is $2.
nat_host() {
    local host=$1 wan=$2 kind=$3
    local router="r-$host" n=$((LINK + 1))
    ip netns add "$router"
    ip netns add "$host"
    attach "$router" "$wan"
    ip link add name lan type veth peer name eth0
    ip link set lan netns "$router"
    ip link set eth0 netns "$host"
    in_ns "$router" ip addr add "192.168.$n.1/24" dev lan
    in_ns "$router" ip link set lan up
    in_ns "$host" ip addr add "192.168.$n.2/24" dev eth0
    in_ns "$host" ip link set eth0 up
    in_ns "$host" ip link set lo up
    in_ns "$host" ip route add default via "192.168.$n.1"
    in_ns "$router" sysctl -qw net.ipv4.ip_forward=1
    nat_rules "$router" "$kind"
}

# Several devices ($2...) on one local network behind one cone NAT router whose public
# address is $1, as at home or in an office: whoever they talk to sees one address.
nat_hosts_shared() {
    local wan=$1; shift
    local router="r-$1" n=$((LINK + 1)) i=2 host
    ip netns add "$router"
    attach "$router" "$wan"
    in_ns "$router" ip link add name lan type bridge
    in_ns "$router" ip addr add "192.168.$n.1/24" dev lan
    in_ns "$router" ip link set lan up
    for host in "$@"; do
        ip netns add "$host"
        ip link add name "s$n-$i" type veth peer name eth0
        ip link set "s$n-$i" netns "$router"
        ip link set eth0 netns "$host"
        in_ns "$router" ip link set "s$n-$i" master lan
        in_ns "$router" ip link set "s$n-$i" up
        in_ns "$host" ip addr add "192.168.$n.$i/24" dev eth0
        in_ns "$host" ip link set eth0 up
        in_ns "$host" ip link set lo up
        in_ns "$host" ip route add default via "192.168.$n.1"
        i=$((i + 1))
    done
    in_ns "$router" sysctl -qw net.ipv4.ip_forward=1
    nat_rules "$router" cone
}

# NAT and filtering of router $1, of kind $2 (cone or symmetric).
nat_rules() {
    local router=$1 kind=$2
    local masquerade="masquerade"
    [[ $kind == symmetric ]] && masquerade="masquerade fully-random"
    in_ns "$router" nft -f - <<EOF
table ip nat {
    chain post { type nat hook postrouting priority 100; oifname "wan" $masquerade; }
}
table inet filter {
    # Like a home router: unsolicited packets to the router itself are dropped. Accepting
    # them would leave a connection-tracking entry that makes Linux pick another public
    # port for the device's own outgoing packets.
    chain input {
        type filter hook input priority 0; policy drop;
        ct state established,related accept
        iifname { "lan", "lo" } accept
    }
    chain forward {
        type filter hook forward priority 0; policy drop;
        ct state established,related accept
        iifname "lan" accept
    }
}
EOF
}

# Two routers at home: the main router (cone NAT, public address $1) with device $2 on its
# network 192.168.1.0/24, and a second router on that network (as a mesh Wi-Fi router can
# be) with device $3 behind it on 192.168.2.0/24. One public address for both, which does
# not lead back inside: the device behind the second router can reach the other at its
# local address, the other cannot reach it at all. With $4 set, device $2 runs a host
# firewall like ufw's default (nothing comes in unasked but mDNS); $4 = "open:<port>" also
# lets that port in.
home_two_routers() {
    local wan=$1 outer=$2 inner=$3 firewall=${4:-}
    ip netns add r-home
    ip netns add r-mesh
    ip netns add "$outer"
    ip netns add "$inner"
    attach r-home "$wan"
    in_ns r-home ip link add name lan type bridge
    in_ns r-home ip addr add 192.168.1.1/24 dev lan
    in_ns r-home ip link set lan up
    ip link add name hl1 type veth peer name eth0
    ip link set hl1 netns r-home
    ip link set eth0 netns "$outer"
    ip link add name hl2 type veth peer name wan
    ip link set hl2 netns r-home
    ip link set wan netns r-mesh
    for port in hl1 hl2; do
        in_ns r-home ip link set "$port" master lan
        in_ns r-home ip link set "$port" up
    done
    in_ns "$outer" ip addr add 192.168.1.3/24 dev eth0
    in_ns "$outer" ip link set eth0 up
    in_ns "$outer" ip link set lo up
    in_ns "$outer" ip route add default via 192.168.1.1
    in_ns r-mesh ip addr add 192.168.1.2/24 dev wan
    in_ns r-mesh ip link set wan up
    in_ns r-mesh ip link set lo up
    in_ns r-mesh ip route add default via 192.168.1.1
    ip link add name lan type veth peer name eth0
    ip link set lan netns r-mesh
    ip link set eth0 netns "$inner"
    in_ns r-mesh ip addr add 192.168.2.1/24 dev lan
    in_ns r-mesh ip link set lan up
    in_ns "$inner" ip addr add 192.168.2.123/24 dev eth0
    in_ns "$inner" ip link set eth0 up
    in_ns "$inner" ip link set lo up
    in_ns "$inner" ip route add default via 192.168.2.1
    for router in r-home r-mesh; do
        in_ns "$router" sysctl -qw net.ipv4.ip_forward=1
        in_ns "$router" nft -f - <<EOF2
table ip nat {
    chain post { type nat hook postrouting priority 100; oifname "wan" masquerade; }
}
table inet filter {
    chain input {
        type filter hook input priority 0; policy drop;
        ct state established,related accept
        iifname { "lan", "lo" } accept
    }
    chain forward {
        type filter hook forward priority 0; policy drop;
        ct state established,related accept
        iifname "lan" accept
    }
}
EOF2
    done
    if [[ -n $firewall ]]; then
        local open=""
        [[ $firewall == open:* ]] && open="udp dport ${firewall#open:} accept
        tcp dport ${firewall#open:} accept"
        in_ns "$outer" nft -f - <<EOF2
table inet host {
    chain input {
        type filter hook input priority 0; policy drop;
        ct state established,related accept
        iifname "lo" accept
        udp dport 5353 accept
        $open
    }
}
EOF2
    fi
}

# Starts a lab DHT node in namespace $1; its QUIC address ends up in $DHT_ADDR. Every
# announcement it takes is written to $WORK/$1.records, and what each peer lists in identify
# to $WORK/$1.identified, as a stranger's DHT node sees them.
start_dht() {
    local ns=$1 port=$2; shift 2
    in_ns "$ns" "$BIN" lab-dht --port "$port" --records "$WORK/$ns.records" \
        --identified "$WORK/$ns.identified" "$@" \
        > "$WORK/$ns.out" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 50); do
        # Its address on the internet, not the loopback one.
        if grep -v "^/ip4/127\." "$WORK/$ns.out" | grep -q "^/ip4/[0-9.]*/udp/$port/quic-v1"; then
            DHT_ADDR=$(grep -v "^/ip4/127\." "$WORK/$ns.out" |
                grep -m1 "^/ip4/[0-9.]*/udp/$port/quic-v1")
            return
        fi
        sleep 0.1
    done
    echo "lab DHT in $ns did not start" >&2
    exit 1
}

# Adds delay, jitter and loss to traffic leaving namespace $1 to the internet, and a smaller
# packet size (WAN_MTU, as on a VPN) when asked.
impair() {
    [[ -n "${WAN_MTU:-}" ]] && in_ns "$1" ip link set dev wan mtu "$WAN_MTU"
    [[ -n "${NETEM:-}" ]] && in_ns "$1" tc qdisc add dev wan root netem $NETEM
    return 0
}

# Media probe: $1 receives, $2 sends to it.
run_probe() {
    local receiver=$1 sender=$2
    local to
    to=$("$BIN" --data-dir "$WORK/data-$receiver" id | awk '/Short form/ {print $3}')
    in_ns "$receiver" timeout 240 "$BIN" --data-dir "$WORK/data-$receiver" media-probe lab \
        --mode "$PROBE_MODE" --no-mdns $BOOT --packets "$PROBE_PACKETS" ${PROBE_EXTRA:-} \
        > "$WORK/$receiver.out" 2>&1 &
    PIDS+=($!)
    in_ns "$sender" timeout 240 "$BIN" --data-dir "$WORK/data-$sender" media-probe lab --send \
        --mode "$PROBE_MODE" --to "$to" --no-mdns $BOOT --packets "$PROBE_PACKETS" \
        ${PROBE_EXTRA:-} > "$WORK/$sender.out" 2>&1 &
    PIDS+=($!)
}

# Runs device $1 (namespace $1) for $DURATION seconds; it says hello after $SAY_AT seconds
# and leaves STAY seconds after listing the members (2 when not set).
run_device() {
    local name=$1; shift
    local data="$WORK/data-$name"
    (sleep "$SAY_AT"; echo "hello from $name"; sleep 8; echo /members; sleep "${STAY:-2}") |
        in_ns "$name" timeout "$DURATION" "$BIN" --data-dir "$data" chat lab --nick "$name" \
            --no-mdns $BOOT "$@" > "$WORK/$name.out" 2>&1 &
    PIDS+=($!)
}

# Checks the simulated NATs themselves with a plain UDP hole punch (needs python3):
# cone against cone must work, symmetric against symmetric must not.
if [[ $SCENARIO == selftest ]]; then
    nat_host A 203.0.113.21 cone
    nat_host B 203.0.113.22 cone
    nat_host C 203.0.113.23 symmetric
    nat_host D 203.0.113.24 symmetric
    punch() {
        in_ns "$1" python3 -I -c '
import socket, sys
port, peer, peer_port = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(("0.0.0.0", port))
s.settimeout(0.2)
for _ in range(25):
    s.sendto(b"x", (peer, peer_port))
    try:
        s.recvfrom(16)
        print("through")
        break
    except socket.timeout:
        pass
else:
    print("blocked")
' "$2" "$3" "$4"
    }
    a=$(punch A 5000 203.0.113.22 5000 & punch B 5000 203.0.113.21 5000; wait)
    c=$(punch C 5000 203.0.113.24 5000 & punch D 5000 203.0.113.23 5000; wait)
    echo "=== selftest"
    echo "cone/cone UDP punch: $(echo $a)   (expected: through through)"
    echo "symmetric/symmetric UDP punch: $(echo $c)   (expected: blocked blocked)"
    [[ $(echo $a) == "through through" && $(echo $c) == "blocked blocked" ]]
    exit
fi

public_host dht1 203.0.113.10
start_dht dht1 4001
DHT1=$DHT_ADDR
public_host dht2 198.51.100.11
start_dht dht2 4001 --peer "$DHT1"
DHT2=$DHT_ADDR
# A third, on a network of its own: a device takes its address from strangers only when
# peers on three networks agree, as on the internet, where the DHT nodes a device's own
# lookups lead to are on many.
public_host dht3 192.0.2.12
start_dht dht3 4001 --peer "$DHT1"
DHT3=$DHT_ADDR
BOOT="--bootstrap $DHT1 --bootstrap $DHT2 --bootstrap $DHT3"
"$BIN" --data-dir "$WORK/creator" group create lab > /dev/null

# Makes devices $@ members of the lab group: the creator (who stays offline) adds each one
# by its device key, then every device gets a copy of the group log.
members() {
    local device key
    for device in "$@"; do
        key=$("$BIN" --data-dir "$WORK/data-$device" id | awk '/Device key/ {print $3}')
        "$BIN" --data-dir "$WORK/creator" group add lab "$key" --nick "$device" > /dev/null
    done
    for device in "$@"; do
        mkdir -p "$WORK/data-$device/logs"
        cp "$WORK/creator/groups.json" "$WORK/data-$device/"
        cp "$WORK/creator/logs/"*.log "$WORK/data-$device/logs/"
    done
}
# Makes devices $1 and $2 friends: $1 shows a friend link, $2 sends the request with it.
befriend() {
    local a=$1 b=$2 link
    in_ns "$a" timeout 200 "$BIN" --data-dir "$WORK/data-$a" friends invite --accept-first \
        --no-mdns $BOOT > "$WORK/$a-invite-$b.out" 2>&1 &
    PIDS+=($!)
    for _ in $(seq 300); do
        grep -q 'fogata:friend:' "$WORK/$a-invite-$b.out" && break
        sleep 0.1
    done
    link=$(grep -o -m1 'fogata:friend:[A-Za-z0-9_-]*' "$WORK/$a-invite-$b.out")
    in_ns "$b" timeout 200 "$BIN" --data-dir "$WORK/data-$b" friends add "$link" --no-mdns $BOOT \
        > "$WORK/$b-add-$a.out" 2>&1 || true
    wait "${PIDS[-1]}" 2>/dev/null || true
}

# A test release key, trusted by every copy here through FOGATA_RELEASE_KEY, and release
# build $REL in $WORK/rel2 with a Linux and a Windows package of random bytes. $RUNS is the
# build the copies here run (the program's own, which grows with every release), $REL the
# next one: a release no newer than the program's is refused, at publish as on the wire.
release_setup() {
    RUNS=$("$BIN" --data-dir "$WORK/build-probe" update status |
        awk '/^This copy: build/ {print $4}' || true)
    [[ $RUNS =~ ^[0-9]+$ ]] || { echo "netlab: cannot tell the build $BIN runs" >&2; exit 1; }
    REL=$((RUNS + 1))
    # The lab's key is sealed like a real one; its passphrase comes from the environment
    # (release --lab).
    export FOGATA_RELEASE_PASSPHRASE=network-lab-release-key
    # The copies here take releases from one another only, never from the releases page.
    export FOGATA_UPDATE_WEB=off
    "$BIN" release --lab keygen --dir "$WORK/key" > "$WORK/keygen.out"
    FOGATA_RELEASE_KEY=$(cat "$WORK/key/release-key.pub")
    export FOGATA_RELEASE_KEY
    head -c 3000000 /dev/urandom > "$WORK/linux.pack"
    head -c 2500000 /dev/urandom > "$WORK/windows.pack"
    echo "A release of the network lab." > "$WORK/notes"
    "$BIN" release --lab sign --key "$WORK/key/release-key" --build "$REL" \
        --version "0.1.0-beta.$REL" --min-build 1 --notes "$WORK/notes" --out "$WORK/rel2" \
        linux-flatpak="$WORK/linux.pack" windows-x64="$WORK/windows.pack" > /dev/null
}

# The short form of device $1's key, as notes name it.
short_id() {
    "$BIN" --data-dir "$WORK/data-$1" id | awk '/Short form/ {print $3}'
}

# An older B from before the codes grew refuses a newer A's code as not a code (its format
# is newer): prints the result and succeeds when B did. An older B that knows the format
# takes the code, and the scenario goes on as with any B.
refused_newer_code() {
    [[ -n ${OLD_B:-} && -z ${OLD_A:-} ]] && grep -q -E '^FGT-.{5}-' "$WORK/shown" || return 1
    grep -q 'looks like FGT-XXXX-XXXX' "$WORK/B-add.out" || return 1
    echo "=== $SCENARIO (older B: $OLD_B)"
    echo "--- shown by A: $(cat "$WORK/shown")"
    echo "--- B-add"
    cat "$WORK/B-add.out"
    echo "result: PASS (the older copy refuses the newer code)   logs: $WORK"
    return 0
}

# Starts `lab-soak` on device $1 with options $2...; its PID ends up in $SOAK.
# $SOAK is the program itself: `ip netns exec` becomes it, where `in_ns ... &` would give
# the shell that runs the function, and a signal sent to that one (a stop, a kill, an
# interrupt) never reaches the program on a bash that keeps that shell.
soak() {
    local device=$1; shift
    ip netns exec "$device" "$BIN" --data-dir "$WORK/data-$device" lab-soak --no-mdns $BOOT "$@" \
        >> "$WORK/$device.out" 2>&1 < /dev/null &
    SOAK=$!
    PIDS+=($SOAK)
}

# Waits for the programs $@ to end, 30 s at most; one still there then is killed, so a
# scenario never waits for ever.
wait_end() {
    local pid running
    for _ in $(seq 60); do
        running=0
        for pid in "$@"; do
            # One that ended and waits to be asked after is no longer running.
            if [[ -r /proc/$pid/status ]] && ! grep -q '^State:[[:space:]]*Z' "/proc/$pid/status"; then
                running=1
            fi
        done
        [[ $running == 0 ]] && break
        sleep 0.5
    done
    for pid in "$@"; do
        if [[ -r /proc/$pid/status ]] && ! grep -q '^State:[[:space:]]*Z' "/proc/$pid/status"; then
            echo "netlab: $pid did not end when asked to; killed" >&2
            kill -9 "$pid" 2> /dev/null || true
        fi
    done
    wait "$@" 2> /dev/null || true
}

# Whether everything device $1 wrote to $3 (named $4 there) reached it once, in order, and
# was confirmed; $2 is $1's name at $3. Prints what it found.
direct_arrived() {
    local from=$1 from_name=$2 to=$3 to_name=$4 mine theirs waiting
    mine=$(awk -v n="$to_name" '$1 == "item" && $2 == n && $3 == "mine" {print $NF}' \
        "$WORK/$from.dump" | sort -n | tr '\n' ' ')
    theirs=$(awk -v n="$from_name" '$1 == "item" && $2 == n && $3 == "theirs" {print $NF}' \
        "$WORK/$to.dump" | tr '\n' ' ')
    waiting=$(grep -c "^item $to_name mine delivered=false" "$WORK/$from.dump")
    echo "    $from -> $to: wrote $(wc -w <<< "$mine"), $to has $(wc -w <<< "$theirs")," \
        "not confirmed $waiting"
    [[ -n $mine && $mine == "$theirs" && $waiting == 0 ]]
}

# Moving an account: A (Marta, a member of the lab group with C) moves to B, a new computer
# elsewhere, with the code B shows; then B talks in the group as Marta. transfer-cone-cone:
# two cone NATs, found on the DHT and punched; transfer-lan: one local network, mDNS only, no
# DHT; transfer-cone-symmetric: a pair that cannot connect, which must say so;
# transfer-two-routers(-back): two routers at home, one public address without a way back
# in, the device on the main router letting only its own port in (no mDNS): the device
# behind the inner router reaches the other's local address, sealed in its announcement
# under a key from the code's nameplate. B (new) is on the main router, or A (old) with -back.
if [[ $SCENARIO == transfer-* ]]; then
    nolan=(--no-mdns) boot=($BOOT) seconds=150 expect=moved recv_extra=() send_extra=()
    case $SCENARIO in
        transfer-cone-cone)
            nat_host A 203.0.113.21 cone
            nat_host B 203.0.113.22 cone ;;
        transfer-lan)
            nat_hosts_shared 203.0.113.30 A B
            nolan=() boot=(--no-dht) ;;
        transfer-cone-symmetric)
            nat_host A 203.0.113.21 cone
            nat_host B 203.0.113.22 symmetric
            seconds=90 expect=no-contact ;;
        transfer-two-routers)
            home_two_routers 203.0.113.30 B A open:31000
            recv_extra=(--port 31000) ;;
        transfer-two-routers-back)
            home_two_routers 203.0.113.30 A B open:31000
            send_extra=(--port 31000) ;;
        *) echo "unknown scenario $SCENARIO" >&2; exit 2 ;;
    esac
    public_host C 203.0.113.50
    "$BIN" --data-dir "$WORK/data-A" profile --name Marta > /dev/null
    "$BIN" --data-dir "$WORK/data-C" profile --name Leo > /dev/null
    members A C
    before=$("$BIN" --data-dir "$WORK/data-A" id | awk '/Device key/ {print $3}')
    in_ns B timeout 300 "$BIN" --data-dir "$WORK/data-B" transfer receive --yes "${nolan[@]}" \
        "${boot[@]}" "${recv_extra[@]}" --name "new computer" > "$WORK/B-receive.out" 2>&1 &
    RECEIVE=$!
    PIDS+=($RECEIVE)
    for _ in $(seq 300); do
        grep -q 'Code: ' "$WORK/B-receive.out" && break
        sleep 0.1
    done
    code=$(grep -o -m1 -E 'Code: [0-9A-Z-]+' "$WORK/B-receive.out" | cut -d' ' -f2)
    started=$(date +%s)
    in_ns A timeout 300 "$BIN" --data-dir "$WORK/data-A" transfer send "$code" --yes \
        --seconds "$seconds" "${nolan[@]}" "${boot[@]}" "${send_extra[@]}" --name "old computer" \
        > "$WORK/A-send.out" 2>&1 || true
    took=$(($(date +%s) - started))
    [[ $expect == moved ]] && wait "$RECEIVE" 2>/dev/null || kill "$RECEIVE" 2>/dev/null || true
    echo "=== $SCENARIO (code $code, the old computer's part took $took s)"
    for file in B-receive A-send; do
        echo "--- $file"
        grep -v -E '^[0-9:]{8} [0-9]+%$' "$WORK/$file.out" || true
    done
    status=$("$BIN" --data-dir "$WORK/data-A" transfer status 2>&1 || true)
    echo "--- A: $status"
    result=0
    if [[ $expect == no-contact ]]; then
        grep -q 'could not reach each other' "$WORK/A-send.out" || result=1
        [[ $status == "No move here." ]] || result=1
        [[ ! -e "$WORK/data-B/identity.key" ]] || result=1
        echo "(expected: no contact, and nothing changed on either side)"
    else
        after=$("$BIN" --data-dir "$WORK/data-B" id 2>/dev/null | awk '/Device key/ {print $3}' || true)
        echo "    identity before: $before"
        echo "    identity on B:   $after"
        [[ -n $after && $after == "$before" ]] || result=1
        [[ $status == *"moved to"* ]] || result=1
        [[ ! -e "$WORK/data-A/identity.key" ]] || result=1
        # B, now Marta, and C talk in the group.
        DURATION=95 SAY_AT=70
        run_device B
        run_device C
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        for pair in "B C" "C B"; do
            set -- $pair
            if grep -q "hello from $2" "$WORK/$1.out"; then
                echo "    $1 received the message from $2"
            else
                echo "    $1: NO message from $2"
                result=1
            fi
        done
    fi
    echo "result: $([[ $result == 0 ]] && echo PASS || echo FAIL)   logs: $WORK"
    exit $result
fi

DURATION=95
SAY_AT=70

case $SCENARIO in
    update-chain)
        # A publishes the next release ($REL, see release_setup). B is A's friend; C is B's
        # friend but not A's, and runs on another platform (Windows): C can only get its package through B, which fetches
        # it from A because C asked it for one. D, a stranger that dials B, gets nothing.
        nat_host A 203.0.113.21 cone
        public_host B 203.0.113.30
        nat_host C 203.0.113.23 cone
        public_host D 203.0.113.31
        for device in A B C; do
            "$BIN" --data-dir "$WORK/data-$device" profile --name "$device" > /dev/null
        done
        befriend A B
        befriend B C
        release_setup
        "$BIN" --data-dir "$WORK/data-A" release publish "$WORK/rel2" > "$WORK/A-publish.out"
        in_ns A timeout 300 "$BIN" --data-dir "$WORK/data-A" update run --for 280 --no-mdns \
            $BOOT > "$WORK/A.out" 2>&1 &
        PIDS+=($!)
        in_ns B timeout 300 "$BIN" --data-dir "$WORK/data-B" update run --for 280 --port 4100 \
            --no-mdns $BOOT > "$WORK/B.out" 2>&1 &
        PIDS+=($!)
        in_ns C timeout 300 "$BIN" --data-dir "$WORK/data-C" update run --for 280 --until-ready \
            --platform windows-x64 --no-mdns $BOOT > "$WORK/C.out" 2>&1 &
        c_pid=$!
        PIDS+=($c_pid)
        sleep 15
        in_ns D timeout 90 "$BIN" --data-dir "$WORK/data-D" lab-update-probe --no-mdns --no-dht \
            --peer /ip4/203.0.113.30/udp/4100/quic-v1 > "$WORK/D.out" 2>&1 || true
        wait "$c_pid" 2>/dev/null || true
        a_id=$(short_id A) b_id=$(short_id B)
        echo "=== $SCENARIO"
        for device in A B C D; do
            echo "--- $device"
            grep -E 'update:|state: (ready|failed)|ask:|fetch:' "$WORK/$device.out" | sort -u || true
        done
        result=0
        grep -q "package linux-flatpak of build $REL from $a_id" "$WORK/B.out" || result=1
        grep -q "package windows-x64 of build $REL from $a_id" "$WORK/B.out" || result=1
        grep -q "build $REL (0.1.0-beta.$REL) from $b_id" "$WORK/C.out" || result=1
        grep -q "package windows-x64 of build $REL from $b_id" "$WORK/C.out" || result=1
        grep -q 'state: ready to install' "$WORK/C.out" || result=1
        cmp -s "$WORK/windows.pack" "$WORK/data-C/updates/windows-x64.pack" || result=1
        grep -q "from $a_id" "$WORK/C.out" && result=1
        grep -q 'ask: refused' "$WORK/D.out" || result=1
        grep -q 'fetch: refused' "$WORK/D.out" || result=1
        echo "(expected: B gets both packages from A, C gets its own from B only, D nothing)"
        echo "result: $([[ $result == 0 ]] && echo PASS || echo FAIL)   logs: $WORK"
        exit $result ;;
    update-refusals)
        # B holds the next release ($REL, see release_setup). L, a friend of B, runs a
        # modified copy that offers B, one after the other: a release signed by an unknown
        # key, one whose signature does not match, an older build (a downgrade), one further
        # above B's build than a release key may go, one with a package over the size limit,
        # and a good release $REL + 1 whose chunks it damages. B refuses each; that release,
        # which is good, takes $REL's place, but none of its damaged package is kept.
        public_host B 203.0.113.30
        nat_host L 203.0.113.21 cone
        for device in B L; do
            "$BIN" --data-dir "$WORK/data-$device" profile --name "$device" > /dev/null
        done
        befriend B L
        release_setup
        "$BIN" release --lab keygen --dir "$WORK/key2" > /dev/null
        "$BIN" release --lab sign --key "$WORK/key2/release-key" --build $((REL + 3)) --version 5 --min-build 1 \
            --out "$WORK/rel-unknown" linux-flatpak="$WORK/linux.pack" > /dev/null
        cp -r "$WORK/rel2" "$WORK/rel-tampered"
        sed -i 's/A release of the network lab./A release of the network lap./' \
            "$WORK/rel-tampered/release.json"
        sed -i "s/\"build\": $REL/\"build\": $((REL + 2))/" "$WORK/rel-tampered/release.json"
        "$BIN" release --lab sign --key "$WORK/key/release-key" --build "$RUNS" --version "0.1.0-beta.$RUNS" \
            --min-build 1 --out "$WORK/rel1" linux-flatpak="$WORK/linux.pack" > /dev/null
        # Further above B's build ($RUNS) than a release key may go.
        "$BIN" release --lab sign --key "$WORK/key/release-key" --build $((RUNS + 1001)) --version "0.1.0-beta.$((RUNS + 1001))" \
            --min-build 1 --out "$WORK/rel-far" linux-flatpak="$WORK/linux.pack" > /dev/null
        python3 -I -c '
import json, sys
m = json.load(open(sys.argv[1]))
m["build"] = int(sys.argv[3])
m["artifacts"]["linux-flatpak"]["size"] = 500 * 1024 * 1024
json.dump(m, open(sys.argv[2], "w"), indent=2)
' "$WORK/rel2/release.json" "$WORK/big.json" $((REL + 4))
        "$BIN" lab-sign-raw --key "$WORK/key/release-key" "$WORK/big.json" --out "$WORK/rel-big"
        head -c 3500000 /dev/urandom > "$WORK/linux3.pack"
        "$BIN" release --lab sign --key "$WORK/key/release-key" --build $((REL + 1)) --version "0.1.0-beta.$((REL + 1))" \
            --min-build 1 --out "$WORK/rel3" linux-flatpak="$WORK/linux3.pack" > /dev/null
        "$BIN" --data-dir "$WORK/data-B" release publish "$WORK/rel2" > "$WORK/B-publish.out"
        in_ns B timeout 600 "$BIN" --data-dir "$WORK/data-B" update run --for 580 --port 4100 \
            --no-mdns $BOOT > "$WORK/B.out" 2>&1 &
        b_pid=$!
        PIDS+=($b_pid)
        result=0
        # Runs L with release folder $2 planted, until B's output says $3 or 100 s pass.
        liar() {
            local name=$1 from=$2 expect=$3 pid; shift 3
            rm -rf "${WORK:?}/data-L/updates"
            "$BIN" --data-dir "$WORK/data-L" lab-plant-release "$from" > /dev/null
            in_ns L timeout 110 "$BIN" --data-dir "$WORK/data-L" update run --lab-unchecked \
                --for 105 --no-mdns $BOOT "$@" > "$WORK/L-$name.out" 2>&1 &
            pid=$!
            for _ in $(seq 100); do
                grep -q -E "$expect" "$WORK/B.out" && break
                sleep 1
            done
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
            if grep -q -E "$expect" "$WORK/B.out"; then
                echo "    $name: refused" >> "$WORK/cases"
            else
                echo "    $name: NOT refused" >> "$WORK/cases"
                result=1
            fi
        }
        liar unknown-key "$WORK/rel-unknown" 'refused a release from .*key this copy does not know'
        liar bad-signature "$WORK/rel-tampered" "refused a release from .*signature is not good"
        liar downgrade "$WORK/rel1" "refused build $RUNS from .*not newer than build $REL"
        liar too-far "$WORK/rel-far" "refused build $((RUNS + 1001)) from .*further ahead of build $RUNS"
        liar oversize "$WORK/rel-big" 'refused a release from .*larger than'
        liar damaged-chunk "$WORK/rel3" 'arrived damaged' --lab-damage-chunks
        kill "$b_pid" 2>/dev/null || true
        wait "$b_pid" 2>/dev/null || true
        "$BIN" --data-dir "$WORK/data-B" update status > "$WORK/B-status.out" 2>&1 || true
        echo "=== $SCENARIO"
        cat "$WORK/cases"
        echo "--- B"
        grep -E 'update:' "$WORK/B.out" | sort -u || true
        cat "$WORK/B-status.out"
        # Release $REL + 1 itself is good, so B keeps its manifest, but none of its package came.
        grep -q "build $((REL + 1))" "$WORK/B-status.out" || result=1
        grep -q 'ready to install' "$WORK/B-status.out" && result=1
        [[ -e "$WORK/data-B/updates/linux-flatpak.pack" ]] && result=1
        grep -q -E "build ($((REL + 2))|$((REL + 3))|$((REL + 4))|$((RUNS + 1001)))\b" "$WORK/B-status.out" && result=1
        echo "(expected: each one refused; build $((REL + 1)) known, its package not taken)"
        echo "result: $([[ $result == 0 ]] && echo PASS || echo FAIL)   logs: $WORK"
        exit $result ;;
    cone-cone)
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        DEVICES=(A B) ;;
    cone-cone-move)
        # B moves to another network while connected (from home Wi-Fi to a phone's, say):
        # its router gets another public address and forgets every mapping. A has to find
        # B's new address and punch through to it again.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        MOVE=B MOVE_AT=100 DURATION=280 SAY_AT=260
        DEVICES=(A B) ;;
    member-lost-direct)
        # A and B are connected directly and both to C. Then nothing passes between A and B
        # any more (Wi-Fi client isolation, a NAT that drops its mapping), while both still
        # reach C: they must find each other again through C, and soon. Later the way
        # between them opens again.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        public_host C 203.0.113.50
        # Bound: up to 10 s of silence before a device looks for another way, its 2 s
        # check, then a roster and a dial through C; well under the 30 s after which QUIC
        # would close the silent connection by itself.
        CUT="A=203.0.113.21 B=203.0.113.22" CUT_AT=60 CUT_UNTIL=150 CUT_WITHIN=${CUT_WITHIN:-20}
        DURATION=215 SAY_AT=110 STAY=85
        DEVICES=(A B C) ;;
    cone-symmetric)
        # Known limit: without a member in between these two cannot connect.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 symmetric
        EXPECT_NO_CONTACT=1
        DEVICES=(A B) ;;
    cone-symmetric-member)
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 symmetric
        public_host C 203.0.113.50
        DEVICES=(A B C) ;;
    symmetric-symmetric-member)
        nat_host A 203.0.113.21 symmetric
        nat_host B 203.0.113.22 symmetric
        public_host C 203.0.113.50
        DEVICES=(A B C) ;;
    home-two-routers)
        home_two_routers 203.0.113.30 A B
        DEVICES=(A B) ;;
    home-two-routers-firewall)
        # The firewall of the device on the main router lets its own port in.
        home_two_routers 203.0.113.30 A B open:31000
        EXTRA_A="--port 31000"
        DEVICES=(A B) ;;
    home-two-routers-closed)
        # Known limit: nothing gets in to A unasked and B cannot be reached, so no path.
        home_two_routers 203.0.113.30 A B closed
        EXPECT_NO_CONTACT=1
        DEVICES=(A B) ;;
    cone-cone-member)
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        public_host C 203.0.113.50
        DEVICES=(A B C) ;;
    group-join)
        # A creates a group and stays online; B, behind another NAT, joins with an invite
        # found only through the DHT, then both chat.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        "$BIN" --data-dir "$WORK/data-A" group create club > /dev/null
        "$BIN" --data-dir "$WORK/data-A" group invite club | tail -n1 > "$WORK/invite-B"
        (sleep 110; echo "hello from A"; sleep 30) |
            in_ns A timeout 150 "$BIN" --data-dir "$WORK/data-A" chat club --nick A --accept-joins --no-mdns $BOOT \
            > "$WORK/A.out" 2>&1 &
        PIDS+=($!)
        in_ns B timeout 90 "$BIN" --data-dir "$WORK/data-B" group join "$(cat "$WORK/invite-B")" \
            --nick B --no-mdns $BOOT > "$WORK/B-join.out" 2>&1 || true
        (sleep 60; echo "hello from B"; sleep 40) |
            in_ns B timeout 105 "$BIN" --data-dir "$WORK/data-B" chat club --nick B --no-mdns $BOOT \
            > "$WORK/B.out" 2>&1 || true
        wait "${PIDS[-1]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        tail -n2 "$WORK/B-join.out"
        grep -h -E 'connected|hello from|joined' "$WORK/A.out" "$WORK/B.out" | sort -u
        if grep -q 'Joined' "$WORK/B-join.out" && grep -q 'hello from A' "$WORK/B.out" \
            && grep -q 'hello from B' "$WORK/A.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    group-join-long)
        # A's group has a long history: members added and removed, each removal a new key
        # sealed to everyone left. B, behind another NAT, joins it with an invite over links
        # as NETEM shapes them, then both chat.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        impair r-A
        impair r-B
        "$BIN" --data-dir "$WORK/data-A" group create club > /dev/null
        keys=() shorts=()
        for i in $(seq 18); do
            "$BIN" --data-dir "$WORK/id-$i" id > "$WORK/id-$i.out"
            keys+=("$(awk '/Device key/ {print $3}' "$WORK/id-$i.out")")
            shorts+=("$(awk '/Short form/ {print $3}' "$WORK/id-$i.out")")
        done
        for key in "${keys[@]}"; do
            "$BIN" --data-dir "$WORK/data-A" group add club "$key" > /dev/null
        done
        want=$(( ${LONG_KB:-3072} * 1024 ))
        while (( $(cat "$WORK"/data-A/logs/*.log | wc -c) < want )); do
            for _ in $(seq 10); do
                "$BIN" --data-dir "$WORK/data-A" group remove club "${shorts[0]}" > /dev/null
                "$BIN" --data-dir "$WORK/data-A" group add club "${keys[0]}" > /dev/null
            done
        done
        echo "log of A: $(cat "$WORK"/data-A/logs/*.log | wc -c) bytes"
        "$BIN" --data-dir "$WORK/data-A" group invite club | tail -n1 > "$WORK/invite-B"
        # A says hello every 15 s, so B hears one whenever its join ends.
        (for _ in $(seq 24); do sleep 15; echo "hello from A"; done) |
            in_ns A timeout 370 "$BIN" --data-dir "$WORK/data-A" chat club --nick A --accept-joins --no-mdns $BOOT \
            > "$WORK/A.out" 2>&1 &
        PIDS+=($!)
        started=$(date +%s)
        in_ns B timeout 300 "$BIN" --data-dir "$WORK/data-B" group join "$(cat "$WORK/invite-B")" \
            --nick B --no-mdns $BOOT > "$WORK/B-join.out" 2>&1 || true
        echo "join took $(( $(date +%s) - started )) s"
        (sleep 20; echo "hello from B"; sleep 40) |
            in_ns B timeout 65 "$BIN" --data-dir "$WORK/data-B" chat club --nick B --no-mdns $BOOT \
            > "$WORK/B.out" 2>&1 || true
        wait "${PIDS[-1]}" 2>/dev/null || true
        echo "=== $SCENARIO netem='${NETEM:-none}'"
        tail -n2 "$WORK/B-join.out"
        grep -h -E 'connected|hello from|joined' "$WORK/A.out" "$WORK/B.out" | sort -u
        if grep -q 'Joined' "$WORK/B-join.out" && grep -q 'hello from A' "$WORK/B.out" \
            && grep -q 'hello from B' "$WORK/A.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    friends-link|friends-code)
        # A and B, behind different NATs, become friends with a link (or a short code, which
        # carries no address and is found only through the DHT). Then both come back later
        # and find each other as friends.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        "$BIN_A" --data-dir "$WORK/data-A" profile --name Ana > /dev/null
        "$BIN_B" --data-dir "$WORK/data-B" profile --name Bea > /dev/null
        flag=() pattern='fogata:friend:[A-Za-z0-9_-]*'
        if [[ $SCENARIO == friends-code ]]; then
            flag=(--code) pattern='FGT-[0-9A-Z]{4,5}-[0-9A-Z]{4,5}'
        fi
        in_ns A timeout 200 "$BIN_A" --data-dir "$WORK/data-A" friends invite "${flag[@]}" \
            --accept-first --no-mdns $BOOT > "$WORK/A-invite.out" 2>&1 &
        PIDS+=($!)
        for _ in $(seq 300); do
            grep -q -E "$pattern" "$WORK/A-invite.out" && break
            sleep 0.1
        done
        what=$(grep -o -m1 -E "$pattern" "$WORK/A-invite.out")
        echo "$what" > "$WORK/shown"
        in_ns B timeout 200 "$BIN_B" --data-dir "$WORK/data-B" friends add "$what" --no-mdns $BOOT \
            > "$WORK/B-add.out" 2>&1 || true
        if refused_newer_code; then
            kill "${PIDS[-1]}" 2>/dev/null || true
            exit 0
        fi
        wait "${PIDS[-1]}" 2>/dev/null || true
        for device in A B; do
            bin=$BIN_A
            [[ $device == B ]] && bin=$BIN_B
            in_ns "$device" timeout 100 "$bin" --data-dir "$WORK/data-$device" friends online \
                --no-mdns $BOOT > "$WORK/$device-online.out" 2>&1 &
            PIDS+=($!)
        done
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for file in A-invite B-add A-online B-online; do
            echo "--- $file"
            grep -E '^[0-9:]{8} [-+?·]' "$WORK/$file.out" | grep -v 'waiting for a friend' || true
        done
        if grep -q 'friends now' "$WORK/B-add.out" && grep -q 'Bea .* is online' "$WORK/A-online.out" \
            && grep -q 'Ana .* is online' "$WORK/B-online.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    network-check)
        # `fogata network` behind an easy NAT, a hard NAT, with a public address, and behind
        # an easy NAT on a network that drops every UDP packet going out (D, which meets the
        # DHT nodes over TCP). The third DHT node gives enough reports to tell the NAT kinds
        # apart, and D three peers alive over TCP that it can ask over QUIC.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 symmetric
        public_host C 203.0.113.50
        nat_host D 203.0.113.24 cone
        in_ns D nft -f - <<EOF
table inet block {
    chain output { type filter hook output priority 0; policy accept; meta l4proto udp drop; }
}
EOF
        for device in A B C; do
            in_ns "$device" timeout 60 "$BIN" --data-dir "$WORK/data-$device" network --no-mdns \
                $BOOT > "$WORK/$device.out" 2>&1 &
            PIDS+=($!)
        done
        tcp_boot=()
        for dht in "$DHT1" "$DHT2" "$DHT3"; do
            tcp_boot+=(--bootstrap "${dht/\/udp\/4001\/quic-v1//tcp/4001}")
        done
        in_ns D timeout 60 "$BIN" --data-dir "$WORK/data-D" network --no-mdns "${tcp_boot[@]}" \
            > "$WORK/D.out" 2>&1 &
        PIDS+=($!)
        wait "${PIDS[-1]}" "${PIDS[-2]}" "${PIDS[-3]}" "${PIDS[-4]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for device in A B C D; do
            echo "--- $device"
            grep '^\[' "$WORK/$device.out" || true
        done
        # UDP is said to be blocked where it is, and only there.
        if grep -q 'easy NAT' "$WORK/A.out" && grep -q 'hard NAT' "$WORK/B.out" \
            && grep -q 'public IPv4' "$WORK/C.out" && grep -q 'UDP seems blocked' "$WORK/D.out" \
            && ! grep -q 'UDP seems blocked' "$WORK/A.out" "$WORK/B.out" "$WORK/C.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    network-check-negative)
        # What `fogata network` must not say. E, behind an easy NAT, has its UDP dropped
        # towards two of the three DHT nodes only, whose QUIC is dead from there while TCP
        # works: UDP still works, not blocked. F has no way out at all: nothing answers,
        # however many DHT nodes its table holds from the bootstrap list.
        ip1=$(echo "$DHT1" | cut -d/ -f3)
        ip2=$(echo "$DHT2" | cut -d/ -f3)
        nat_host E 203.0.113.25 cone
        in_ns E nft -f - <<EOF
table inet block {
    chain output { type filter hook output priority 0; policy accept; ip daddr { $ip1, $ip2 } meta l4proto udp drop; }
}
EOF
        nat_host F 203.0.113.26 cone
        in_ns F nft -f - <<EOF
table inet block {
    chain output { type filter hook output priority 0; policy drop; oif lo accept; }
}
EOF
        tcp_boot=()
        for dht in "$DHT1" "$DHT2" "$DHT3"; do
            tcp_boot+=(--bootstrap "${dht/\/udp\/4001\/quic-v1//tcp/4001}")
        done
        in_ns E timeout 60 "$BIN" --data-dir "$WORK/data-E" network --no-mdns "${tcp_boot[@]}" \
            > "$WORK/E.out" 2>&1 &
        PIDS+=($!)
        in_ns F timeout 60 "$BIN" --data-dir "$WORK/data-F" network --no-mdns $BOOT \
            > "$WORK/F.out" 2>&1 &
        PIDS+=($!)
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for device in E F; do
            echo "--- $device"
            grep '^\[' "$WORK/$device.out" || true
        done
        if grep -q 'UDP works' "$WORK/E.out" && ! grep -q 'UDP seems blocked' "$WORK/E.out" \
            && grep -q 'does not seem to reach the internet' "$WORK/F.out" \
            && ! grep -q 'UDP' "$WORK/F.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    room-files)
        # A and B are behind NATs that change ports per destination, so they reach each
        # other only through C. A shares a file in the room; B, which entered later, gets
        # it through C and checks it.
        nat_host A 203.0.113.21 symmetric
        nat_host B 203.0.113.22 symmetric
        public_host C 203.0.113.50
        members A B C
        head -c 3000000 /dev/urandom > "$WORK/photo.bin"
        mkdir -p "$WORK/dl-B"
        (sleep 150) |
            in_ns C timeout 150 "$BIN" --data-dir "$WORK/data-C" chat lab --nick C --no-mdns $BOOT \
            > "$WORK/C.out" 2>&1 &
        PIDS+=($!)
        (sleep 50; echo "/share $WORK/photo.bin"; sleep 95) |
            in_ns A timeout 150 "$BIN" --data-dir "$WORK/data-A" chat lab --nick A --no-mdns $BOOT \
            > "$WORK/A.out" 2>&1 &
        PIDS+=($!)
        sleep 30
        (sleep 60; echo /files; echo "/get 1"; sleep 50; echo /files; sleep 5) |
            in_ns B timeout 120 "$BIN" --data-dir "$WORK/data-B" chat lab --nick B --no-mdns $BOOT \
                --downloads "$WORK/dl-B" > "$WORK/B.out" 2>&1 || true
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for device in A B C; do
            echo "--- $device"
            grep -E '^[0-9:]{8} [-+·] ' "$WORK/$device.out" |
                grep -E 'room|share|getting|saved|%|gone|from|nobody|connected \(through' || true
        done
        if cmp -s "$WORK/photo.bin" "$WORK/dl-B/photo.bin"; then
            echo "result: PASS (the file arrived intact)   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    friends-remove)
        # A is friends with B and C, then removes C while nobody is online. When all three
        # come back, B gets A's new friend secret across the NATs and they meet; C finds
        # nobody.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        nat_host C 203.0.113.23 cone
        for device in A B C; do
            "$BIN" --data-dir "$WORK/data-$device" profile --name "$device" > /dev/null
        done
        for device in B C; do
            in_ns A timeout 200 "$BIN" --data-dir "$WORK/data-A" friends invite --accept-first \
                --no-mdns $BOOT > "$WORK/A-invite-$device.out" 2>&1 &
            PIDS+=($!)
            for _ in $(seq 300); do
                grep -q 'fogata:friend:' "$WORK/A-invite-$device.out" && break
                sleep 0.1
            done
            link=$(grep -o -m1 'fogata:friend:[A-Za-z0-9_-]*' "$WORK/A-invite-$device.out")
            in_ns "$device" timeout 200 "$BIN" --data-dir "$WORK/data-$device" friends add "$link" \
                --no-mdns $BOOT > "$WORK/$device-add.out" 2>&1 || true
            wait "${PIDS[-1]}" 2>/dev/null || true
        done
        "$BIN" --data-dir "$WORK/data-A" friends remove C > "$WORK/A-remove.out"
        for device in A B C; do
            in_ns "$device" timeout 150 "$BIN" --data-dir "$WORK/data-$device" friends online \
                --no-mdns $BOOT > "$WORK/$device-online.out" 2>&1 &
            PIDS+=($!)
        done
        wait "${PIDS[-1]}" "${PIDS[-2]}" "${PIDS[-3]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for file in B-add C-add A-remove A-online B-online C-online; do
            echo "--- $file"
            grep -E '^([0-9:]{8} [-+·]|Forgot|This device)' "$WORK/$file.out" || true
        done
        if grep -q 'friends now' "$WORK/C-add.out" && grep -q 'B .* is online' "$WORK/A-online.out" \
            && grep -q 'A .* is online' "$WORK/B-online.out" \
            && ! grep -q 'is online' "$WORK/C-online.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    friends-ex-mutual)
        # A is friends with M and E, and E with M too. A removes E while nobody is online.
        # A comes back first and waits for M; then M and E come back, and A gives M its new
        # friend secret. E, which looks M up under M's own secret as before, must not find A
        # there (nor A's address) at any time.
        nat_host A 203.0.113.21 cone
        nat_host M 203.0.113.22 cone
        nat_host E 203.0.113.23 cone
        "$BIN_A" --data-dir "$WORK/data-A" profile --name A > /dev/null
        for device in M E; do
            "$BIN" --data-dir "$WORK/data-$device" profile --name "$device" > /dev/null
        done
        befriend() {
            local inviter=$1 invited=$2 bin=$BIN
            [[ $inviter == A ]] && bin=$BIN_A
            in_ns "$inviter" timeout 200 "$bin" --data-dir "$WORK/data-$inviter" friends invite \
                --accept-first --no-mdns $BOOT > "$WORK/$inviter-invite-$invited.out" 2>&1 &
            PIDS+=($!)
            for _ in $(seq 300); do
                grep -q 'fogata:friend:' "$WORK/$inviter-invite-$invited.out" && break
                sleep 0.1
            done
            link=$(grep -o -m1 'fogata:friend:[A-Za-z0-9_-]*' "$WORK/$inviter-invite-$invited.out")
            in_ns "$invited" timeout 200 "$BIN" --data-dir "$WORK/data-$invited" friends add "$link" \
                --no-mdns $BOOT > "$WORK/$invited-add-$inviter.out" 2>&1 || true
            wait "${PIDS[-1]}" 2>/dev/null || true
        }
        befriend A M
        befriend A E
        befriend E M
        "$BIN_A" --data-dir "$WORK/data-A" friends remove E > "$WORK/A-remove.out"
        cat "$WORK/dht1.records" "$WORK/dht2.records" > "$WORK/records-before"
        # A comes back first and waits a while for M, announcing for it.
        in_ns A timeout 180 "$BIN_A" --data-dir "$WORK/data-A" friends online --no-mdns $BOOT \
            > "$WORK/A-online.out" 2>&1 &
        PIDS+=($!)
        sleep 80
        for device in M E; do
            in_ns "$device" timeout 100 "$BIN" --data-dir "$WORK/data-$device" friends online \
                --no-mdns $BOOT > "$WORK/$device-online.out" 2>&1 &
            PIDS+=($!)
        done
        wait "${PIDS[-1]}" "${PIDS[-2]}" "${PIDS[-3]}" 2>/dev/null || true
        cat "$WORK/dht1.records" "$WORK/dht2.records" > "$WORK/records"
        # Only what was announced after the removal.
        tail -n +$(($(wc -l < "$WORK/records-before") + 1)) "$WORK/records" > "$WORK/records-after"
        "$BIN" --data-dir "$WORK/data-E" lab-open "$WORK/records-after" > "$WORK/E-open.out"
        echo "=== $SCENARIO"
        for file in M-add-A E-add-A M-add-E A-remove A-online M-online E-online; do
            echo "--- $file"
            grep -E '^([0-9:]{8} [-+·]|Forgot|This device)' "$WORK/$file.out" || true
        done
        echo "--- what E finds under its friends' secrets"
        grep '^announced under' "$WORK/E-open.out" || true
        pass=1
        grep -q 'M .* is online' "$WORK/A-online.out" || pass=0
        grep -q 'A .* is online' "$WORK/M-online.out" || pass=0
        grep -q 'M .* is online' "$WORK/E-online.out" || pass=0
        grep -q '^announced under friend M: ' "$WORK/E-open.out" || pass=0
        ! grep -E '^announced under friend M: ' "$WORK/E-open.out" | grep -q 203.0.113.21 || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    friends-later|friends-later-code)
        # B sends a friend request (with a link, or a short code) that A does not answer while
        # B is online. A accepts later, offline; when both come back, A must find B to
        # deliver the acceptance.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        "$BIN_A" --data-dir "$WORK/data-A" profile --name Ana > /dev/null
        "$BIN_B" --data-dir "$WORK/data-B" profile --name Bea > /dev/null
        flag=() pattern='fogata:friend:[A-Za-z0-9_-]*'
        if [[ $SCENARIO == friends-later-code ]]; then
            flag=(--code) pattern='FGT-[0-9A-Z]{4,5}-[0-9A-Z]{4,5}'
        fi
        in_ns A timeout 150 "$BIN_A" --data-dir "$WORK/data-A" friends invite "${flag[@]}" \
            --no-mdns $BOOT > "$WORK/A-invite.out" 2>&1 &
        PIDS+=($!)
        for _ in $(seq 300); do
            grep -q -E "$pattern" "$WORK/A-invite.out" && break
            sleep 0.1
        done
        what=$(grep -o -m1 -E "$pattern" "$WORK/A-invite.out")
        echo "$what" > "$WORK/shown"
        in_ns B timeout 120 "$BIN_B" --data-dir "$WORK/data-B" friends add "$what" --no-mdns $BOOT \
            > "$WORK/B-add.out" 2>&1 || true
        kill "${PIDS[-1]}" 2>/dev/null || true
        refused_newer_code && exit 0
        "$BIN_A" --data-dir "$WORK/data-A" friends accept Bea > /dev/null
        for device in A B; do
            bin=$BIN_A
            [[ $device == B ]] && bin=$BIN_B
            in_ns "$device" timeout 120 "$bin" --data-dir "$WORK/data-$device" friends online \
                --no-mdns $BOOT > "$WORK/$device-online.out" 2>&1 &
            PIDS+=($!)
        done
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        echo "=== $SCENARIO"
        for file in A-invite B-add A-online B-online; do
            echo "--- $file"
            grep -E '^[0-9:]{8} [-+?·]' "$WORK/$file.out" | grep -v 'waiting for a friend' || true
        done
        if grep -q 'request delivered' "$WORK/B-add.out" && grep -q 'friends now' "$WORK/B-online.out" \
            && grep -q 'Bea .* is online' "$WORK/A-online.out"; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    direct-apart)
        # A and B, behind different NATs, are friends and write to each other every 2 s. B
        # sleeps for 40 s (stopped, as a closed laptop), then A crashes and starts again.
        # What was written meanwhile arrives once both are back: each message once, in order,
        # and confirmed.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        "$BIN" --data-dir "$WORK/data-A" profile --name Ana > /dev/null
        "$BIN" --data-dir "$WORK/data-B" profile --name Bea > /dev/null
        befriend A B
        soak A --every 2000 --rounds 45
        pid_a=$SOAK
        soak B --every 2000 --rounds 60
        pid_b=$SOAK
        sleep 25
        date -u +%T > "$WORK/stopped-B"
        kill -STOP "$pid_b"
        sleep 40
        kill -CONT "$pid_b"
        sleep 20
        kill -9 "$pid_a"
        sleep 5
        soak A --every 2000 --rounds 10
        pid_a=$SOAK
        sleep 100
        kill -INT "$pid_a" "$pid_b" 2>/dev/null
        wait_end "$pid_a" "$pid_b"
        for device in A B; do
            "$BIN" --data-dir "$WORK/data-$device" lab-soak --dump > "$WORK/$device.dump" 2>&1
        done
        echo "=== $SCENARIO"
        grep -h 'friends now' "$WORK/B-add-A.out" || true
        for device in A B; do
            echo "--- $device"
            grep -E 'friend-(on|off)|member-(on|off)| start ' "$WORK/$device.out" || true
        done
        pass=1
        direct_arrived A Ana B Bea || pass=0
        direct_arrived B Bea A Ana || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    fake-observed)
        # Peers tell D false addresses of its own. Three behind one public address: two say D
        # is at 203.0.113.66, one says D's address with another port. Two more, each on an
        # address of its own but both in one network (/16), say 203.0.113.77. Three more, on
        # three networks, that connect by themselves (as anyone may, from three cheap
        # machines), say 203.0.113.88. Strangers make an address taken only from three
        # networks, and only once one D dialled itself says it too: D takes and announces only
        # its real one, which the three DHT nodes it dials see, from three networks.
        public_host D 203.0.113.50
        nat_hosts_shared 203.0.113.60 L1 L2 L3
        public_host L4 203.0.113.61
        public_host L5 203.0.113.62
        public_host L6 203.0.113.63
        public_host L7 198.51.100.63
        public_host L8 192.0.2.63
        members D
        real=/ip4/203.0.113.50/udp/41000/quic-v1
        (sleep 90) |
            in_ns D env RUST_LOG=warn,fogata_net=debug timeout 85 "$BIN" --data-dir "$WORK/data-D" \
                chat lab --nick D --port 41000 --no-mdns $BOOT > "$WORK/D.out" 2>&1 &
        PIDS+=($!)
        device=$!
        sleep 2
        liar() { # namespace, address it says
            in_ns "$1" timeout 75 "$BIN" lab-liar --dial "$real" --says "$2" > "$WORK/$1.out" 2>&1 &
            PIDS+=($!)
        }
        liar L1 /ip4/203.0.113.66/udp/41000/quic-v1
        liar L2 /ip4/203.0.113.66/udp/41000/quic-v1
        liar L3 /ip4/203.0.113.50/udp/4242/quic-v1
        liar L4 /ip4/203.0.113.77/udp/41000/quic-v1
        liar L5 /ip4/203.0.113.77/udp/41000/quic-v1
        liar L6 /ip4/203.0.113.88/udp/41000/quic-v1
        liar L7 /ip4/203.0.113.88/udp/41000/quic-v1
        liar L8 /ip4/203.0.113.88/udp/41000/quic-v1
        wait "$device" 2>/dev/null || true
        echo "=== $SCENARIO"
        for host in L1 L2 L3 L4 L5 L6 L7 L8; do
            echo "--- $host"
            grep -m2 -E 'connected to|told' "$WORK/$host.out" || true
        done
        echo "--- D"
        grep -E 'other peers see|changes the public port' "$WORK/D.out" || true
        grep 'address observed by peers' "$WORK/D.out" |
            grep -o -E 'observed=[^ ]+ peers=[0-9]+' | sort -u || true
        grep -E 'came in by themselves agree|public address confirmed' "$WORK/D.out" |
            grep -o -E '(agree|confirmed).*' | sort -u || true
        cat "$WORK/dht1.records" "$WORK/dht2.records" "$WORK/dht3.records" > "$WORK/records"
        lies='203\.0\.113\.(66|77|88)|203\.0\.113\.50/udp/4242'
        echo "--- announcements on the lab DHT: $(grep -c -F "$real" "$WORK/records") with $real," \
            "$(grep -c -E "$lies" "$WORK/records") with a false one"
        heard() { grep -q -E "address observed by peers observed=$1 peers=$2" "$WORK/D.out"; }
        pass=1
        # The lies reached D (the two liars behind one address count once; the three on
        # three networks are said once they agree, and not taken)...
        heard /ip4/203.0.113.66/udp/41000/quic-v1 2 || pass=0
        heard /ip4/203.0.113.50/udp/4242/quic-v1 1 || pass=0
        heard /ip4/203.0.113.77/udp/41000/quic-v1 2 || pass=0
        grep -q -E 'came in by themselves agree.* observed=/ip4/203\.0\.113\.88/udp/41000/quic-v1 reporters=3 networks=3' \
            "$WORK/D.out" || pass=0
        # ...yet D took and announced only its real address.
        grep -q -F "other peers see this device at $real" "$WORK/D.out" || pass=0
        grep 'other peers see this device at' "$WORK/D.out" | grep -q -v -F "$real" && pass=0
        grep -q 'changes the public port' "$WORK/D.out" && pass=0
        grep -q -F "$real" "$WORK/records" || pass=0
        grep -q -E "$lies" "$WORK/records" && pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    inbound-flood)
        # A stranger on 16 public addresses keeps 32 connections open to D from each, as
        # many as D takes from one address, 512 in all (with four peers on each address, as D
        # takes 8 from one peer), and asks D something cheap every half minute. B, a member behind a symmetric NAT, can only come in: D must still
        # take B's connection, by closing one of the stranger's, and get B's message.
        public_host D 203.0.113.50
        public_host F 203.0.113.100
        nat_host B 203.0.113.22 symmetric
        members D B
        for i in $(seq 101 115); do in_ns F ip addr add "203.0.113.$i/24" dev wan; done
        real=/ip4/203.0.113.50/udp/41000/quic-v1
        (sleep 150) |
            in_ns D env RUST_LOG=warn,fogata_net=debug timeout 160 "$BIN" --data-dir "$WORK/data-D" \
                chat lab --nick D --port 41000 --no-mdns $BOOT > "$WORK/D.out" 2>&1 &
        PIDS+=($!)
        sleep 2
        for i in $(seq 100 115); do
            for k in 1 2 3 4; do
                in_ns F timeout 155 "$BIN" lab-flood --dial "$real" --connections 8 \
                    --listen "/ip4/203.0.113.$i/udp/0/quic-v1" > "$WORK/F$i-$k.out" 2>&1 &
                PIDS+=($!)
            done
        done
        flood() { for f in "$WORK"/F*.out; do grep -o 'open [0-9]*' "$f" | tail -n 1; done |
            awk '{s += $2} END {print s + 0}'; }
        for _ in $(seq 60); do [[ $(flood) -ge 512 ]] && break; sleep 1; done
        flooded=$(flood)
        DURATION=80 SAY_AT=55
        run_device B --peer "$real"
        b_pid=$!
        wait "$b_pid" 2>/dev/null || true
        echo "=== $SCENARIO"
        echo "--- the stranger's connections open when B started: $flooded, at the end: $(flood)"
        echo "--- answers to the stranger: $(cat "$WORK"/F*.out | grep -c 'answered')"
        echo "--- B"
        grep -v -E '^\s*$' "$WORK/B.out" | tail -n 8 || true
        pass=1
        [[ $flooded -ge 512 ]] || pass=0
        grep -q "hello from B" "$WORK/D.out" || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    silent-flood)
        # Machines that open TCP connections and never say anything on them, as fast as they
        # are closed: F from 8 public addresses of one provider (one /16, another than B's)
        # to D, N from one address on E's local network to E. D and E may open 2048 files,
        # like an app that raised its limit to a few thousand. Each must keep only a bounded
        # number of those connections at once (each is closed if it is not set up within
        # five seconds, and only so many are set up at once), keep its sockets, and still
        # get its member's message over TCP: B's from the internet, from an address D never
        # reached, and M's from the local network. (One new to D in F's own /16 gets no
        # place while the flood lasts: F holds that block's share.)
        public_host D 203.0.113.50
        public_host F 203.0.113.100
        public_host B 203.0.113.22
        nat_hosts_shared 203.0.113.60 E N M
        members D B E M
        for i in $(seq 1 8); do in_ns F ip addr add "198.18.0.$i/24" dev wan; done
        lan_e=$(in_ns E ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1)
        lan_n=$(in_ns N ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1)
        quiet_device() { # name, seconds
            (sleep "$2") |
                in_ns "$1" bash -c 'ulimit -n 2048; exec "$@"' - env RUST_LOG=warn,fogata_net=debug \
                    timeout "$2" "$BIN" --data-dir "$WORK/data-$1" chat lab --nick "$1" \
                    --port 41000 --no-mdns $BOOT > "$WORK/$1.out" 2>&1 &
            PIDS+=($!)
        }
        quiet_device D 130
        d_pid=$!
        quiet_device E 130
        e_pid=$!
        sleep 2
        in_ns F timeout 125 "$BIN" lab-silent --dial 203.0.113.50:41000 --per-address 512 \
            $(for i in $(seq 1 8); do echo "--from 198.18.0.$i"; done) > "$WORK/F.out" 2>&1 &
        PIDS+=($!)
        in_ns N timeout 125 "$BIN" lab-silent --dial "$lan_e:41000" --per-address 4096 \
            --from "$lan_n" > "$WORK/N.out" 2>&1 &
        PIDS+=($!)
        # The open files of a device's process (the child of `timeout`).
        files() { local p; p=$(pgrep -P "$1" | head -n 1); ls "/proc/$p/fd" 2>/dev/null | wc -l; }
        peak_d=0 peak_e=0
        watch_files() {
            for _ in $(seq 110); do
                peak_d=$(( $(files "$d_pid") > peak_d ? $(files "$d_pid") : peak_d ))
                peak_e=$(( $(files "$e_pid") > peak_e ? $(files "$e_pid") : peak_e ))
                echo "$peak_d $peak_e" > "$WORK/peaks"
                sleep 1
            done
        }
        watch_files &
        PIDS+=($!)
        sleep 40
        DURATION=75 SAY_AT=40
        run_device B --peer /ip4/203.0.113.50/tcp/41000
        b_pid=$!
        run_device M --peer "/ip4/$lan_e/tcp/41000"
        m_pid=$!
        wait "$b_pid" "$m_pid" 2>/dev/null || true
        read -r peak_d peak_e < "$WORK/peaks"
        echo "=== $SCENARIO"
        echo "--- F (8 public addresses): $(tail -n 1 "$WORK/F.out")"
        echo "--- N (one local address): $(tail -n 1 "$WORK/N.out")"
        echo "--- most open files: D $peak_d, E $peak_e"
        pass=1
        # Only so many are taken at once: 8 being set up from each public address, 32 from
        # one block and from one local address, 192 over TCP in all. (What the flooders
        # count as open includes those the system took and the device has not looked at
        # yet.)
        [[ $peak_d -le 400 && $peak_e -le 400 ]] || pass=0
        grep -q 'Too many open files' "$WORK/D.out" "$WORK/E.out" && pass=0
        grep -q "hello from B" "$WORK/D.out" || pass=0
        grep -q "hello from M" "$WORK/E.out" || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    tcp-known)
        # Machines on 16 public addresses in four blocks (/16) open TCP connections to D
        # and never say anything on them, as fast as they are closed: as many at once as D
        # sets up over TCP for strangers, B's own block (/16) full among them. B, a member on
        # a network that blocks UDP (so D only ever saw it over TCP, at an address it cannot
        # dial back), was connected once and comes back: D must still take its connection,
        # from an address it knows, and get its message, and hold the flood to a bounded
        # number of open files. (A member D never reached gets
        # no place while the flood lasts: the strangers' share is used up.)
        public_host D 203.0.113.50
        public_host F 203.0.113.100
        public_host B 203.0.113.22
        members D B
        for block in 198.18 198.19 198.20; do
            for i in 1 2 3 4; do in_ns F ip addr add "$block.0.$i/24" dev wan; done
        done
        for i in 101 102 103; do in_ns F ip addr add "203.0.113.$i/24" dev wan; done
        (sleep 200) |
            in_ns D bash -c 'ulimit -n 2048; exec "$@"' - env RUST_LOG=warn,fogata_net=debug \
                timeout 200 "$BIN" --data-dir "$WORK/data-D" chat lab --nick D --port 41000 \
                --no-mdns $BOOT > "$WORK/D.out" 2>&1 &
        d_pid=$!
        PIDS+=($!)
        sleep 2
        # B is on a network that drops every UDP packet it sends, and meets D once.
        in_ns B nft -f - <<EOF
table inet block {
    chain output { type filter hook output priority 0; policy accept; meta l4proto udp drop; }
}
EOF
        DURATION=35 SAY_AT=12 run_device B --peer /ip4/203.0.113.50/tcp/41000
        wait "$!" 2>/dev/null || true
        heard() { grep -c '\]: hello from B$' "$WORK/D.out" || true; }
        met=$(heard)
        mv "$WORK/B.out" "$WORK/B-first.out"
        from=()
        for a in 203.0.113.100 203.0.113.101 203.0.113.102 203.0.113.103; do from+=(--from "$a"); done
        for block in 198.18 198.19 198.20; do
            for i in 1 2 3 4; do from+=(--from "$block.0.$i"); done
        done
        in_ns F timeout 150 "$BIN" lab-silent --dial 203.0.113.50:41000 --per-address 512 \
            "${from[@]}" > "$WORK/F.out" 2>&1 &
        PIDS+=($!)
        # The most files D's process (the child of `timeout`) had open while flooded.
        echo 0 > "$WORK/peak"
        (
            peak=0
            for _ in $(seq 130); do
                p=$(pgrep -P "$d_pid" | head -n 1 || true)
                n=$(ls "/proc/$p/fd" 2>/dev/null | wc -l || true)
                if (( n > peak )); then peak=$n; echo "$peak" > "$WORK/peak"; fi
                sleep 1
            done
        ) &
        PIDS+=($!)
        sleep 20
        DURATION=90 SAY_AT=50 run_device B --peer /ip4/203.0.113.50/tcp/41000
        wait "$!" 2>/dev/null || true
        flood=$(tail -n 1 "$WORK/F.out")
        peak=$(cat "$WORK/peak")
        echo "=== $SCENARIO"
        echo "--- F (16 public addresses, 4 blocks): $flood"
        echo "--- most open files: D $peak"
        echo "--- B over TCP, first: heard $met time(s); then under the flood:"
        grep -v -E '^\s*$' "$WORK/B.out" | tail -n 6 || true
        pass=1
        [[ $met == 1 ]] || pass=0
        [[ $(heard) == 2 ]] || pass=0
        # The flood was held to a bounded number of D's files (what the flooder counts as
        # open also holds those the system took and D has not looked at yet, which bursts
        # when D is busy), and kept being closed and opened again.
        closed=$(echo "$flood" | sed -n 's/.*closed \([0-9]*\).*/\1/p')
        [[ $peak -gt 0 && $peak -le 400 && ${closed:-0} -ge 1000 ]] || pass=0
        grep -q 'Too many open files' "$WORK/D.out" && pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    mdns-stale)
        # A and B, members on one local network, mDNS only, start together; S, a device of
        # another group there, started earlier and answers every query at once. What A and B
        # answer each other in their first seconds is lost (a busy Wi-Fi, a copy not
        # listening yet). They must still find each other within seconds, not at the next
        # query, half a minute later.
        nat_hosts_shared 203.0.113.60 A B S
        members A B
        "$BIN" --data-dir "$WORK/data-S" group create other > /dev/null
        lan() { in_ns "$1" ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1; }
        lan_a=$(lan A) lan_b=$(lan B)
        (sleep 70) | in_ns S env RUST_LOG=warn,libp2p_mdns=debug timeout 70 "$BIN" --data-dir "$WORK/data-S" chat other --nick S \
            --no-dht > "$WORK/S.out" 2>&1 &
        PIDS+=($!)
        sleep 3
        for pair in "A $lan_b" "B $lan_a"; do
            read -r host from <<< "$pair"
            in_ns "$host" nft -f - <<EOF2
table inet lost {
    chain input { type filter hook input priority 0; policy accept; ip saddr $from meta l4proto udp drop; }
}
EOF2
        done
        started=$(date -u +%s)
        lan_device() { # name
            (sleep 40; echo "hello from $1"; sleep 15) |
                in_ns "$1" env RUST_LOG=warn,libp2p_mdns=debug,fogata_net=debug timeout 60 "$BIN" --data-dir "$WORK/data-$1" chat lab --nick "$1" \
                    --no-dht > "$WORK/$1.out" 2>&1 &
            PIDS+=($!)
        }
        lan_device A
        lan_device B
        sleep 3
        in_ns A nft delete table inet lost
        in_ns B nft delete table inet lost
        sleep 60
        echo "=== $SCENARIO"
        pass=1
        for pair in "A B" "B A"; do
            read -r device other <<< "$pair"
            first=$(grep -m1 -E "^[0-9:]{8} \+ $other \[[^]]*\] connected" "$WORK/$device.out" |
                awk '{print $1}' || true)
            after=never
            [[ -n $first ]] && after=$(( $(date -u -d "$first" +%s) - started ))
            echo "--- $device: $other connected after $after s"
            grep -q "hello from $other" "$WORK/$device.out" || pass=0
            # The answers come again once the first 3 s are over: the next query must not
            # wait for the 30 s interval.
            [[ $after != never ]] && (( after <= 15 )) || pass=0
        done
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    dht-liars)
        # Six DHT nodes on the way to every key answer each lookup first, each with eight
        # members made up for that answer, at an address that answers only after seconds (S,
        # behind a link that holds every packet 12 s, past the 10 s any dial may take): dials to them hold every place a lookup
        # has for new members while they last. The lab DHT nodes, which keep the real
        # announcements, answer 1.5 s later. A and B, members on public addresses that only
        # the DHT tells of, must still find each other at the lookups that name them, not
        # once the made-up members' dials are over.
        public_host S 203.0.113.90
        start_dht S 4001
        in_ns S tc qdisc add dev wan root netem delay 12000ms
        slow=/ip4/203.0.113.90/udp/4001/quic-v1
        liars=""
        for i in 1 2 3 4 5 6; do
            public_host "L$i" "203.0.113.8$i"
            start_dht "L$i" 4001 --peer "$DHT1" --lies 8 --lie-at "$slow"
            liars="$liars --bootstrap $DHT_ADDR"
        done
        in_ns dht1 tc qdisc add dev wan root netem delay 1500ms
        in_ns dht2 tc qdisc add dev wan root netem delay 1500ms
        in_ns dht3 tc qdisc add dev wan root netem delay 1500ms
        BOOT="$liars $BOOT"
        public_host A 203.0.113.21
        public_host B 203.0.113.22
        members A B
        DURATION=90 SAY_AT=60
        started=$(date -u +%s)
        RUST_LOG=warn,fogata_net=debug run_device A --port 41000
        RUST_LOG=warn,fogata_net=debug run_device B --port 41000
        sleep $((DURATION + 2))
        echo "=== $SCENARIO"
        pass=1
        for pair in "A B" "B A"; do
            read -r device other <<< "$pair"
            taken=$(grep -c 'found a member announcement' "$WORK/$device.out" || true)
            left=$(grep -c 'left out of a lookup' "$WORK/$device.out" || true)
            first=$(grep -m1 -E "^[0-9:]{8} \+ $other \[[^]]*\] connected" "$WORK/$device.out" |
                awk '{print $1}' || true)
            after=never
            [[ -n $first ]] && after=$(( $(date -u -d "$first" +%s) - started ))
            echo "--- $device: $taken announcements taken, $left left out, $other connected" \
                "after $after s"
            grep -q "hello from $other" "$WORK/$device.out" || pass=0
            # Both announce within some 15 s and look again at the next slot (every 10 s),
            # whose answers name the other: the made-up members' dials, which hold a
            # lookup's places for 10 s, must not keep them apart until the next lookup,
            # half a minute later.
            [[ $after != never ]] && (( after <= 30 )) || pass=0
        done
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    tcp-slow)
        # Members on networks that block UDP, so that only TCP goes, over links that hold
        # every packet SLOW_DELAY (800 ms when not set; netem arguments may follow, such as
        # "1000ms loss 10%") each way, as a satellite or a crowded
        # mobile network does: setting a TCP connection up takes several such round trips.
        # B dials A on their local network, at an address A never reached it at; M dials D on
        # the internet, from an address D never reached it at. Neither is flooded, so the
        # time strangers get to set a connection up must not keep them out.
        nat_hosts_shared 203.0.113.60 A B
        public_host D 203.0.113.50
        public_host M 203.0.113.22
        members A B D M
        delay=${SLOW_DELAY:-800ms}
        for host in A B D M; do
            dev=wan
            [[ $host == A || $host == B ]] && dev=eth0
            # shellcheck disable=SC2086
            in_ns "$host" tc qdisc add dev "$dev" root netem delay $delay
            in_ns "$host" nft -f - <<EOF2
table inet block {
    chain output { type filter hook output priority 0; policy accept; meta l4proto udp drop; }
}
EOF2
        done
        lan_a=$(in_ns A ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1)
        DURATION=90 SAY_AT=60
        run_device A --port 41000 --no-dht
        run_device D --port 41000 --no-dht
        sleep 2
        run_device B --peer "/ip4/$lan_a/tcp/41000" --no-dht
        run_device M --peer /ip4/203.0.113.50/tcp/41000 --no-dht
        sleep $((DURATION + 2))
        echo "=== $SCENARIO (each way: $delay)"
        pass=1
        for pair in "A B" "B A" "D M" "M D"; do
            read -r device other <<< "$pair"
            if grep -q "hello from $other" "$WORK/$device.out"; then
                echo "--- $device received the message from $other"
            else
                echo "--- $device: NO message from $other"
                pass=0
            fi
        done
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    tcp-slow-trials)
        # A measurement, not run by default: M, a member on a network that blocks UDP, dials
        # D over TCP TRIALS times (20 when not set), each a new copy that gives up after 15 s,
        # over links as SLOW_DELAY shapes them (800 ms and 10% loss when not set). Prints how
        # many connections each side set up and how many D gave up on while setting them up.
        public_host D 203.0.113.50
        public_host M 203.0.113.22
        members D M
        for host in D M; do
            # shellcheck disable=SC2086
            in_ns "$host" tc qdisc add dev wan root netem delay ${SLOW_DELAY:-800ms loss 10%}
        done
        in_ns M nft -f - <<EOF2
table inet block {
    chain output { type filter hook output priority 0; policy accept; meta l4proto udp drop; }
}
EOF2
        trials=${TRIALS:-20}
        (sleep $((trials * 16 + 10))) |
            in_ns D env RUST_LOG=warn,libp2p_swarm=debug timeout $((trials * 16 + 10)) "$BIN" \
                --data-dir "$WORK/data-D" chat lab --nick D --port 41000 --no-mdns --no-dht \
                > "$WORK/D.out" 2>&1 &
        PIDS+=($!)
        d_pid=$!
        sleep 2
        for i in $(seq "$trials"); do
            (sleep 15) | in_ns M env RUST_LOG=warn,libp2p_swarm=debug timeout 15 "$BIN" \
                --data-dir "$WORK/data-M" chat lab --nick M --no-mdns --no-dht \
                --peer /ip4/203.0.113.50/tcp/41000 > "$WORK/M-$i.out" 2>&1 || true
        done
        kill "$d_pid" 2>/dev/null || true
        wait "$d_pid" 2>/dev/null || true
        met=$(grep -l 'Connection established' "$WORK"/M-*.out | wc -l)
        echo "=== $SCENARIO (each way: ${SLOW_DELAY:-800ms loss 10%})"
        echo "--- M: connected in $met of $trials trials"
        echo "--- D: $(grep -c 'Connection established' "$WORK/D.out" || true) connections set up," \
            "$(grep -c 'Incoming connection failed.*Timeout' "$WORK/D.out" || true) given up on" \
            "while being set up"
        echo "logs: $WORK"
        exit 0 ;;
    mdns-flood)
        # A and B, members on one local network, find each other over mDNS only (no DHT).
        # N, a neighbour there, answers each of their queries with 4 responses of 20 made-up
        # peers that never expire, and sends 50 more responses a second that answer no
        # query. A must take only a bounded number of them, stay light, and still talk with
        # B.
        nat_hosts_shared 203.0.113.60 A B N
        members A B
        lan_n=$(in_ns N ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1)
        in_ns N timeout 100 "$BIN" lab-mdns-flood --interface "$lan_n" > "$WORK/N.out" 2>&1 &
        PIDS+=($!)
        sleep 1
        lan_device() { # name
            (sleep 30; echo "hello from $1"; sleep 55) |
                in_ns "$1" env RUST_LOG=warn,fogata_net=debug,libp2p_mdns=info timeout 90 "$BIN" \
                    --data-dir "$WORK/data-$1" chat lab --nick "$1" --no-dht > "$WORK/$1.out" 2>&1 &
            PIDS+=($!)
        }
        lan_device A
        a_pid=$!
        lan_device B
        b_pid=$!
        # Memory and processor time of A's process (the child of `timeout`).
        sleep 5
        a=$(pgrep -P "$a_pid" | head -n 1)
        peak=0 cpu=0
        while kill -0 "$a" 2>/dev/null; do
            rss=$(awk '/^VmRSS/ {print $2}' "/proc/$a/status" 2>/dev/null || echo 0)
            (( ${rss:-0} > peak )) && peak=$rss
            cpu=$(awk '{print $14 + $15}' "/proc/$a/stat" 2>/dev/null || echo "$cpu")
            sleep 1
        done
        wait "$b_pid" 2>/dev/null || true
        found=$(grep -c 'discovered peer on address' "$WORK/A.out" || true)
        skipped=$(grep -c 'too many new peers on the local network' "$WORK/A.out" || true)
        echo "=== $SCENARIO"
        echo "--- N: $(tail -n 1 "$WORK/N.out")"
        echo "--- A: $found addresses found over mDNS, $skipped new peers not dialled," \
            "most memory $((peak / 1024)) MiB, processor time $((cpu / 100)) s"
        pass=1
        grep -q "hello from B" "$WORK/A.out" || pass=0
        grep -q "hello from A" "$WORK/B.out" || pass=0
        [[ $found -le 2000 ]] || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    lan-sealed)
        # A and B, each behind its own router with a local address, are friends and members
        # of the lab group. What they announce on the lab DHT holds no local address in
        # clear, only sealed names: B opens A's back into A's local address, with A's friend
        # secret and with the group's; S, with a profile and a group of its own, opens none.
        # Neither lists a Fogata protocol to the DHT nodes in identify.
        nat_host A 203.0.113.21 cone
        nat_host B 203.0.113.22 cone
        "$BIN" --data-dir "$WORK/data-A" profile --name Ana > /dev/null
        "$BIN" --data-dir "$WORK/data-B" profile --name Bea > /dev/null
        "$BIN" --data-dir "$WORK/data-S" profile --name Sofia > /dev/null
        "$BIN" --data-dir "$WORK/data-S" group create other > /dev/null
        in_ns A timeout 200 "$BIN" --data-dir "$WORK/data-A" friends invite --accept-first \
            --no-mdns $BOOT > "$WORK/A-invite.out" 2>&1 &
        PIDS+=($!)
        for _ in $(seq 300); do
            grep -q 'fogata:friend:' "$WORK/A-invite.out" && break
            sleep 0.1
        done
        link=$(grep -o -m1 'fogata:friend:[A-Za-z0-9_-]*' "$WORK/A-invite.out")
        in_ns B timeout 200 "$BIN" --data-dir "$WORK/data-B" friends add "$link" --no-mdns $BOOT \
            > "$WORK/B-add.out" 2>&1 || true
        wait "${PIDS[-1]}" 2>/dev/null || true
        members A B
        DURATION=90 SAY_AT=60
        run_device A
        run_device B
        sleep $((DURATION + 2))
        cat "$WORK/dht1.records" "$WORK/dht2.records" "$WORK/dht3.records" > "$WORK/records"
        local_a=$(in_ns A ip -4 -o addr show dev eth0 | awk '{print $4}' | cut -d/ -f1)
        "$BIN" --data-dir "$WORK/data-B" lab-open "$WORK/records" > "$WORK/B-open.out"
        "$BIN" --data-dir "$WORK/data-S" lab-open "$WORK/records" > "$WORK/S-open.out"
        private=$(grep -o -E -e '/ip4/(10|127|192\.168|169\.254|172\.(1[6-9]|2[0-9]|3[01])|100\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7]))\.[0-9.]+' \
            -e '/ip6/(f[cd][0-9a-f]{0,2}|fe[89ab][0-9a-f]):[0-9a-f:]*' "$WORK/records" |
            sort -u | tr '\n' ' ' || true)
        echo "=== $SCENARIO"
        grep -h 'friends now' "$WORK/B-add.out" || true
        echo "announcements: $(wc -l < "$WORK/records"), from A: $(grep -c 203.0.113.21 "$WORK/records")," \
            "from B: $(grep -c 203.0.113.22 "$WORK/records")"
        echo "sealed names: $(grep -o -E '/dns4/[0-9a-f]+\.fogata' "$WORK/records" | sort -u | wc -l)"
        echo "local addresses in clear: ${private:-none}"
        # What the devices list in identify to the DHT nodes (the lab's own list kad, which
        # Fogata devices, DHT clients, never do).
        cat "$WORK/dht1.identified" "$WORK/dht2.identified" "$WORK/dht3.identified" |
            grep -v -F /ipfs/kad/ \
            > "$WORK/identified" || true
        told=$(grep -o -E '/fogata/[^ ]*' "$WORK/identified" | sort -u | tr '\n' ' ' || true)
        echo "devices identified to the DHT nodes: $(wc -l < "$WORK/identified")," \
            "Fogata protocols listed to them: ${told:-none}"
        echo "--- B opens (A is at $local_a)"
        cat "$WORK/B-open.out"
        echo "--- S opens"
        cat "$WORK/S-open.out"
        pass=1
        grep -q 203.0.113.21 "$WORK/records" && grep -q 203.0.113.22 "$WORK/records" || pass=0
        [[ -z $private ]] || pass=0
        [[ -s $WORK/identified && -z $told ]] || pass=0
        grep -q -E "^opened with friend Ana: .* -> /ip4/$local_a/" "$WORK/B-open.out" || pass=0
        grep -q -E "^opened with group lab epoch [0-9]+: .* -> /ip4/$local_a/" "$WORK/B-open.out" || pass=0
        grep -q 'opened: 0,' "$WORK/S-open.out" || pass=0
        if [[ $pass == 1 ]]; then
            echo "result: PASS   logs: $WORK"
            exit 0
        fi
        echo "result: FAIL   logs: $WORK"
        exit 1 ;;
    probe-direct|probe-member)
        PROBE_MODE=${PROBE_MODE:-single}
        PROBE_PACKETS=${PROBE_PACKETS:-1500}
        if [[ $SCENARIO == probe-direct ]]; then
            nat_host A 203.0.113.21 cone
            nat_host B 203.0.113.22 cone
        else
            nat_host A 203.0.113.21 symmetric
            nat_host B 203.0.113.22 symmetric
            public_host C 203.0.113.50
            members C
            run_device C
        fi
        impair r-A
        impair r-B
        members A B
        run_probe B A
        wait "${PIDS[-1]}" "${PIDS[-2]}" 2>/dev/null || true
        echo "=== $SCENARIO mode=$PROBE_MODE netem='${NETEM:-none}'"
        grep -E 'member .* connected|path now|RESULT' "$WORK/B.out" "$WORK/A.out" | sed "s#$WORK/##"
        grep -q RESULT "$WORK/B.out"
        exit ;;
    *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;;
esac

EXPECT_NO_CONTACT=${EXPECT_NO_CONTACT:-0}
members "${DEVICES[@]}"
for device in "${DEVICES[@]}"; do
    extra="EXTRA_$device"
    # shellcheck disable=SC2086
    run_device "$device" ${!extra:-}
done
if [[ -n "${MOVE:-}" ]]; then
    # Another public address: masquerading drops the mappings of the old one.
    sleep "$MOVE_AT"
    date -u +%T > "$WORK/moved-at"
    in_ns "r-$MOVE" ip addr flush dev wan
    in_ns "r-$MOVE" ip addr add 203.0.113.42/24 dev wan
    in_ns "r-$MOVE" ip route replace default dev wan
    sleep $((DURATION - MOVE_AT + 2))
elif [[ -n "${CUT:-}" ]]; then
    # Drops everything between two routers on the internet's bridge, from CUT_AT to CUT_UNTIL.
    read -r cut1 cut2 <<< "$CUT"
    sleep "$CUT_AT"
    date -u +%T > "$WORK/cut-at"
    in_ns net nft -f - <<EOF
table bridge cut {
    chain between {
        type filter hook forward priority 0;
        ip saddr ${cut1#*=} ip daddr ${cut2#*=} drop
        ip saddr ${cut2#*=} ip daddr ${cut1#*=} drop
    }
}
EOF
    sleep $((CUT_UNTIL - CUT_AT))
    date -u +%T > "$WORK/cut-until"
    in_ns net nft delete table bridge cut
    sleep $((DURATION - CUT_UNTIL + 2))
else
    sleep $((DURATION + 2))
fi

echo "=== $SCENARIO"
result=0
for device in "${DEVICES[@]}"; do
    echo "--- $device"
    grep -E '^([0-9:]{8} )?([+-] |· (other peers|this network|hole punch|helping|reachable))' "$WORK/$device.out" | sort -u || true
    for other in "${DEVICES[@]}"; do
        [[ $other == "$device" ]] && continue
        if grep -q "hello from $other" "$WORK/$device.out"; then
            echo "    received the message from $other"
            [[ $EXPECT_NO_CONTACT == 1 ]] && result=1
        else
            echo "    NO message from $other"
            [[ $EXPECT_NO_CONTACT == 1 ]] || result=1
        fi
    done
done
[[ $EXPECT_NO_CONTACT == 1 ]] && echo "(expected: no contact without a member in between)"
if [[ -n "${MOVE:-}" ]]; then
    # How long the others took to have the device that moved again.
    moved=$(date -u -d "$(cat "$WORK/moved-at")" +%s)
    for device in "${DEVICES[@]}"; do
        [[ $device == "$MOVE" ]] && continue
        # The first connection after its last disconnection since it moved: the old link
        # can still report a connection of its own just before it goes.
        back=never
        while read -r t kind; do
            s=$(date -u -d "$t" +%s)
            (( s > moved )) || continue
            if [[ $kind == "-" ]]; then back=never; elif [[ $back == never ]]; then back=$((s - moved)); fi
        done < <(grep -E "^[0-9:]{8} [-+] $MOVE \[[^]]*\] (connected|disconnected)" "$WORK/$device.out" |
            awk '{print $1, $2}' || true)
        echo "    $device had $MOVE again $back s after it moved"
    done
fi
if [[ -n "${CUT:-}" ]]; then
    # How each of the two had the other before the cut, how long it took to have it again
    # through another member, and what way they had at the end.
    cut_at=$(date -u -d "$(cat "$WORK/cut-at")" +%s)
    cut_until=$(date -u -d "$(cat "$WORK/cut-until")" +%s)
    names=("${cut1%%=*}" "${cut2%%=*}")
    for device in "${names[@]}"; do
        other=${names[0]}
        [[ $other == "$device" ]] && other=${names[1]}
        before=none back=never held=no last=none
        while read -r t sign rest; do
            s=$(date -u -d "$t" +%s)
            path=${rest#*connected (}
            path=${path%)}
            if (( s < cut_at )); then
                [[ $sign == + ]] && before=$path || before=none
                continue
            fi
            if (( s <= cut_until )); then
                if [[ $sign == + && $path == through* && $back == never ]]; then
                    back=$((s - cut_at))
                fi
                [[ $sign == + && $back != never ]] && held=yes || held=no
            fi
            [[ $sign == + ]] && last=$path || last=none
        done < <(grep -E "^[0-9:]{8} [-+] $other \[[^]]*\] (connected|disconnected)" \
            "$WORK/$device.out" || true)
        echo "    $device: $other $before before the cut; through a member $back s after it;" \
            "at the end: $last"
        if [[ $before == none || $before == through* ]]; then
            echo "    ($device did not have $other directly before the cut)"
            result=1
        fi
        if [[ $back == never ]] || (( back > CUT_WITHIN )) || [[ $held != yes ]]; then
            result=1
        fi
    done
    echo "(expected: back through a member within $CUT_WITHIN s, and kept until the cut ends)"
fi
echo "result: $([[ $result == 0 ]] && echo PASS || echo FAIL)   logs: $WORK"
exit $result
