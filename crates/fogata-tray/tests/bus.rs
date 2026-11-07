//! The tray icon on a bus of its own: a `dbus-daemon` started for each test, with a stand-in
//! for the desktop's watcher, and calls as a host makes them. Skipped where there is no
//! `dbus-daemon`.

#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use fogata_tray::{Event, ITEM_PATH, Item, MENU_PATH, Pixmap, Spec, Tray, WATCHER};
use zbus::{
    connection, fdo, interface,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

/// A message bus that only these tests use, in a folder of its own. It starts no services:
/// its configuration names no place to find them.
struct Bus {
    daemon: Child,
    dir: PathBuf,
    address: String,
}

impl Bus {
    fn start() -> Option<Bus> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fogata-tray-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let config = dir.join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}/bus</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                dir.display()
            ),
        )
        .ok()?;
        let daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .env("XDG_RUNTIME_DIR", &dir)
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut daemon) = daemon else {
            eprintln!("no dbus-daemon here: skipped");
            let _ = std::fs::remove_dir_all(&dir);
            return None;
        };
        let mut address = String::new();
        let stdout = daemon.stdout.take()?;
        BufReader::new(stdout).read_line(&mut address).ok()?;
        Some(Bus {
            daemon,
            dir,
            address: address.trim().to_owned(),
        })
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The desktop's watcher, as far as an item sees it: it takes registrations and says
/// whether a host shows them.
struct StandInWatcher {
    host: bool,
    registered: Arc<Mutex<Vec<String>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StandInWatcher {
    fn register_status_notifier_item(&self, service: String, #[zbus(header)] header: Header<'_>) {
        // A path alone stands for the sender's own name.
        let service = if service.starts_with('/') {
            header.sender().map(|s| s.to_string()).unwrap_or_default()
        } else {
            service
        };
        self.registered.lock().unwrap().push(service);
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        self.host
    }
}

async fn watcher(bus: &Bus, host: bool) -> (zbus::Connection, Arc<Mutex<Vec<String>>>) {
    let registered = Arc::new(Mutex::new(Vec::new()));
    let connection = connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(WATCHER)
        .unwrap()
        .serve_at(
            "/StatusNotifierWatcher",
            StandInWatcher {
                host,
                registered: registered.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    (connection, registered)
}

fn spec() -> Spec {
    Spec {
        id: "fogata".into(),
        title: "Fogata".into(),
        icon: vec![Pixmap {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 40, 50, 60, 70, 80],
        }],
        items: vec![
            Item::Action {
                key: "open".into(),
                label: "Open Fogata".into(),
            },
            Item::Separator,
            Item::Action {
                key: "quit".into(),
                label: "Quit Fogata".into(),
            },
        ],
    }
}

async fn start(bus: &Bus) -> (Option<Tray>, Arc<Mutex<Vec<Event>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let tray = fogata_tray::start_at(&bus.address, spec(), move |e| seen.lock().unwrap().push(e))
        .await
        .unwrap();
    (tray, events)
}

async fn client(bus: &Bus) -> zbus::Connection {
    connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap()
}

async fn property<T>(host: &zbus::Connection, item: &str, path: &str, iface: &str, name: &str) -> T
where
    T: TryFrom<OwnedValue>,
    T::Error: std::fmt::Debug,
{
    let reply = host
        .call_method(
            Some(item),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(iface, name),
        )
        .await
        .unwrap();
    let value: OwnedValue = reply.body().deserialize().unwrap();
    T::try_from(value).unwrap()
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..100 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never happened: {what}");
}

/// Width, height and ARGB pixels, as the item gives its pictures.
type Icon = Vec<(i32, i32, Vec<u8>)>;

const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";

#[tokio::test]
async fn no_icon_without_a_watcher_or_a_host() {
    let Some(bus) = Bus::start() else { return };
    let (tray, _) = start(&bus).await;
    assert!(tray.is_none(), "no watcher runs");

    let (_watcher, registered) = watcher(&bus, false).await;
    let (tray, _) = start(&bus).await;
    assert!(tray.is_none(), "no host shows icons");
    assert!(registered.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_host_reads_the_icon_and_its_menu_and_clicks() {
    let Some(bus) = Bus::start() else { return };
    let (_watcher, registered) = watcher(&bus, true).await;
    let (tray, events) = start(&bus).await;
    let tray = tray.expect("shown");
    assert_eq!(*registered.lock().unwrap(), vec![tray.name()]);
    let item = tray.name();
    let host = client(&bus).await;

    assert_eq!(
        property::<String>(&host, &item, ITEM_PATH, ITEM, "Category").await,
        "Communications"
    );
    assert_eq!(
        property::<String>(&host, &item, ITEM_PATH, ITEM, "Id").await,
        "fogata"
    );
    assert_eq!(
        property::<String>(&host, &item, ITEM_PATH, ITEM, "Title").await,
        "Fogata"
    );
    assert_eq!(
        property::<String>(&host, &item, ITEM_PATH, ITEM, "Status").await,
        "Active"
    );
    assert!(!property::<bool>(&host, &item, ITEM_PATH, ITEM, "ItemIsMenu").await);
    assert_eq!(
        property::<OwnedObjectPath>(&host, &item, ITEM_PATH, ITEM, "Menu")
            .await
            .as_str(),
        MENU_PATH
    );
    let icon: Icon = property(&host, &item, ITEM_PATH, ITEM, "IconPixmap").await;
    assert_eq!(icon, vec![(2, 1, vec![40, 10, 20, 30, 80, 50, 60, 70])]);
    let tip: (String, Icon, String, String) =
        property(&host, &item, ITEM_PATH, ITEM, "ToolTip").await;
    assert_eq!(tip.2, "Fogata");
    assert_eq!(
        property::<u32>(&host, &item, MENU_PATH, MENU, "Version").await,
        3
    );

    // A click on the icon.
    host.call_method(
        Some(item.as_str()),
        ITEM_PATH,
        Some(ITEM),
        "Activate",
        &(10i32, 20i32),
    )
    .await
    .unwrap();
    assert_eq!(*events.lock().unwrap(), vec![Event::Activate]);

    // The whole menu, as Plasma asks for it.
    let reply = host
        .call_method(
            Some(item.as_str()),
            MENU_PATH,
            Some(MENU),
            "GetLayout",
            &(0i32, -1i32, Vec::<String>::new()),
        )
        .await
        .unwrap();
    type Entry = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let (revision, (root, root_props, children)): (u32, Entry) =
        reply.body().deserialize().unwrap();
    assert_eq!(revision, 1);
    assert_eq!(root, 0);
    assert_eq!(
        String::try_from(root_props["children-display"].try_clone().unwrap()).unwrap(),
        "submenu"
    );
    let entries: Vec<Entry> = children
        .into_iter()
        .map(|c| Entry::try_from(c).unwrap())
        .collect();
    let text = |props: &HashMap<String, OwnedValue>, key: &str| {
        props
            .get(key)
            .map(|v| String::try_from(v.try_clone().unwrap()).unwrap())
    };
    assert_eq!(
        entries.iter().map(|e| e.0).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(text(&entries[0].1, "label").as_deref(), Some("Open Fogata"));
    assert_eq!(text(&entries[1].1, "type").as_deref(), Some("separator"));
    assert_eq!(text(&entries[2].1, "label").as_deref(), Some("Quit Fogata"));
    assert!(entries.iter().all(|e| e.2.is_empty()));

    // Only the asked properties, of some entries.
    let reply = host
        .call_method(
            Some(item.as_str()),
            MENU_PATH,
            Some(MENU),
            "GetGroupProperties",
            &(vec![1i32, 3], vec!["label".to_owned()]),
        )
        .await
        .unwrap();
    let group: Vec<(i32, HashMap<String, OwnedValue>)> = reply.body().deserialize().unwrap();
    assert_eq!(group.len(), 2);
    assert!(group.iter().all(|(_, props)| props.len() == 1));
    assert_eq!(text(&group[1].1, "label").as_deref(), Some("Quit Fogata"));

    // Opening the menu needs no update.
    let reply = host
        .call_method(
            Some(item.as_str()),
            MENU_PATH,
            Some(MENU),
            "AboutToShow",
            &(0i32,),
        )
        .await
        .unwrap();
    assert!(!reply.body().deserialize::<bool>().unwrap());

    // Choosing entries, one at a time and in a group; hovering chooses nothing.
    host.call_method(
        Some(item.as_str()),
        MENU_PATH,
        Some(MENU),
        "Event",
        &(1i32, "clicked", Value::from(0i32), 0u32),
    )
    .await
    .unwrap();
    host.call_method(
        Some(item.as_str()),
        MENU_PATH,
        Some(MENU),
        "Event",
        &(3i32, "hovered", Value::from(0i32), 0u32),
    )
    .await
    .unwrap();
    let reply = host
        .call_method(
            Some(item.as_str()),
            MENU_PATH,
            Some(MENU),
            "EventGroup",
            &(vec![
                (3i32, "clicked", Value::from(0i32), 0u32),
                (9i32, "clicked", Value::from(0i32), 0u32),
            ],),
        )
        .await
        .unwrap();
    assert_eq!(reply.body().deserialize::<Vec<i32>>().unwrap(), vec![9]);
    assert!(
        host.call_method(
            Some(item.as_str()),
            MENU_PATH,
            Some(MENU),
            "Event",
            &(7i32, "clicked", Value::from(0i32), 0u32),
        )
        .await
        .is_err()
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            Event::Activate,
            Event::Item("open".into()),
            Event::Item("quit".into())
        ]
    );
}

#[tokio::test]
async fn the_icon_registers_again_when_the_watcher_starts_again_and_goes_when_dropped() {
    let Some(bus) = Bus::start() else { return };
    let (first, registered) = watcher(&bus, true).await;
    let (tray, _) = start(&bus).await;
    let tray = tray.expect("shown");
    let name = tray.name();
    assert_eq!(*registered.lock().unwrap(), vec![name.clone()]);

    // The desktop's watcher stops and another starts (Plasma starting again).
    first.close().await.unwrap();
    let (_second, again) = watcher(&bus, true).await;
    until("registered with the new watcher", || {
        *again.lock().unwrap() == vec![name.clone()]
    })
    .await;

    // Taken away: its name leaves the bus.
    drop(tray);
    let host = client(&bus).await;
    let dbus = fdo::DBusProxy::new(&host).await.unwrap();
    for _ in 0..100 {
        let unique = zbus::names::BusName::try_from(name.as_str()).unwrap();
        if !dbus.name_has_owner(unique).await.unwrap() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the icon stayed on the bus");
}
