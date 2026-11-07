//! The tray icon on Linux, served by the application itself on the session bus: a
//! StatusNotifierItem (`org.kde.StatusNotifierItem`) with its menu as a
//! `com.canonical.dbusmenu` object. Desktops that show tray icons (KDE Plasma, GNOME with its
//! AppIndicator extension, most panels) run a watcher, `org.kde.StatusNotifierWatcher`, that
//! the item registers with; the desktop's host then reads the icon and the menu from the item
//! and calls it back on a click. The item goes under the connection's own unique name, so
//! inside a sandbox it needs only to talk to the watcher, not to own a name.

#![forbid(unsafe_code)]
#![cfg(target_os = "linux")]

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::StreamExt as _;
use serde::Serialize;
use zbus::{
    connection, fdo, interface, proxy,
    proxy::CacheProperties,
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, StructureBuilder, Type, Value},
};

/// The watcher's well-known name, which is also its interface.
pub const WATCHER: &str = "org.kde.StatusNotifierWatcher";
/// Where the item is served.
pub const ITEM_PATH: &str = "/StatusNotifierItem";
/// Where its menu is served.
pub const MENU_PATH: &str = "/MenuBar";
/// Longest wait for an answer on the bus: the watcher answers at once when it runs.
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// What the icon shows and offers.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Names the item for the desktop, which remembers by it whether to show it.
    pub id: String,
    /// Its name and tooltip.
    pub title: String,
    /// The icon in one or more sizes; the host picks the one that fits.
    pub icon: Vec<Pixmap>,
    /// The menu, in order.
    pub items: Vec<Item>,
}

/// An image, in rows of red, green, blue and alpha bytes, not premultiplied.
#[derive(Debug, Clone)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// An entry that says `label` and, when chosen, gives [`Event::Item`] with `key`.
    Action { key: String, label: String },
    /// A line between entries.
    Separator,
}

/// What the person did with the icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A click on the icon itself.
    Activate,
    /// The menu entry with this key was chosen.
    Item(String),
}

type OnEvent = Arc<dyn Fn(Event) + Send + Sync>;

/// The icon while it is shown; dropping it takes the icon away.
#[derive(Debug)]
pub struct Tray {
    connection: zbus::Connection,
    reregister: tokio::task::JoinHandle<()>,
}

impl Tray {
    /// The bus name the item is served under.
    pub fn name(&self) -> String {
        self.connection
            .unique_name()
            .map(|n| n.to_string())
            .unwrap_or_default()
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.reregister.abort();
    }
}

/// Shows the icon on the session bus. `None` when the desktop shows no tray icons there (no
/// watcher, or no host registered with it): an icon made then would never be seen. Needs a
/// Tokio runtime, which keeps the item registered when the watcher starts again (Plasma
/// starting again, say). `on_event` runs on the bus's own thread and should return soon.
pub async fn start(
    spec: Spec,
    on_event: impl Fn(Event) + Send + Sync + 'static,
) -> zbus::Result<Option<Tray>> {
    start_with(connection::Builder::session()?, spec, Arc::new(on_event)).await
}

/// As [`start`], on the bus at `address`.
pub async fn start_at(
    address: &str,
    spec: Spec,
    on_event: impl Fn(Event) + Send + Sync + 'static,
) -> zbus::Result<Option<Tray>> {
    start_with(
        connection::Builder::address(address)?,
        spec,
        Arc::new(on_event),
    )
    .await
}

async fn start_with(
    builder: connection::Builder<'_>,
    spec: Spec,
    on_event: OnEvent,
) -> zbus::Result<Option<Tray>> {
    let item = StatusNotifierItem {
        id: spec.id,
        title: spec.title,
        icon: spec.icon.iter().filter_map(argb).collect(),
        on_event: on_event.clone(),
    };
    let menu = DbusMenu {
        items: spec.items,
        on_event,
    };
    let connection = builder
        .method_timeout(CALL_TIMEOUT)
        .serve_at(ITEM_PATH, item)?
        .serve_at(MENU_PATH, menu)?
        .build()
        .await?;
    let watcher = WatcherProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    if !watcher
        .is_status_notifier_host_registered()
        .await
        .unwrap_or(false)
    {
        return Ok(None);
    }
    // Listened to before registering, so a watcher that starts again in between is not missed.
    let mut owners = fdo::DBusProxy::new(&connection)
        .await?
        .receive_name_owner_changed_with_args(&[(0, WATCHER)])
        .await?;
    let name = connection
        .unique_name()
        .map(|n| n.to_string())
        .ok_or_else(|| zbus::Error::Failure("no name on the bus".into()))?;
    if let Err(e) = watcher.register_status_notifier_item(&name).await {
        tracing::debug!("the tray's watcher did not take the icon: {e}");
        return Ok(None);
    }
    let reregister = tokio::spawn(async move {
        while let Some(change) = owners.next().await {
            let Ok(args) = change.args() else { continue };
            if args.new_owner().is_none() {
                continue;
            }
            match watcher.register_status_notifier_item(&name).await {
                Ok(()) => tracing::debug!("the tray icon registered again"),
                Err(e) => tracing::debug!("the tray icon could not register again: {e}"),
            }
        }
    });
    Ok(Some(Tray {
        connection,
        reregister,
    }))
}

/// One image as the protocol carries it: width, height and ARGB32 pixels in network byte
/// order. `None` when the bytes do not fit the size.
fn argb(pixmap: &Pixmap) -> Option<(i32, i32, Vec<u8>)> {
    let pixels = (pixmap.width as usize).checked_mul(pixmap.height as usize)?;
    if pixels == 0 || pixmap.rgba.len() != pixels.checked_mul(4)? {
        return None;
    }
    let data = pixmap
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect();
    Some((
        i32::try_from(pixmap.width).ok()?,
        i32::try_from(pixmap.height).ok()?,
        data,
    ))
}

#[proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher",
    gen_blocking = false
)]
trait Watcher {
    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> zbus::Result<bool>;
}

type Icon = Vec<(i32, i32, Vec<u8>)>;

struct StatusNotifierItem {
    id: String,
    title: String,
    icon: Icon,
    on_event: OnEvent,
}

/// The item as hosts read it. Its icon is the picture itself, which a host outside the
/// application's sandbox can show, rather than an icon name to find in a theme.
#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> String {
        "Communications".into()
    }

    #[zbus(property)]
    fn id(&self) -> String {
        self.id.clone()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        self.title.clone()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".into()
    }

    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Icon {
        self.icon.clone()
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Icon {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> String {
        String::new()
    }

    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Icon {
        Vec::new()
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> String {
        String::new()
    }

    /// Icon name, icon, title and description.
    #[zbus(property)]
    fn tool_tip(&self) -> (String, Icon, String, String) {
        (String::new(), Vec::new(), self.title.clone(), String::new())
    }

    /// A click on the icon activates it; the menu opens from the other button.
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        ObjectPath::from_static_str_unchecked(MENU_PATH).into()
    }

    fn activate(&self, _x: i32, _y: i32) {
        (self.on_event)(Event::Activate);
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {}

    /// Hosts that read [`Self::menu`] draw the menu themselves; there is no other to show.
    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: String) {}
}

struct DbusMenu {
    items: Vec<Item>,
    on_event: OnEvent,
}

/// A menu entry and its children, as `GetLayout` gives them.
#[derive(Debug, Serialize, Type)]
struct Layout {
    id: i32,
    properties: HashMap<String, OwnedValue>,
    children: Vec<OwnedValue>,
}

fn value<'a>(v: impl Into<Value<'a>>) -> OwnedValue {
    // Only values that carry a file descriptor fail to be owned.
    v.into()
        .try_to_owned()
        .expect("a value without file descriptors")
}

impl DbusMenu {
    /// The item with this id: 0 is the root, entries count from 1.
    fn entry(&self, id: i32) -> Option<Option<&Item>> {
        match id {
            0 => Some(None),
            // The id comes from whoever calls over the bus: the lowest i32 has no id - 1.
            _ => id
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| self.items.get(i))
                .map(Some),
        }
    }

    fn properties(&self, id: i32, names: &[String]) -> Option<HashMap<String, OwnedValue>> {
        let mut properties = HashMap::new();
        match self.entry(id)? {
            None => {
                properties.insert("children-display".to_owned(), value("submenu"));
            }
            Some(Item::Action { label, .. }) => {
                // An underscore marks the access key; a plain one is written twice.
                properties.insert("label".to_owned(), value(label.replace('_', "__")));
                properties.insert("enabled".to_owned(), value(true));
                properties.insert("visible".to_owned(), value(true));
            }
            Some(Item::Separator) => {
                properties.insert("type".to_owned(), value("separator"));
                properties.insert("visible".to_owned(), value(true));
            }
        }
        if !names.is_empty() {
            properties.retain(|name, _| names.contains(name));
        }
        Some(properties)
    }

    fn layout(&self, id: i32, depth: i32, names: &[String]) -> Option<Layout> {
        let properties = self.properties(id, names)?;
        let mut children = Vec::new();
        if id == 0 && depth != 0 {
            for child in 1..=self.items.len() as i32 {
                let Some(props) = self.properties(child, names) else {
                    continue;
                };
                let structure = StructureBuilder::new()
                    .add_field(child)
                    .add_field(props)
                    .add_field(Vec::<OwnedValue>::new())
                    .build()
                    .expect("a structure with fields");
                children.push(value(structure));
            }
        }
        Some(Layout {
            id,
            properties,
            children,
        })
    }

    fn handle(&self, id: i32, event: &str) -> bool {
        match self.entry(id) {
            Some(Some(Item::Action { key, .. })) => {
                if event == "clicked" {
                    (self.on_event)(Event::Item(key.clone()));
                }
                true
            }
            Some(_) => true,
            None => false,
        }
    }
}

fn unknown(id: i32) -> fdo::Error {
    fdo::Error::InvalidArgs(format!("no menu entry {id}"))
}

/// The menu, which never changes while it is shown: its revision stays 1.
#[interface(name = "com.canonical.dbusmenu")]
impl DbusMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".into()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "normal".into()
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(out_args("revision", "layout"))]
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> fdo::Result<(u32, Layout)> {
        let layout = self
            .layout(parent_id, recursion_depth, &property_names)
            .ok_or_else(|| unknown(parent_id))?;
        Ok((1, layout))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        // No ids asks for every entry.
        let ids = if ids.is_empty() {
            (0..=self.items.len() as i32).collect()
        } else {
            ids
        };
        ids.into_iter()
            .filter_map(|id| Some((id, self.properties(id, &property_names)?)))
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> fdo::Result<OwnedValue> {
        self.properties(id, &[])
            .ok_or_else(|| unknown(id))?
            .remove(&name)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("no property {name}")))
    }

    fn event(
        &self,
        id: i32,
        event_id: String,
        _data: OwnedValue,
        _timestamp: u32,
    ) -> fdo::Result<()> {
        if self.handle(id, &event_id) {
            Ok(())
        } else {
            Err(unknown(id))
        }
    }

    /// Ids it knows nothing of come back.
    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> fdo::Result<Vec<i32>> {
        let errors: Vec<i32> = events
            .iter()
            .filter(|(id, event, _, _)| !self.handle(*id, event))
            .map(|(id, ..)| *id)
            .collect();
        if !events.is_empty() && errors.len() == events.len() {
            return Err(fdo::Error::InvalidArgs("no such menu entries".into()));
        }
        Ok(errors)
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    #[zbus(out_args("updates_needed", "id_errors"))]
    fn about_to_show_group(&self, ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        let errors = ids
            .into_iter()
            .filter(|id| self.entry(*id).is_none())
            .collect();
        (Vec::new(), errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_become_argb_in_network_order() {
        let pixmap = Pixmap {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert_eq!(argb(&pixmap), Some((2, 1, vec![4, 1, 2, 3, 8, 5, 6, 7])));
    }

    #[test]
    fn a_pixmap_whose_bytes_do_not_fit_is_left_out() {
        let short = Pixmap {
            width: 2,
            height: 2,
            rgba: vec![0; 12],
        };
        let empty = Pixmap {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        assert_eq!(argb(&short), None);
        assert_eq!(argb(&empty), None);
    }

    /// Ids come from any program on the bus: none, however odd, is an entry or a panic.
    #[test]
    fn a_menu_id_out_of_range_names_no_entry() {
        let chosen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = chosen.clone();
        let menu = DbusMenu {
            items: vec![
                Item::Action {
                    key: "open".into(),
                    label: "Open".into(),
                },
                Item::Separator,
            ],
            on_event: Arc::new(move |event| seen.lock().unwrap().push(event)),
        };
        for id in [i32::MIN, -1, 3, i32::MAX] {
            assert!(menu.entry(id).is_none(), "{id}");
            assert!(!menu.handle(id, "clicked"), "{id}");
        }
        assert!(menu.handle(1, "clicked"));
        assert_eq!(*chosen.lock().unwrap(), [Event::Item("open".into())]);
    }
}
