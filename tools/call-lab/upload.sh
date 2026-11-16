#!/usr/bin/env bash
# A home connection's upload for one copy of the call lab (measure.sh's LAB_UPLOAD), on the
# computer's loopback, where every copy's packets go. Run as root (sudo) from outside the
# lab's sessions, which share the computer's network.
#
# Only what that copy sends from its own ports goes through the upload: its QUIC
# datagrams and streams (the media of every connection it has, their control and its voice)
# leave from the port it listens on, as from a house to the internet. Everything else on the
# loopback is left alone: what the other copies send it (their feedback, their voices), and
# each copy's WebRTC engine talking to its own TURN endpoint, which never leaves a computer
# but crosses the loopback too. Limiting the whole loopback instead counts a packet sent
# from one copy to another three times (to the sender's TURN endpoint, from it to the other
# copy, and from that copy's endpoint to its engine), and every copy's feedback with it.
#
# The upload is a rate (HTB) with a tail-drop queue behind it as long as the rate sends in
# <queue> ms, as in a home router or a cable or DSL modem without any queue management:
# packets wait there up to that long and are dropped when it is full. HTB's burst is kept
# to a few packets, as a link of that rate sends nothing faster. With <loss> (in percent)
# the queue is netem's instead, which also loses that share of what goes up at random;
# with <delay> (in ms) what comes back to the copy waits that long, so the round trip is
# about that plus what waits in the queue.
#
# Usage: upload.sh on <ports> <kbit/s> [queue ms, default 100] [loss %] [delay ms]
#        upload.sh off
#        upload.sh show     (what went up and what was dropped; needs no root)
# <ports>: the copy's own ports, separated by commas (lab.sh's lab_upload_ports).
set -euo pipefail
DEV=lo

case "${1:-}" in
  on)
    [ $# -ge 3 ] || { echo "usage: $0 on <ports> <kbit/s> [queue ms] [loss %] [delay ms]" >&2; exit 2; }
    ports=$2 kbit=$3 queue=${4:-100} loss=${5:-0} delay=${6:-0}
    [[ $ports =~ ^[0-9]+(,[0-9]+)*$ && $kbit =~ ^[0-9]+$ && $queue =~ ^[0-9]+$ &&
      $loss =~ ^[0-9]+(\.[0-9]+)?$ && $delay =~ ^[0-9]+$ ]] || { echo "upload.sh: bad arguments" >&2; exit 2; }
    # What the rate sends in <queue> ms, and at least three of the largest batches the copy
    # hands the kernel at once (QUIC's segmentation offload: up to 10 datagrams).
    bytes=$((kbit * 1000 / 8 * queue / 1000))
    [ "$bytes" -lt 48000 ] && bytes=48000
    tc qdisc del dev "$DEV" root 2> /dev/null || true
    tc qdisc add dev "$DEV" root handle 1: htb default 30
    # Everything else, as fast as the loopback goes.
    tc class add dev "$DEV" parent 1: classid 1:30 htb rate 10gbit quantum 65536
    tc qdisc add dev "$DEV" parent 1:30 handle 30: pfifo limit 10000
    tc class add dev "$DEV" parent 1: classid 1:10 htb rate "${kbit}kbit" ceil "${kbit}kbit" \
      burst 4500 cburst 4500 quantum 1514
    if [ "$loss" = 0 ]; then
      tc qdisc add dev "$DEV" parent 1:10 handle 10: bfifo limit "$bytes"
    else
      tc qdisc add dev "$DEV" parent 1:10 handle 10: netem loss "$loss%" limit $((bytes / 1200))
    fi
    if [ "$delay" != 0 ]; then
      tc class add dev "$DEV" parent 1: classid 1:20 htb rate 10gbit quantum 65536
      tc qdisc add dev "$DEV" parent 1:20 handle 20: netem delay "${delay}ms" limit 10000
    fi
    for port in ${ports//,/ }; do
      for proto in 6 17; do
        tc filter add dev "$DEV" parent 1: protocol ip prio 1 u32 \
          match ip protocol "$proto" 0xff match ip sport "$port" 0xffff flowid 1:10
        tc filter add dev "$DEV" parent 1: protocol ipv6 prio 2 u32 \
          match ip6 protocol "$proto" 0xff match ip6 sport "$port" 0xffff flowid 1:10
        if [ "$delay" != 0 ]; then
          tc filter add dev "$DEV" parent 1: protocol ip prio 3 u32 \
            match ip protocol "$proto" 0xff match ip dport "$port" 0xffff flowid 1:20
          tc filter add dev "$DEV" parent 1: protocol ipv6 prio 4 u32 \
            match ip6 protocol "$proto" 0xff match ip6 dport "$port" 0xffff flowid 1:20
        fi
      done
    done
    echo "$(date +%s) upload of ports $ports: $kbit kbit/s, queue $queue ms ($bytes bytes), loss $loss %, delay back $delay ms"
    ;;
  off)
    tc qdisc del dev "$DEV" root 2> /dev/null || true
    ;;
  show)
    # One line: the time, the upload's bytes and packets sent and dropped, and its queue's
    # backlog in bytes; then the loopback's own bytes sent.
    tc -s class show dev "$DEV" classid 1:10 2> /dev/null | awk -v t="$(date +%s)" '
      /Sent/ { sent = $2; pkts = $4; dropped = $7; sub(",", "", dropped) }
      /backlog/ { backlog = $2 }
      END { printf "%s up sent=%s pkts=%s dropped=%s backlog=%s", t, sent, pkts, dropped, backlog }'
    echo " lo=$(cat /sys/class/net/$DEV/statistics/tx_bytes)"
    ;;
  *)
    echo "usage: $0 on <ports> <kbit/s> [queue ms] [loss %] [delay ms] | off | show" >&2
    exit 2
    ;;
esac
