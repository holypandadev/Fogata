//! New versions from the project's releases page (GitHub), besides friends and members.
//!
//! A copy reads the newest releases through the API ([`fogata_update::RELEASES_API`]) every
//! few hours, and fetches `release.json` and `release.sig` of those that carry them. What it
//! fetches is held to exactly the same rules as a release a friend offers: the release key's
//! signature, the fields, a newer build. The package for this platform is then fetched from
//! the asset with the file name the manifest gives, chunk by chunk, each chunk checked
//! against the manifest's hash as it arrives, going on from the last whole chunk after a
//! stop. The page and the connection are trusted for nothing but finding the files: a
//! release or a package that the key did not sign is refused as it would be from anyone.
//!
//! What the releases page learns: this computer's network address, that it runs Fogata (the
//! `User-Agent`), and when it looks. Settings turn it off.

use std::{collections::HashMap, path::Path, time::Duration};

use fogata_net::updates::FetchError;
use fogata_update::{Artifact, Signed};
use serde::Deserialize;
use tokio::io::AsyncWriteExt as _;

/// The most bytes read from the list of releases.
const MAX_LIST: usize = 4 * 1024 * 1024;
/// The releases looked at, newest first.
const LOOKED_AT: usize = 10;
/// Of those, the ones whose manifests are fetched.
const FETCHED: usize = 3;
/// A connection that sends nothing for this long is given up.
const IDLE: Duration = Duration::from_secs(60);

/// Where releases are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSource {
    /// The API's address: `https://api.github.com`, or a test server's.
    pub api: String,
    /// `owner/name` of the repository.
    pub repo: String,
}

impl WebSource {
    /// The project's own releases page.
    pub fn compiled_in() -> WebSource {
        WebSource {
            api: fogata_update::RELEASES_API.to_string(),
            repo: fogata_update::RELEASES_REPO.to_string(),
        }
    }
}

/// A release found on the page: its manifest, still to check, and where its packages are,
/// by file name.
#[derive(Clone, Debug)]
pub(crate) struct WebOffer {
    pub signed: Signed,
    pub packages: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Reads releases from a [`WebSource`].
#[derive(Clone, Debug)]
pub(crate) struct Web {
    client: reqwest::Client,
    source: WebSource,
}

impl Web {
    pub(crate) fn new(source: WebSource, version: &str) -> anyhow::Result<Web> {
        let client = reqwest::Client::builder()
            .user_agent(format!("Fogata/{version}"))
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(IDLE)
            // The real page is HTTPS only; a test server on this computer is plain HTTP.
            .https_only(source.api.starts_with("https://"))
            .build()?;
        Ok(Web { client, source })
    }

    /// The releases on the page that carry a manifest and its signature, newest first, at
    /// most [`FETCHED`].
    pub(crate) async fn releases(&self) -> Result<Vec<WebOffer>, FetchError> {
        let url = format!(
            "{}/repos/{}/releases?per_page={LOOKED_AT}",
            self.source.api.trim_end_matches('/'),
            self.source.repo
        );
        let list = self
            .get_small(&url, MAX_LIST, Some("application/vnd.github+json"))
            .await?;
        let releases: Vec<Release> = serde_json::from_slice(&list)
            .map_err(|e| FetchError::Unreachable(format!("the releases page: {e}")))?;
        let mut offers = Vec::new();
        for release in releases.into_iter().filter(|r| !r.draft) {
            if offers.len() == FETCHED {
                break;
            }
            let url_of = |name: &str| {
                release
                    .assets
                    .iter()
                    .find(|a| a.name == name)
                    .map(|a| a.browser_download_url.clone())
            };
            let (Some(manifest), Some(signature)) = (url_of("release.json"), url_of("release.sig"))
            else {
                continue;
            };
            let body = self
                .get_small(&manifest, fogata_update::manifest::MAX_MANIFEST, None)
                .await?;
            let signature = self.get_small(&signature, 1024, None).await?;
            let Ok(signed) = Signed::from_files(body, &signature) else {
                tracing::debug!("a release on the page has a manifest that does not read");
                continue;
            };
            let packages = release
                .assets
                .iter()
                .map(|a| (a.name.clone(), a.browser_download_url.clone()))
                .collect();
            offers.push(WebOffer { signed, packages });
        }
        Ok(offers)
    }

    /// Fetches a file of at most `max` bytes.
    async fn get_small(
        &self,
        url: &str,
        max: usize,
        accept: Option<&str>,
    ) -> Result<Vec<u8>, FetchError> {
        let mut request = self.client.get(url);
        if let Some(accept) = accept {
            request = request.header(reqwest::header::ACCEPT, accept);
        }
        let mut response = request
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(unreachable)?;
        if response.content_length().is_some_and(|n| n > max as u64) {
            return Err(too_large());
        }
        let mut bytes = Vec::new();
        while let Some(piece) = response.chunk().await.map_err(unreachable)? {
            if bytes.len() + piece.len() > max {
                return Err(too_large());
            }
            bytes.extend_from_slice(&piece);
        }
        Ok(bytes)
    }

    /// Downloads `artifact` from `url` into `part`, going on from the whole chunks a
    /// previous try left there (each hashed again). Every chunk is checked as it arrives; a
    /// damaged one ends the download, keeping the good ones before it. `progress` gets the
    /// bytes there so far. Returns once the whole package is there and hashes to what the
    /// manifest says.
    pub(crate) async fn fetch(
        &self,
        url: &str,
        artifact: &Artifact,
        part: &Path,
        mut progress: impl FnMut(u64),
    ) -> Result<(), FetchError> {
        let (mut file, mut whole, have) = fogata_net::updates::resume(part, artifact).await?;
        let chunk = fogata_update::manifest::CHUNK;
        let count = artifact.chunk_count();
        let mut next = have / chunk;
        progress(have);
        // The good chunks written so far are kept for the next try, whatever stops this one:
        // they are on the disk before the error goes up, not still on their way there.
        let fetched: Result<(), FetchError> = async {
            if next < count {
                let mut request = self.client.get(url);
                if have > 0 {
                    request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
                }
                let mut response = request
                    .send()
                    .await
                    .and_then(|r| r.error_for_status())
                    .map_err(unreachable)?;
                // A server that sends the whole file in place of the range asked for: what is
                // here already is skipped as it comes again.
                let mut skip = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                    0
                } else {
                    have
                };
                let mut buf: Vec<u8> = Vec::with_capacity(chunk as usize);
                while next < count {
                    let Some(mut piece) = response.chunk().await.map_err(unreachable)? else {
                        return Err(FetchError::Io(std::io::ErrorKind::UnexpectedEof.into()));
                    };
                    if skip > 0 {
                        let n = skip.min(piece.len() as u64);
                        let _ = piece.split_to(n as usize);
                        skip -= n;
                    }
                    while !piece.is_empty() && next < count {
                        let want = artifact.chunk_len(next) as usize - buf.len();
                        let take = want.min(piece.len());
                        buf.extend_from_slice(&piece.split_to(take));
                        if buf.len() < artifact.chunk_len(next) as usize {
                            continue;
                        }
                        if Some(*blake3::hash(&buf).as_bytes()) != artifact.chunk_hash(next) {
                            return Err(FetchError::Damaged);
                        }
                        whole.update(&buf);
                        file.write_all(&buf).await?;
                        buf.clear();
                        next += 1;
                        progress((next * chunk).min(artifact.size));
                    }
                    if !piece.is_empty() {
                        // More than the package's size.
                        return Err(FetchError::Damaged);
                    }
                }
            }
            Ok(())
        }
        .await;
        if let Err(e) = fetched {
            let _ = file.flush().await;
            return Err(e);
        }
        file.flush().await?;
        file.sync_data().await?;
        if *whole.finalize().as_bytes() != artifact.hash() {
            drop(file);
            let _ = tokio::fs::remove_file(part).await;
            return Err(FetchError::Damaged);
        }
        Ok(())
    }
}

fn unreachable(e: reqwest::Error) -> FetchError {
    FetchError::Unreachable(format!("the releases page: {e}"))
}

fn too_large() -> FetchError {
    FetchError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "the releases page sent more than a release holds",
    ))
}
