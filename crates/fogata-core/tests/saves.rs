//! The friend book is written whole, so a burst of changes is written once a moment later;
//! what waits is written when the core goes, with a shutdown or without one (a command that
//! ends right after a change).

use std::time::Duration;

use fogata_core::{Core, NetOptions, Store};

#[tokio::test(flavor = "multi_thread")]
async fn a_change_that_waits_is_written_when_the_core_goes() {
    for shut_down in [true, false] {
        let dir =
            std::env::temp_dir().join(format!("fogata-saves-{}-{shut_down}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Core::create_profile(&dir, "Ana", None).unwrap();
        let options = NetOptions {
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, events) = Core::start(dir.clone(), options).unwrap();
        core.set_profile(Some("Ana One".into()), None)
            .await
            .unwrap();
        // Written at once.
        let written = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let book = Store::open(dir.clone()).unwrap().friends();
                if book.is_ok_and(|b| b.is_some_and(|b| b.profile.name == "Ana One")) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert!(written.is_ok(), "the first change was not written");
        core.set_profile(Some("Ana Two".into()), None)
            .await
            .unwrap();
        // The second one waits to be written while the core goes on a moment.
        tokio::time::sleep(Duration::from_millis(500)).await;
        if shut_down {
            core.shutdown().await.unwrap();
        }
        drop(core);
        drop(events);
        let book = Store::open(dir.clone())
            .unwrap()
            .friends()
            .unwrap()
            .unwrap();
        assert_eq!(book.profile.name, "Ana Two", "shut down: {shut_down}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
