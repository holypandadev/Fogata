Part of [Fogata architecture](../architecture.md).

## 9. The first milestone (done): the CLI prototype

Kept as the record of where Fogata started; the command line of today is in docs/cli.md.
Two machines on different networks find each other using only the group secret and exchange
encrypted text messages.

```
fogata id                         Create or show this device's identity.
fogata group create <name>        Create a group (this device is first on its ladder).
fogata group invite <name>        Print a single-use invite (managers).
fogata group join                 Join through a member that is online, with the invite typed
                                  when asked (an argument shows in the list of processes).
fogata group members|remove|move-up|move-down|managers|add|rename|leave
fogata group list                 List the groups of this device.
fogata chat <group> [--nick N]    Connect to the group's members and chat on stdin/stdout.
```

The CLI prints how each member was reached (LAN, direct, hole-punched, through a member, which
it names when that member is in the same group).

Done means: the netlab scenarios pass (direct, scheduled punch, member coordinator, member
bridge), and two machines on separate networks (home fibre and mobile data) exchange
messages, with the connection type shown.
