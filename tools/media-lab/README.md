# Media bridge lab

A WebRTC call between two Fogata media bridges on one machine. Each side of the call may
only use its own bridge's local TURN endpoint (relay-only ICE), so every media packet crosses
the Fogata connection between the two bridges as a QUIC datagram.

```
B=target/release/fogata
PASS=$(head -c 12 /dev/urandom | base64 | tr '+/' '-_')
$B --data-dir /tmp/fa group create call                # prints an invite
$B --data-dir /tmp/fb group join 'fogata:...'
$B --data-dir /tmp/fb media-bridge call --turn-port 3479 --port 47901 --no-dht --no-mdns \
    --turn-user lab --turn-pass "$PASS"
$B --data-dir /tmp/fa media-bridge call --turn-port 3478 --no-dht --no-mdns \
    --turn-user lab --turn-pass "$PASS" --peer /ip4/127.0.0.1/udp/47901/quic-v1
python3 -m http.server 8790 --bind 127.0.0.1 --directory tools/media-lab
```

Each bridge prints, as the other connects, the TURN user its engine's connection for that
member takes (`lab.` and the member's device key in hex). Open
`http://127.0.0.1:8790/call.html?a=3478&b=3479&ua=$UA&ub=$UB&pass=$PASS`, with `$UA` the user
the bridge on 3478 printed and `$UB` the one the bridge on 3479 printed, in a
browser and press "Start call". Without `--turn-user` and `--turn-pass` each bridge makes up
its own and prints them. The page shows the receiver's WebRTC statistics; `window.result` holds them.

To imitate a bad network, start both bridges (a `fogata` built with `--features lab`) with
`--lab-loss 0.02 --lab-delay-ms 30 --lab-jitter-ms 5` (test only: it degrades the media
datagrams each bridge sends).
