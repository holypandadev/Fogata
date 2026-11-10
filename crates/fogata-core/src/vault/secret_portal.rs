//! The secret the Secret portal (`org.freedesktop.portal.Secret`) gives a sandboxed
//! application: its own, the same at every call, kept by the desktop's keychain (GNOME
//! Keyring, KWallet) and given to no other application. The Flatpak seals its data keys with
//! it ([`super::file::FileKeyStore`]) and so needs no access to the whole Secret Service.

use std::{
    collections::HashMap,
    io::Read as _,
    os::{fd::AsFd as _, unix::net::UnixStream},
    time::{Duration, Instant},
};

use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue, Value};
use zeroize::Zeroizing;

const DESKTOP: &str = "org.freedesktop.portal.Desktop";
const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const SECRET: &str = "org.freedesktop.portal.Secret";
const REQUEST: &str = "org.freedesktop.portal.Request";
/// The longest secret read: the backends give 64 bytes.
const MAX_SECRET: usize = 4096;

/// How long a portal that has no Secret portal yet is waited for: one started by this very
/// call (at login, say) offers its portals as it finds their backends.
const STARTING: Duration = Duration::from_secs(20);

/// Asks the portal for this application's secret.
pub(super) fn retrieve() -> Result<Zeroizing<Vec<u8>>, String> {
    let started = Instant::now();
    loop {
        match retrieve_inner() {
            Err(Failure::NotYet(e)) if started.elapsed() < STARTING => {
                tracing::debug!("the Secret portal is not there yet ({e})");
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(Failure::NotYet(e) | Failure::Other(e)) => {
                return Err(format!("the Secret portal: {e}"));
            }
            Ok(secret) => return Ok(secret),
        }
    }
}

enum Failure {
    /// The portal does not offer the Secret portal (yet).
    NotYet(String),
    Other(String),
}

impl From<String> for Failure {
    fn from(e: String) -> Self {
        Failure::Other(e)
    }
}

impl From<&str> for Failure {
    fn from(e: &str) -> Self {
        Failure::Other(e.to_string())
    }
}

fn retrieve_inner() -> Result<Zeroizing<Vec<u8>>, Failure> {
    let fail = |e: zbus::Error| e.to_string();
    let connection = zbus::blocking::Connection::session().map_err(fail)?;
    let mut token = [0u8; 8];
    fogata_crypto::random_bytes(&mut token).map_err(|e| e.to_string())?;
    let token = format!("fogata_{}", hex::encode(token));
    // The request's path is known before the call, so its answer cannot come before this
    // process listens for it.
    let sender = connection
        .unique_name()
        .ok_or("no name on the session bus")?
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("{DESKTOP_PATH}/request/{sender}/{token}");
    let request =
        zbus::blocking::Proxy::new(&connection, DESKTOP, path.as_str(), REQUEST).map_err(fail)?;
    let mut answers = request.receive_signal("Response").map_err(fail)?;

    let (mut ours, theirs) = UnixStream::pair().map_err(|e| e.to_string())?;
    let portal =
        zbus::blocking::Proxy::new(&connection, DESKTOP, DESKTOP_PATH, SECRET).map_err(fail)?;
    let options = HashMap::from([("handle_token", Value::from(token.as_str()))]);
    let handle: OwnedObjectPath = portal
        .call("RetrieveSecret", &(Fd::from(theirs.as_fd()), options))
        .map_err(|e| match &e {
            zbus::Error::MethodError(name, _, _)
                if [
                    "org.freedesktop.DBus.Error.UnknownMethod",
                    "org.freedesktop.DBus.Error.UnknownInterface",
                    "org.freedesktop.DBus.Error.ServiceUnknown",
                ]
                .contains(&name.as_str()) =>
            {
                Failure::NotYet(e.to_string())
            }
            _ => Failure::Other(e.to_string()),
        })?;
    drop(theirs);
    if handle.as_str() != path {
        return Err(format!("it answered on {handle}, not {path}").into());
    }
    let answer = answers.next().ok_or("it closed the request")?;
    let (response, _): (u32, HashMap<String, OwnedValue>) =
        answer.body().deserialize().map_err(fail)?;
    if response != 0 {
        return Err(format!("it refused (response {response})").into());
    }
    // The backend wrote the secret before it answered; what is there is read without
    // waiting for a writer that may stay open.
    ours.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut buf = Zeroizing::new([0u8; 512]);
    // Room for all it may read from the start: a vector that grows leaves behind, unwiped,
    // the copies of what it held.
    let mut secret = Zeroizing::new(Vec::with_capacity(MAX_SECRET + buf.len()));
    loop {
        match ours.read(buf.as_mut_slice()) {
            Ok(0) => break,
            Ok(n) => {
                secret.extend_from_slice(&buf[..n]);
                if secret.len() > MAX_SECRET {
                    return Err("it gave a secret too long".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string().into()),
        }
    }
    if secret.len() < 16 {
        return Err("it gave no secret".into());
    }
    Ok(secret)
}
