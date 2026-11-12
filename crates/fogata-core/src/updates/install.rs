//! Installing a release that is ready, the way each system takes it. The package is hashed
//! once more first. Then, in every case, the application quits and the new version starts
//! in its place; the data folder is never touched.
//!
//! - **Linux** (the Flatpak): the package is an OSTree repository holding the new version,
//!   its commit and summary signed by the Flatpak signing key, whose public half the first
//!   install carried (`tools/beta/flatpak.sh`): flatpak takes nothing from the origin that
//!   this key did not sign. The repository is unpacked into `updates/flatpak-repo` and served
//!   on the loopback interface, at the address the installed Flatpak names as its origin
//!   ([`FLATPAK_REPO_PORT`]), to requests for that address only; the server is first asked
//!   for the repository's `config` through the port, to see that it, and no other program,
//!   answers there. Then the Flatpak portal updates the application from there
//!   (`UpdateMonitor.Update`). The portal asks the person once whether Fogata may update
//!   itself and refuses a version that asks for more permissions; the sandbox stays as it
//!   is. The new version is started through the portal (`Spawn`, newest version), with no
//!   arguments, and this one quits.
//! - **Windows**: the application lives in `%LOCALAPPDATA%\Programs\Fogata`. The package
//!   (that folder's contents) is unpacked beside it as `Fogata.new`; a hidden PowerShell
//!   waits for this process to end, moves the old folder aside, puts the new one in its
//!   place, starts it and removes the old one (or puts it back and starts it, if the swap
//!   fails). A lock file beside the folder keeps a second install from working on the same
//!   folders meanwhile. Files written by Fogata itself carry no mark of the web, so
//!   SmartScreen does not ask again.
//! - **macOS**: the package is the `Fogata.app` bundle, unpacked into the data folder (inside
//!   the sandbox's container). The sandboxed application cannot replace its own bundle, so
//!   the helper inside it (`Contents/Library/Helpers/Fogata Updater.app`, signed by the same
//!   team, not sandboxed) is opened with this process's id and the new bundle's path. The
//!   helper takes the installed bundle to be the one it is inside of, takes the new bundle
//!   only from the place inside Fogata's container where it is unpacked, and only the id of
//!   a running process of the installed bundle; it waits for this process to end, copies the
//!   new bundle to a folder of its own that nothing else can write, checks that copy's
//!   signature (valid, for Fogata's identifier, by the team pinned in when it was built),
//!   clears the quarantine the sandbox put on it, swaps it in and opens it. This process
//!   stays a moment after opening the helper, so the helper finds it running.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use fogata_update::{Manifest, pack};

use super::store;

/// The loopback port the Flatpak's origin repository is served on while an update installs:
/// the first install names `http://127.0.0.1:<port>/` as the repository (see
/// `tools/beta/flatpak.sh`).
#[cfg(target_os = "linux")]
pub const FLATPAK_REPO_PORT: u16 = 47_811;

/// This copy cannot install itself.
#[derive(Debug)]
pub struct CannotInstall(pub String);

impl std::fmt::Display for CannotInstall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CannotInstall {}

/// How this copy installs a new version of itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installer {
    /// Through the Flatpak portal.
    Flatpak,
    /// By swapping its folder (Windows), `dir`.
    Folder { dir: PathBuf },
    /// By swapping its bundle (macOS), with the helper in it.
    Bundle { bundle: PathBuf, helper: PathBuf },
    /// It cannot, for this reason.
    None(String),
}

/// How this copy would install a new version of itself.
pub fn installer() -> Installer {
    #[cfg(target_os = "linux")]
    {
        if Path::new("/.flatpak-info").exists() {
            Installer::Flatpak
        } else {
            Installer::None("only the Flatpak of Fogata installs updates by itself".into())
        }
    }
    #[cfg(windows)]
    {
        windows::installer()
    }
    #[cfg(target_os = "macos")]
    {
        macos::installer()
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        Installer::None("this system has no updates".into())
    }
}

/// Installs `manifest`'s package for `platform`, kept in data folder `data`. On success the
/// caller quits at once: the new version is starting (or starts once this process ends).
pub(crate) async fn install(data: &Path, manifest: &Manifest, platform: &str) -> Result<()> {
    let installer = installer();
    if let Installer::None(reason) = &installer {
        return Err(CannotInstall(reason.clone()).into());
    }
    manifest.current(crate::now_ms() / 1000)?;
    let dir = store::dir(data);
    let artifact = manifest
        .artifacts
        .get(platform)
        .with_context(|| format!("build {} has no package for {platform}", manifest.build))?
        .clone();
    let package = store::package_path(&dir, platform);
    let checked = {
        let package = package.clone();
        tokio::task::spawn_blocking(move || fogata_net::updates::check_package(&package, &artifact))
            .await
            .map_err(fogata_net::task_failed)??
    };
    if !checked {
        let _ = fs::remove_file(&package);
        anyhow::bail!("the package is damaged on this disk; it will be fetched again");
    }
    match installer {
        #[cfg(target_os = "linux")]
        Installer::Flatpak => flatpak::install(data, &package, manifest).await,
        #[cfg(windows)]
        Installer::Folder { dir: app } => windows::install(data, &package, manifest, &app).await,
        #[cfg(target_os = "macos")]
        Installer::Bundle { bundle, helper } => {
            macos::install(data, &package, manifest, &bundle, &helper).await
        }
        other => Err(CannotInstall(format!("{other:?} cannot install here")).into()),
    }
}

/// Unpacks `package` into `dest`, in place of whatever an earlier try left there, held to
/// the package's own size. Blocking.
fn unpack(package: &Path, dest: &Path, max: u64) -> Result<pack::Unpacked> {
    remove_tree(dest)?;
    let mut input = io::BufReader::new(fs::File::open(package)?);
    match pack::extract(&mut input, dest, max) {
        Ok(unpacked) => Ok(unpacked),
        Err(e) => {
            let _ = remove_tree(dest);
            Err(e).with_context(|| format!("cannot unpack the update into {}", dest.display()))
        }
    }
}

/// Removes `path` and what is in it, never following a link: a link there goes alone.
fn remove_tree(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Leaves the note the new copy reads as it starts (`window/restart`, as the application
/// writes when it starts itself again): it then waits for this one to let go of the data
/// folder.
fn leave_restart_note(data: &Path) -> Result<()> {
    let window = data.join("window");
    fs::create_dir_all(&window)?;
    fs::write(
        window.join("restart"),
        format!("{{\"at\":{}}}", crate::now_ms()),
    )?;
    Ok(())
}

/// The `FOGATA_*` settings of this process, which a test copy's new version needs too.
#[cfg_attr(windows, allow(dead_code))]
fn fogata_environment() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(key, _)| key.starts_with("FOGATA_"))
        .collect()
}

#[cfg(target_os = "linux")]
mod flatpak {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        time::Duration,
    };

    use anyhow::{Context, Result, bail};
    use fogata_update::Manifest;
    use futures::StreamExt;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue, Value};

    use super::{FLATPAK_REPO_PORT, store};

    const PORTAL: &str = "org.freedesktop.portal.Flatpak";
    const PORTAL_PATH: &str = "/org/freedesktop/portal/Flatpak";
    /// A portal that says nothing for this long has stopped.
    const SILENCE: Duration = Duration::from_secs(10 * 60);
    /// `FLATPAK_SPAWN_FLAGS_LATEST_VERSION`: the newest version installed, not this one.
    const LATEST_VERSION: u32 = 2;

    pub(super) async fn install(data: &Path, package: &Path, manifest: &Manifest) -> Result<()> {
        let dir = store::dir(data);
        let repo = dir.join("flatpak-repo");
        let (package2, repo2) = (package.to_path_buf(), repo.clone());
        let max = manifest
            .artifacts
            .values()
            .map(|a| a.size)
            .max()
            .unwrap_or(0)
            .saturating_mul(8);
        tokio::task::spawn_blocking(move || super::unpack(&package2, &repo2, max))
            .await
            .map_err(fogata_net::task_failed)??;
        if !repo.join("config").is_file() {
            bail!("the update's package holds no Flatpak repository");
        }
        let listener = TcpListener::bind(("127.0.0.1", FLATPAK_REPO_PORT))
            .await
            .with_context(|| {
                format!("another program uses port {FLATPAK_REPO_PORT}, which the update needs")
            })?;
        let server = tokio::spawn(serve_repo(listener, repo.clone()));
        let result = match served_here(&repo).await {
            Ok(()) => update_through_portal().await,
            Err(e) => Err(e),
        };
        server.abort();
        let _ = super::remove_tree(&repo);
        result?;
        store::note_installing(&dir, manifest.build)?;
        super::leave_restart_note(data)?;
        start_newest().await
    }

    /// Asks the Flatpak portal to update this application from its origin repository and
    /// follows it to the end.
    async fn update_through_portal() -> Result<()> {
        let connection = zbus::Connection::session().await?;
        let portal = zbus::Proxy::new(&connection, PORTAL, PORTAL_PATH, PORTAL).await?;
        let monitor: OwnedObjectPath = portal
            .call("CreateUpdateMonitor", &(HashMap::<&str, Value<'_>>::new(),))
            .await
            .context("the Flatpak portal does not answer")?;
        let monitor = zbus::Proxy::new(
            &connection,
            PORTAL,
            monitor,
            "org.freedesktop.portal.Flatpak.UpdateMonitor",
        )
        .await?;
        let mut signals = monitor.receive_signal("Progress").await?;
        monitor
            .call::<_, _, ()>("Update", &("", HashMap::<&str, Value<'_>>::new()))
            .await
            .context("the Flatpak portal refused to update Fogata")?;
        let outcome = loop {
            let Ok(Some(signal)) = tokio::time::timeout(SILENCE, signals.next()).await else {
                break Err(anyhow::anyhow!("the Flatpak portal stopped answering"));
            };
            let (info,): (HashMap<String, OwnedValue>,) = signal.body().deserialize()?;
            let number = |key: &str| {
                info.get(key)
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or_default()
            };
            let text = |key: &str| {
                info.get(key)
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .unwrap_or_default()
            };
            match number("status") {
                0 => tracing::info!(
                    "Flatpak update: operation {} of {}, {} %",
                    number("op") + 1,
                    number("n_ops"),
                    number("progress")
                ),
                1 => break Err(anyhow::anyhow!("the Flatpak portal found no newer version")),
                2 => break Ok(()),
                _ => {
                    let (name, message) = (text("error"), text("error_message"));
                    break Err(if name.ends_with("AccessDenied") {
                        anyhow::anyhow!("updates of Fogata are not allowed on this computer")
                    } else if message.contains("permissions") {
                        anyhow::anyhow!(
                            "the new version asks for more permissions: update it with \
                             `flatpak update` instead"
                        )
                    } else if message.is_empty() {
                        anyhow::anyhow!("the Flatpak update failed: {name}")
                    } else {
                        anyhow::anyhow!("the Flatpak update failed: {message}")
                    });
                }
            }
        };
        let _ = monitor.call::<_, _, ()>("Close", &()).await;
        outcome
    }

    /// Asks the port for the repository's `config` and compares it with the file: the answer
    /// must come from this process's own server, not from another program on the port.
    async fn served_here(repo: &Path) -> Result<()> {
        let wrong = || anyhow::anyhow!("another program answers on port {FLATPAK_REPO_PORT}");
        let expected = tokio::fs::read(repo.join("config")).await?;
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", FLATPAK_REPO_PORT))
            .await
            .with_context(|| format!("cannot reach the update's port {FLATPAK_REPO_PORT}"))?;
        stream
            .write_all(
                format!("GET /config HTTP/1.1\r\nHost: 127.0.0.1:{FLATPAK_REPO_PORT}\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut answer = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut answer))
            .await
            .map_err(|_| wrong())??;
        let head_end = answer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(wrong)?;
        let head = String::from_utf8_lossy(&answer[..head_end]);
        if !head.starts_with("HTTP/1.1 200 ") || !head.contains(SERVER_MARK) {
            return Err(wrong());
        }
        if answer[head_end + 4..] != expected[..] {
            return Err(wrong());
        }
        Ok(())
    }

    /// Starts the newest installed version of the application through the portal, with no
    /// arguments (none of this one's are carried over) and its `FOGATA_*` settings.
    async fn start_newest() -> Result<()> {
        let c_string = |text: &str| {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            bytes
        };
        let argv: Vec<Vec<u8>> = std::env::args().take(1).map(|a| c_string(&a)).collect();
        let environment: HashMap<String, String> =
            super::fogata_environment().into_iter().collect();
        let connection = zbus::Connection::session().await?;
        let portal = zbus::Proxy::new(&connection, PORTAL, PORTAL_PATH, PORTAL).await?;
        // The new copy gets this one's standard input and outputs. They must be given: with
        // none, the portal passes the environment on a descriptor numbered 0, which the
        // flatpak command refuses.
        use std::os::fd::AsFd as _;
        let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
        let fds: HashMap<u32, Fd<'_>> = HashMap::from([
            (0, Fd::from(stdin.as_fd())),
            (1, Fd::from(stdout.as_fd())),
            (2, Fd::from(stderr.as_fd())),
        ]);
        let _pid: u32 = portal
            .call(
                "Spawn",
                &(
                    c_string("/"),
                    argv,
                    fds,
                    environment,
                    LATEST_VERSION,
                    HashMap::<&str, Value<'_>>::new(),
                ),
            )
            .await
            .context("the new version could not be started")?;
        Ok(())
    }

    /// A header every answer of this server carries, so that [`served_here`] knows it.
    const SERVER_MARK: &str = "Server: fogata-update";

    /// Serves the files of `repo` over plain HTTP on the loopback interface, for the Flatpak
    /// portal to pull from: GET and HEAD of files inside it, for requests that name the
    /// loopback address as their host, nothing else.
    async fn serve_repo(listener: TcpListener, repo: PathBuf) {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let repo = repo.clone();
            tokio::spawn(async move {
                if let Err(e) = answer(stream, &repo).await {
                    tracing::debug!("update repository: {e}");
                }
            });
        }
    }

    async fn answer(mut stream: tokio::net::TcpStream, repo: &Path) -> std::io::Result<()> {
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf))
                .await
                .map_err(|_| std::io::ErrorKind::TimedOut)??;
            if n == 0 || request.len() > 16 * 1024 {
                return Ok(());
            }
            request.extend_from_slice(&buf[..n]);
        }
        let text = String::from_utf8_lossy(&request);
        let mut lines = text.split("\r\n");
        let line = lines.next().unwrap_or_default();
        let mut parts = line.split(' ');
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        // A request made for another name (a page in a browser reaching the port through a
        // name of its own) is not answered.
        let host = lines
            .filter_map(|l| l.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim());
        if !host.is_some_and(loopback_host) {
            stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
            return stream.shutdown().await;
        }
        let file = match (method, repo_file(repo, target)) {
            ("GET" | "HEAD", Some(path)) => tokio::fs::File::open(&path).await.ok(),
            _ => None,
        };
        let Some(mut file) = file else {
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 404 Not Found\r\n{SERVER_MARK}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await?;
            return stream.shutdown().await;
        };
        let len = file.metadata().await?.len();
        let head = format!(
            "HTTP/1.1 200 OK\r\n{SERVER_MARK}\r\nContent-Length: {len}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).await?;
        if method == "GET" {
            tokio::io::copy(&mut file, &mut stream).await?;
        }
        stream.shutdown().await
    }

    /// Whether a request's `Host` names this server's own address.
    fn loopback_host(host: &str) -> bool {
        let name = host
            .strip_suffix(&format!(":{FLATPAK_REPO_PORT}"))
            .unwrap_or(host);
        name == "127.0.0.1" || name == "localhost"
    }

    /// The file of `repo` a request names: plain names only, a regular file, not a link.
    fn repo_file(repo: &Path, target: &str) -> Option<PathBuf> {
        let path = target.split('?').next()?.strip_prefix('/')?;
        if path.is_empty()
            || !path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
            || path.split('/').any(|p| p.is_empty() || p.starts_with('.'))
        {
            return None;
        }
        let full = path
            .split('/')
            .fold(repo.to_path_buf(), |p, part| p.join(part));
        // No step of the way may be a link.
        let mut at = repo.to_path_buf();
        for part in path.split('/') {
            at.push(part);
            if std::fs::symlink_metadata(&at)
                .ok()?
                .file_type()
                .is_symlink()
            {
                return None;
            }
        }
        std::fs::metadata(&full).ok()?.is_file().then_some(full)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn only_plain_files_of_the_repository_are_served() {
            let repo = std::env::temp_dir().join(format!("fogata-repo-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&repo);
            std::fs::create_dir_all(repo.join("objects/ab")).unwrap();
            std::fs::write(repo.join("config"), "[core]").unwrap();
            std::fs::write(repo.join("objects/ab/cd.commit"), "c").unwrap();
            assert_eq!(repo_file(&repo, "/config"), Some(repo.join("config")));
            assert!(repo_file(&repo, "/objects/ab/cd.commit").is_some());
            assert!(repo_file(&repo, "/summary").is_none());
            assert!(repo_file(&repo, "/../config").is_none());
            assert!(repo_file(&repo, "/objects/../config").is_none());
            assert!(repo_file(&repo, "/objects").is_none());
            assert!(repo_file(&repo, "/objects//ab/cd.commit").is_none());
            assert!(repo_file(&repo, "/%2e%2e/config").is_none());
            assert!(repo_file(&repo, "config").is_none());
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink("/etc", repo.join("etc")).unwrap();
                assert!(repo_file(&repo, "/etc/hostname").is_none());
            }
            let _ = std::fs::remove_dir_all(&repo);
        }

        #[test]
        fn only_requests_for_the_loopback_address_are_answered() {
            assert!(loopback_host("127.0.0.1:47811"));
            assert!(loopback_host("127.0.0.1"));
            assert!(loopback_host("localhost:47811"));
            assert!(!loopback_host("127.0.0.1:47812"));
            assert!(!loopback_host("fogata.example:47811"));
            assert!(!loopback_host("127.0.0.1.example"));
            assert!(!loopback_host(""));
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result, bail};
    use fogata_update::Manifest;

    use super::{Installer, store};

    /// Where the application is installed for one person, with no administrator.
    fn install_dir() -> Option<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(local).join("Programs").join("Fogata"))
    }

    pub(super) fn installer() -> Installer {
        let (Some(dir), Ok(exe)) = (install_dir(), std::env::current_exe()) else {
            return Installer::None("cannot tell where Fogata is installed".into());
        };
        let here = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let same = |a: &Path, b: &Path| {
            a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
        };
        if same(&here, &dir) {
            Installer::Folder { dir }
        } else {
            Installer::None(format!(
                "this copy runs from {}, not from {}, where updates go",
                here.display(),
                dir.display()
            ))
        }
    }

    /// Waits for Fogata to end, swaps the folders, starts the new one; puts the old one back
    /// and starts it when the swap fails; lets go of the install lock at the end. Each move
    /// is tried for 20 seconds (an antivirus holds new files a while); a copy that cannot be
    /// put back is started from where it is, and the next install puts it back. Paths come
    /// in the environment, so nothing needs quoting.
    const SWAP: &str = r#"
$ErrorActionPreference = 'Stop'
$app = $env:FOGATA_UPDATE_DIR; $new = $env:FOGATA_UPDATE_NEW; $old = $env:FOGATA_UPDATE_OLD
function Rename-Folder([string]$from, [string]$to) {
  for ($i = 0; $i -lt 40; $i++) {
    try { Rename-Item -LiteralPath $from -NewName (Split-Path $to -Leaf); return $true }
    catch { Start-Sleep -Milliseconds 500 }
  }
  return $false
}
function Open([string]$dir) {
  try { Start-Process -FilePath (Join-Path $dir 'fogata.exe') } catch {}
}
function Swap {
  try { Wait-Process -Id ([int]$env:FOGATA_UPDATE_PID) -Timeout 120 } catch {}
  if (Get-Process -Id ([int]$env:FOGATA_UPDATE_PID) -ErrorAction SilentlyContinue) { return 1 }
  if (Test-Path -LiteralPath $old) {
    try { Remove-Item -LiteralPath $old -Recurse -Force } catch { Open $app; return 1 }
  }
  if (-not (Rename-Folder $app $old)) { Open $app; return 1 }
  if (-not (Rename-Folder $new $app)) {
    if (Rename-Folder $old $app) { Open $app } else { Open $old }
    return 1
  }
  Open $app
  for ($i = 0; $i -lt 30; $i++) {
    try { Remove-Item -LiteralPath $old -Recurse -Force; break } catch { Start-Sleep -Seconds 1 }
  }
  return 0
}
$code = Swap
Remove-Item -LiteralPath $env:FOGATA_UPDATE_LOCK -Force -ErrorAction SilentlyContinue
exit [int]($code | Select-Object -Last 1)
"#;

    /// The step that waits for the process `pid` to end and swaps the folders (SWAP), started
    /// with no window. Not as a detached process: PowerShell started with no console at all
    /// (`DETACHED_PROCESS`) ends before it runs anything, so the swap never happened. A child
    /// goes on after its parent ends on Windows without it. The tests start it the same way.
    fn swap(pid: u32, app: &Path, new: &Path, old: &Path, lock: &Path) -> std::process::Command {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let powershell = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let mut command = std::process::Command::new(powershell);
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-Command",
                SWAP,
            ])
            .env("FOGATA_UPDATE_PID", pid.to_string())
            .env("FOGATA_UPDATE_DIR", app)
            .env("FOGATA_UPDATE_NEW", new)
            .env("FOGATA_UPDATE_OLD", old)
            .env("FOGATA_UPDATE_LOCK", lock)
            .creation_flags(CREATE_NO_WINDOW);
        command
    }

    /// An install left behind for this long is taken over.
    const STALE_LOCK: std::time::Duration = std::time::Duration::from_secs(30 * 60);

    /// Takes the lock an install of the folder holds from the unpacking to the end of the
    /// swap, so two installs never work on the same folders at once.
    fn take_lock(lock: &Path) -> Result<()> {
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock)
            {
                Ok(_) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(lock)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age > STALE_LOCK);
                    if !stale {
                        bail!("another install of Fogata is under way");
                    }
                    let _ = std::fs::remove_file(lock);
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!("another install of Fogata is under way")
    }

    pub(super) async fn install(
        data: &Path,
        package: &Path,
        manifest: &Manifest,
        app: &Path,
    ) -> Result<()> {
        let parent = app.parent().context("the install folder has no parent")?;
        let new = parent.join("Fogata.new");
        let old = parent.join("Fogata.old");
        let lock = parent.join("Fogata.update.lock");
        take_lock(&lock)?;
        let max = manifest
            .artifacts
            .values()
            .map(|a| a.size)
            .max()
            .unwrap_or(0)
            .saturating_mul(8);
        let (package2, new2) = (package.to_path_buf(), new.clone());
        let unpacked =
            tokio::task::spawn_blocking(move || super::unpack(&package2, &new2, max)).await;
        if let Err(e) = unpacked
            .map_err(|e| anyhow::Error::from(fogata_net::task_failed(e)))
            .and_then(|r| r)
        {
            let _ = std::fs::remove_file(&lock);
            return Err(e);
        }
        if !new.join("fogata.exe").is_file() {
            let _ = super::remove_tree(&new);
            let _ = std::fs::remove_file(&lock);
            bail!("the update's package holds no fogata.exe");
        }
        store::note_installing(&store::dir(data), manifest.build)?;
        super::leave_restart_note(data)?;
        let spawned = swap(std::process::id(), app, &new, &old, &lock).spawn();
        if let Err(e) = spawned {
            let _ = std::fs::remove_file(&lock);
            return Err(e).context("cannot start the step that puts the new version in place");
        }
        Ok(())
    }
    /// The swap run for real, by PowerShell, on folders in the temporary folder; the "copy"
    /// started is the system's hostname.exe, which ends at once.
    #[cfg(test)]
    mod tests {
        use std::fs;
        use std::os::windows::fs::OpenOptionsExt as _;
        use std::path::{Path, PathBuf};

        struct Folders {
            root: PathBuf,
            app: PathBuf,
            new: PathBuf,
            old: PathBuf,
            lock: PathBuf,
        }

        fn folders(name: &str) -> Folders {
            let root =
                std::env::temp_dir().join(format!("fogata-swap-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let folders = Folders {
                app: root.join("Fogata"),
                new: root.join("Fogata.new"),
                old: root.join("Fogata.old"),
                lock: root.join("Fogata.update.lock"),
                root,
            };
            copy(&folders.app, "installed");
            copy(&folders.new, "new");
            fs::write(&folders.lock, b"").unwrap();
            folders
        }

        fn copy(dir: &Path, which: &str) {
            fs::create_dir_all(dir).unwrap();
            let system = std::env::var_os("SystemRoot").unwrap();
            fs::copy(
                Path::new(&system).join(r"System32\HOSTNAME.EXE"),
                dir.join("fogata.exe"),
            )
            .unwrap();
            fs::write(dir.join("which.txt"), which).unwrap();
        }

        /// A file opened as an antivirus or another program would: it cannot be deleted or
        /// moved, nor its folder, while it is open.
        fn hold(path: &Path) -> fs::File {
            const FILE_SHARE_READ: u32 = 1;
            fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(path)
                .unwrap()
        }

        /// Runs the swap as Fogata would after ending, started as Fogata starts it; its exit
        /// code.
        fn run(f: &Folders) -> i32 {
            let mut ended = std::process::Command::new("cmd")
                .args(["/C", "exit"])
                .spawn()
                .unwrap();
            ended.wait().unwrap();
            super::swap(ended.id(), &f.app, &f.new, &f.old, &f.lock)
                .status()
                .unwrap()
                .code()
                .unwrap()
        }

        fn which(dir: &Path) -> Option<String> {
            fs::read_to_string(dir.join("which.txt")).ok()
        }

        fn clean(f: Folders) {
            for _ in 0..20 {
                if fs::remove_dir_all(&f.root).is_ok() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }

        #[test]
        fn an_update_takes_the_place_of_the_installed_copy() {
            let f = folders("plain");
            copy(&f.old, "left from before");
            assert_eq!(run(&f), 0);
            assert_eq!(which(&f.app).as_deref(), Some("new"));
            assert!(!f.new.exists() && !f.old.exists() && !f.lock.exists());
            clean(f);
        }

        #[test]
        fn a_leftover_copy_that_cannot_go_leaves_the_installed_one_running() {
            let f = folders("leftover");
            copy(&f.old, "left from before");
            let held = hold(&f.old.join("which.txt"));
            assert_eq!(run(&f), 1);
            assert_eq!(which(&f.app).as_deref(), Some("installed"));
            assert_eq!(which(&f.new).as_deref(), Some("new"));
            assert!(!f.lock.exists(), "the lock is let go of");
            drop(held);
            clean(f);
        }

        #[test]
        fn a_new_copy_held_a_moment_still_goes_in() {
            let f = folders("moment");
            let held = hold(&f.new.join("which.txt"));
            // Let go a second after the installed copy went aside: the first moves of the new
            // one have failed by then.
            let old = f.old.clone();
            let release = std::thread::spawn(move || {
                let start = std::time::Instant::now();
                while !old.exists() && start.elapsed() < std::time::Duration::from_secs(60) {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
                drop(held);
            });
            assert_eq!(run(&f), 0);
            release.join().unwrap();
            assert_eq!(which(&f.app).as_deref(), Some("new"));
            assert!(!f.new.exists() && !f.old.exists() && !f.lock.exists());
            clean(f);
        }

        #[test]
        fn a_new_copy_held_open_gives_the_installed_one_back() {
            let f = folders("held");
            let held = hold(&f.new.join("which.txt"));
            assert_eq!(run(&f), 1);
            assert_eq!(which(&f.app).as_deref(), Some("installed"));
            assert_eq!(which(&f.new).as_deref(), Some("new"));
            assert!(!f.old.exists() && !f.lock.exists());
            drop(held);
            clean(f);
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result, bail};
    use fogata_update::Manifest;

    use super::{Installer, store};

    pub(super) fn installer() -> Installer {
        let Ok(exe) = std::env::current_exe() else {
            return Installer::None("cannot tell where Fogata is".into());
        };
        // .../Fogata.app/Contents/MacOS/Fogata
        let Some(bundle) = exe.ancestors().nth(3).map(Path::to_path_buf) else {
            return Installer::None("Fogata is not in an application bundle".into());
        };
        if bundle.extension().and_then(|e| e.to_str()) != Some("app") {
            return Installer::None("Fogata is not in an application bundle".into());
        }
        let helper = bundle.join("Contents/Library/Helpers/Fogata Updater.app");
        if !helper.is_dir() {
            return Installer::None("this copy of Fogata has no updater in it".into());
        }
        Installer::Bundle { bundle, helper }
    }

    pub(super) async fn install(
        data: &Path,
        package: &Path,
        manifest: &Manifest,
        bundle: &Path,
        helper: &Path,
    ) -> Result<()> {
        if !bundle.join("Contents/MacOS/Fogata").is_file() {
            bail!("Fogata is not in an application bundle");
        }
        let staged: PathBuf = store::dir(data).join("Fogata.app");
        let max = manifest
            .artifacts
            .values()
            .map(|a| a.size)
            .max()
            .unwrap_or(0)
            .saturating_mul(8);
        let (package2, staged2) = (package.to_path_buf(), staged.clone());
        tokio::task::spawn_blocking(move || super::unpack(&package2, &staged2, max))
            .await
            .map_err(fogata_net::task_failed)??;
        if !staged.join("Contents/Info.plist").is_file() {
            let _ = super::remove_tree(&staged);
            bail!("the update's package holds no application bundle");
        }
        store::note_installing(&store::dir(data), manifest.build)?;
        super::leave_restart_note(data)?;
        // Launch Services opens the helper as an application of its own, with its own
        // entitlements: it is not in this one's sandbox.
        let mut open = std::process::Command::new("/usr/bin/open");
        open.args(["-n", "-g", "-a"]).arg(helper);
        for (key, value) in super::fogata_environment() {
            open.arg("--env").arg(format!("{key}={value}"));
        }
        open.arg("--args")
            .arg(std::process::id().to_string())
            .arg(&staged);
        let status = open
            .status()
            .context("cannot open the updater inside Fogata")?;
        if !status.success() {
            bail!("the updater inside Fogata did not open ({status})");
        }
        // The helper takes the id only of a process that is running when it looks.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        Ok(())
    }
}
