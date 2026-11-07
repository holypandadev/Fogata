//! Lines typed on standard input.
//!
//! Read on a plain thread rather than with tokio's stdin: a read still waiting when a command
//! ends would keep the process alive until the next line arrives.

use tokio::sync::mpsc;

/// A line typed after `prompt`, trimmed: secrets such as links are read this way rather
/// than taken as arguments, which other users of the computer see in the list of processes
/// and the shell keeps in its history. Empty input is an error.
pub fn secret_line(prompt: &str) -> anyhow::Result<String> {
    println!("{prompt}");
    let mut line = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line)?;
    let line = line.trim();
    if line.is_empty() {
        anyhow::bail!("nothing was typed");
    }
    Ok(line.to_string())
}

/// Lines from standard input; the receiver ends when the input does.
pub fn lines() -> mpsc::UnboundedReceiver<String> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}
