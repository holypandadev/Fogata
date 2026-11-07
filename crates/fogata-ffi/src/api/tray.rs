//! The tray icon on Linux, which the application serves itself on the session bus
//! (fogata-tray). macOS and Windows use the tray_manager plugin instead; there these calls
//! show nothing.

use std::sync::Mutex;

use anyhow::Result;

use crate::frb_generated::StreamSink;

/// A menu entry; one with an empty `key` is a line between entries.
#[derive(Clone, Debug)]
pub struct UiTrayItem {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTrayEventKind {
    /// A click on the icon itself.
    Activate,
    /// The menu entry `key` was chosen.
    Item,
}

#[derive(Clone, Debug)]
pub struct UiTrayEvent {
    pub kind: UiTrayEventKind,
    pub key: String,
}

static EVENTS: Mutex<Option<StreamSink<UiTrayEvent>>> = Mutex::new(None);

#[cfg(target_os = "linux")]
static TRAY: Mutex<Option<fogata_tray::Tray>> = Mutex::new(None);

/// Clicks on the icon and its menu from now on; a new call replaces the previous stream.
pub fn tray_events(sink: StreamSink<UiTrayEvent>) -> Result<()> {
    *EVENTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
    Ok(())
}

/// Shows the icon, a `width` × `height` picture in rows of red, green, blue and alpha
/// bytes, named `title`, with the menu `items`. False when the desktop shows no tray
/// icons (no StatusNotifier host, or no session bus): none is made then.
pub async fn show_tray(
    title: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    items: Vec<UiTrayItem>,
) -> Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use fogata_tray::{Event, Item, Pixmap, Spec};

        let spec = Spec {
            id: "fogata".into(),
            title,
            icon: vec![Pixmap {
                width,
                height,
                rgba,
            }],
            items: items
                .into_iter()
                .map(|i| {
                    if i.key.is_empty() {
                        Item::Separator
                    } else {
                        Item::Action {
                            key: i.key,
                            label: i.label,
                        }
                    }
                })
                .collect(),
        };
        let on_event = |event: Event| {
            let event = match event {
                Event::Activate => UiTrayEvent {
                    kind: UiTrayEventKind::Activate,
                    key: String::new(),
                },
                Event::Item(key) => UiTrayEvent {
                    kind: UiTrayEventKind::Item,
                    key,
                },
            };
            if let Some(sink) = EVENTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                let _ = sink.add(event);
            }
        };
        // On the core's runtime, which keeps the icon registered for as long as it is shown.
        let tray = super::app::RUNTIME
            .spawn(fogata_tray::start(spec, on_event))
            .await?;
        let tray = match tray {
            Ok(tray) => tray,
            Err(e) => {
                tracing::info!("no tray icon: {e}");
                None
            }
        };
        let shown = tray.is_some();
        *TRAY.lock().unwrap_or_else(|e| e.into_inner()) = tray;
        Ok(shown)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (title, width, height, rgba, items);
        Ok(false)
    }
}

/// Takes the icon away, as the application quits.
pub fn hide_tray() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // Its connection to the bus closes with it, and the watcher forgets it.
        drop(TRAY.lock().unwrap_or_else(|e| e.into_inner()).take());
    }
    Ok(())
}
