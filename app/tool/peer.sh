#!/bin/bash
# The other device for integration_test/with_friend_test.dart: a command-line Fogata called
# Marta that shows a friend link, accepts the request, waits to be added to a group, then
# enters its room, says hello, shares a file and leaves.
#
#   tool/peer.sh <fogata binary> <empty data folder> <output folder>
set -u
fogata="${1:?fogata binary}"
data="${2:?data folder}"
out="${3:?output folder}"
mkdir -p "$out"
head -c 3000000 /dev/urandom > "$out/test-photo.bin"

"$fogata" --data-dir "$data" profile --name Marta > "$out/profile.out" 2>&1
# Answers "y" to the request when it comes.
(while sleep 2; do echo y; done) | "$fogata" --data-dir "$data" friends invite > "$out/invite.out" 2> "$out/invite.err"

"$fogata" --data-dir "$data" friends online < <(sleep 300; echo /quit) > "$out/online.out" 2>&1 &
online=$!
for _ in $(seq 100); do
  if "$fogata" --data-dir "$data" group list 2>/dev/null | grep -q "The usual crew"; then break; fi
  sleep 3
done
kill "$online" 2>/dev/null
wait "$online" 2>/dev/null
sleep 2
# The group arrived as an invitation from a friend: join it.
"$fogata" --data-dir "$data" group accept "The usual crew" > "$out/accept.out" 2>&1
# The group reaches Marta as soon as it is made: she waits so that the app is in the room
# before her and plays the sound of her coming in.
sleep 10

{
  sleep 20
  echo "hi from Marta"
  sleep 2
  echo "/share $out/test-photo.bin"
  sleep 60
  echo "heading off now"
  sleep 3
  echo /quit
} | "$fogata" --data-dir "$data" chat "The usual crew" --downloads "$out" > "$out/chat.out" 2> "$out/chat.err"
echo done > "$out/finished"
