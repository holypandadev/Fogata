//! Measures how real-time packets fare over member connections: how late they arrive and how
//! many never arrive. libp2p streams are reliable and ordered, so one lost packet delays the
//! ones behind it on the same stream; the modes compare ways of limiting that effect.

use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fogata_net::{IncomingStreams, PeerId, Stream, StreamControl, StreamProtocol};
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use tokio::sync::mpsc;

pub const PROBE_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/media-probe/1");

/// How packets are spread over streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One stream for everything.
    Single,
    /// `n` streams opened up front, used in turn: a loss only delays packets on its stream.
    Striped(usize),
    /// A new stream for every packet: no packet waits for another.
    PerPacket,
    /// Unreliable QUIC datagrams: a lost packet is gone and delays nothing.
    Datagram,
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "single" => Ok(Mode::Single),
            "per-packet" => Ok(Mode::PerPacket),
            "datagram" => Ok(Mode::Datagram),
            _ => s
                .strip_prefix("striped")
                .map(|n| n.parse().unwrap_or(4))
                .filter(|n| *n > 0)
                .map(Mode::Striped)
                .ok_or_else(|| {
                    format!("unknown mode {s}: single, stripedN, per-packet or datagram")
                }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProbeConfig {
    pub mode: Mode,
    pub packets: u32,
    pub size: usize,
    pub interval: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("could not open a stream: {0}")]
    Open(String),
    #[error("no direct QUIC connection that carries datagrams")]
    NoDatagrams,
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

fn packet(seq: u32, size: usize) -> Vec<u8> {
    let mut p = vec![0u8; size.max(12)];
    p[..4].copy_from_slice(&seq.to_be_bytes());
    p[4..12].copy_from_slice(&now_us().to_be_bytes());
    p
}

async fn write_frame(stream: &mut Stream, payload: &[u8]) -> std::io::Result<()> {
    stream
        .write_all(&(payload.len() as u16).to_be_bytes())
        .await?;
    stream.write_all(payload).await?;
    stream.flush().await
}

async fn read_frame(stream: &mut Stream) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).await?;
    let mut payload = vec![0u8; u16::from_be_bytes(len) as usize];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

/// Sends `config.packets` packets to `peer` at a steady pace.
pub async fn send(
    mut control: StreamControl,
    peer: PeerId,
    config: ProbeConfig,
) -> Result<(), ProbeError> {
    let mut writers: Vec<mpsc::UnboundedSender<Vec<u8>>> = Vec::new();
    if config.mode == Mode::Datagram {
        return send_datagrams(peer, config).await;
    }
    let stream_count = match config.mode {
        Mode::Single => 1,
        Mode::Striped(n) => n,
        Mode::PerPacket | Mode::Datagram => 0,
    };
    for _ in 0..stream_count {
        let mut stream = control
            .open_stream(peer, PROBE_PROTOCOL)
            .await
            .map_err(|e| ProbeError::Open(e.to_string()))?;
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(p) = rx.recv().await {
                if write_frame(&mut stream, &p).await.is_err() {
                    break;
                }
            }
            let _ = stream.close().await;
        });
        writers.push(tx);
    }

    let mut ticks = tokio::time::interval(config.interval);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for seq in 0..config.packets {
        ticks.tick().await;
        let p = packet(seq, config.size);
        if writers.is_empty() {
            let mut control = control.clone();
            tokio::spawn(async move {
                if let Ok(mut stream) = control.open_stream(peer, PROBE_PROTOCOL).await {
                    let _ = write_frame(&mut stream, &p).await;
                    let _ = stream.close().await;
                }
            });
        } else {
            let _ = writers[seq as usize % writers.len()].send(p);
        }
    }
    // Leave time for the last packets to leave before the caller shuts the node down.
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

async fn send_datagrams(peer: PeerId, config: ProbeConfig) -> Result<(), ProbeError> {
    let link = fogata_net::datagram::link(&peer).ok_or(ProbeError::NoDatagrams)?;
    let size = link
        .max_size()
        .map_or(config.size, |max| config.size.min(max));
    if size < config.size {
        tracing::warn!("datagrams limited to {size} bytes on this path");
    }
    let mut ticks = tokio::time::interval(config.interval);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    for seq in 0..config.packets {
        ticks.tick().await;
        if let Err(e) = link.send(bytes::Bytes::from(packet(seq, size))) {
            tracing::debug!("datagram {seq} not sent: {e}");
        }
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

/// Result of a probe run, as seen by the receiver.
#[derive(Debug, Clone)]
pub struct Report {
    pub expected: u32,
    pub received: u32,
    /// One-way delays in milliseconds, sorted.
    pub delays_ms: Vec<f64>,
}

impl Report {
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.delays_ms.is_empty() {
            return None;
        }
        let i = ((self.delays_ms.len() - 1) as f64 * p).round() as usize;
        Some(self.delays_ms[i])
    }

    /// Share of expected packets that arrived later than `ms`, or never.
    pub fn late_or_lost(&self, ms: f64) -> f64 {
        let late = self.delays_ms.iter().filter(|d| **d > ms).count() as u32;
        let missing = self.expected.saturating_sub(self.received);
        f64::from(late + missing) / f64::from(self.expected.max(1))
    }
}

/// Receives probe packets from members until `config.packets` arrived or the stream of
/// packets has been quiet for a few seconds after starting.
pub async fn receive(
    mut incoming: IncomingStreams,
    is_member: impl Fn(&PeerId) -> bool + Send + Sync + 'static,
    config: ProbeConfig,
    give_up: Duration,
) -> Report {
    let (tx, mut rx) = mpsc::unbounded_channel::<(u32, f64)>();
    let is_member = std::sync::Arc::new(is_member);
    let mut datagrams = {
        let is_member = is_member.clone();
        fogata_net::datagram::receive(4096, move |peer| is_member(peer))
    };
    let datagram_task = {
        let tx = tx.clone();
        let is_member = is_member.clone();
        tokio::spawn(async move {
            while let Some((peer, p)) = datagrams.next().await {
                if p.len() < 12 || !is_member(&peer) {
                    continue;
                }
                let seq = u32::from_be_bytes(p[..4].try_into().expect("4 bytes"));
                let sent = u64::from_be_bytes(p[4..12].try_into().expect("8 bytes"));
                let delay = now_us().saturating_sub(sent) as f64 / 1000.0;
                let _ = tx.send((seq, delay));
            }
        })
    };
    let accept = tokio::spawn(async move {
        while let Some((peer, mut stream)) = incoming.next().await {
            if !is_member(&peer) {
                continue;
            }
            let tx = tx.clone();
            tokio::spawn(async move {
                while let Ok(p) = read_frame(&mut stream).await {
                    if p.len() < 12 {
                        continue;
                    }
                    let seq = u32::from_be_bytes(p[..4].try_into().expect("4 bytes"));
                    let sent = u64::from_be_bytes(p[4..12].try_into().expect("8 bytes"));
                    let delay = now_us().saturating_sub(sent) as f64 / 1000.0;
                    let _ = tx.send((seq, delay));
                }
            });
        }
    });

    let mut seen: HashMap<u32, f64> = HashMap::new();
    let deadline = tokio::time::sleep(give_up);
    tokio::pin!(deadline);
    let quiet = Duration::from_secs(5);
    loop {
        let next = if seen.is_empty() {
            tokio::select! {
                v = rx.recv() => v,
                _ = &mut deadline => None,
            }
        } else {
            tokio::select! {
                v = rx.recv() => v,
                _ = tokio::time::sleep(quiet) => None,
            }
        };
        let Some((seq, delay)) = next else { break };
        seen.entry(seq).or_insert(delay);
        if seen.len() as u32 >= config.packets {
            break;
        }
    }
    accept.abort();
    datagram_task.abort();
    let mut delays_ms: Vec<f64> = seen.values().copied().collect();
    delays_ms.sort_by(f64::total_cmp);
    Report {
        expected: config.packets,
        received: seen.len() as u32,
        delays_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse() {
        assert_eq!("single".parse::<Mode>(), Ok(Mode::Single));
        assert_eq!("per-packet".parse::<Mode>(), Ok(Mode::PerPacket));
        assert_eq!("striped4".parse::<Mode>(), Ok(Mode::Striped(4)));
        assert!("other".parse::<Mode>().is_err());
    }

    #[test]
    fn report_counts_late_and_missing_packets() {
        let report = Report {
            expected: 10,
            received: 8,
            delays_ms: vec![10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 150.0, 300.0],
        };
        assert_eq!(report.late_or_lost(100.0), 0.4);
        assert_eq!(report.percentile(0.0), Some(10.0));
        assert_eq!(report.percentile(1.0), Some(300.0));
    }
}
