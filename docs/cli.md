# The command-line program

The `fogata` program does what the app does for text: friends, groups, rooms and files.
Build it as [building.md](building.md) says.

## Chatting in a group

On the first device:

```
fogata group create friends
fogata group invite friends
```

The invite starts with `fogata:` and works once, for one person, for 7 days (`--days`). It
does not contain the group key. On the second device, while an admin of the group is online:

```
fogata group join --nick luis             # then type the invite when asked
```

Then, on both devices:

```
fogata chat friends
```

This enters the group's room, starting it if nobody is in one. Meanwhile every other group
and friend of this device is online too, as in the app. Type a message and press
Enter. Who arrives later gets the room's chat so far; when the last person leaves, the room
and everything said in it are gone. `/room` shows who is in it, `/members` the members and
how each is connected, `/invite` prints an invite with this device's current addresses,
`/quit` (or Ctrl+C) leaves. `fogata chat friends --watch` stays online in the group without
entering the room (`/join` and `/leave` switch). Finding each other through the public DHT
takes up to a minute.

Files: `/share <path>` offers a file to the room while you are in it, `/files` lists what is
shared, `/get <n>` downloads one (into `--downloads <dir>`, the current folder by default)
and checks it; after that you share it too. A file stays available while someone in the
room has it.

## Friends

```
fogata profile --name Ana                 # your name; --avatar 0-4 or --photo pic.png
fogata friends invite                     # a link and QR code for one person, 10 minutes
fogata friends invite --code              # or a short code to read out: FGT-XXXX-XXXX
fogata friends add                        # on the other device: type the link or code; waits
```

Each side prints its own fingerprint; before accepting, check that the other person's device
shows the same one. Then:

```
fogata friends                            # friends and requests
fogata friends accept|decline|remove <name or fingerprint>
fogata friends online                     # see who is online, answer requests
fogata group add friends --friend Bea     # admins; Bea gets the group when you are both online
```

`fogata chat` also shows friends coming online and entering rooms ("is in a room", with the
group's name only if you are in that group too), and takes `/friends`, `/accept <who>` and
`/decline <who>`. Names default to the profile name.

## Managing a group

Changes reach the others the next time this device connects:

```
fogata group members friends                  # who is in, who is admin
fogata group remove friends <short key>       # admins; makes a new group key
fogata group admin friends <short key> on     # creator only
fogata group add friends <device key>         # admins; key from `fogata id` on that device
fogata group add friends --friend <name>      # admins; a friend, by name or fingerprint
fogata group rename friends "new name"        # admins
fogata group leave friends
```

`fogata network` checks the network this device is on (router kind, UDP, IPv6) and says
what it means for connecting to friends.

Useful options: `--no-mdns` to ignore the local network, `--no-dht` for local-only tests,
`--peer <address>` to dial a member directly, `--no-bridge` to help others connect without
carrying their traffic, `-v` for network details. `--data-dir` keeps separate identities for
several instances on one machine.
