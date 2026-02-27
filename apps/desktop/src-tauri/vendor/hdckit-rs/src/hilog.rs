use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::{mpsc, oneshot};

use crate::connection::Connection;
use crate::error::HdcError;

static REG_ENTRY_START: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d{10}\.\d{3}\s+\d+\s+\d+").expect("valid entry start regex"));
static REG_ENTRY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{10}\.\d{3})\s+(\d+)\s+(\d+)\s+([DIWEF])\s+([AICKP])(.{5})/([^:]*):(.*)")
        .expect("valid entry regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HilogEntry {
    pub date: SystemTime,
    pub pid: i64,
    pub tid: i64,
    pub level: i32,
    pub kind: i32,
    pub domain: String,
    pub tag: String,
    pub message: String,
}

#[derive(Debug)]
pub struct HilogStream {
    receiver: mpsc::UnboundedReceiver<Result<HilogEntry, HdcError>>,
    stop_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl HilogStream {
    pub(crate) fn new(mut connection: Connection) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = oneshot::channel();

        tokio::spawn(async move {
            let mut parser = HilogParser::default();

            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        break;
                    }
                    result = connection.read_value() => {
                        match result {
                            Ok(buf) => {
                                for entry in parser.parse_chunk(&buf) {
                                    let _ = tx.send(Ok(entry));
                                }
                            }
                            Err(err) => {
                                let _ = tx.send(Err(err));
                                break;
                            }
                        }
                    }
                }
            }

            let _ = connection.end().await;
        });

        Self {
            receiver: rx,
            stop_sender: Arc::new(Mutex::new(Some(stop_tx))),
        }
    }

    pub async fn next_entry(&mut self) -> Option<Result<HilogEntry, HdcError>> {
        self.receiver.recv().await
    }

    pub fn end(&self) {
        if let Ok(mut sender) = self.stop_sender.lock() {
            if let Some(stop_tx) = sender.take() {
                let _ = stop_tx.send(());
            }
        }
    }
}

impl Drop for HilogStream {
    fn drop(&mut self) {
        self.end();
    }
}

#[derive(Debug, Default)]
struct HilogParser {
    data: String,
}

impl HilogParser {
    fn parse_chunk(&mut self, buf: &[u8]) -> Vec<HilogEntry> {
        self.data.push_str(&String::from_utf8_lossy(buf));

        let starts: Vec<usize> = REG_ENTRY_START
            .find_iter(&self.data)
            .map(|m| m.start())
            .collect();

        let mut entries = Vec::new();

        if starts.len() >= 2 {
            for window in starts.windows(2) {
                let raw_entry = &self.data[window[0]..window[1]];
                if let Some(entry) = parse_entry(raw_entry) {
                    entries.push(entry);
                }
            }
        }

        if let Some(last_start) = starts.last() {
            self.data = self.data[*last_start..].to_string();
        } else {
            self.data.clear();
        }

        entries
    }
}

fn parse_entry(raw_entry: &str) -> Option<HilogEntry> {
    let captures = REG_ENTRY.captures(raw_entry)?;

    let date_seconds = captures.get(1)?.as_str().parse::<f64>().ok()?;
    let pid = captures.get(2)?.as_str().parse::<i64>().ok()?;
    let tid = captures.get(3)?.as_str().parse::<i64>().ok()?;
    let level_letter = captures.get(4)?.as_str().chars().next()?;
    let kind_letter = captures.get(5)?.as_str().chars().next()?;

    Some(HilogEntry {
        date: UNIX_EPOCH + Duration::from_millis((date_seconds * 1000.0) as u64),
        pid,
        tid,
        level: to_level(level_letter),
        kind: to_kind(kind_letter),
        domain: captures.get(6)?.as_str().to_string(),
        tag: captures.get(7)?.as_str().to_string(),
        message: captures.get(8)?.as_str().trim().to_string(),
    })
}

fn to_level(letter: char) -> i32 {
    ['?', '?', 'V', 'D', 'I', 'W', 'E', 'F']
        .iter()
        .position(|value| *value == letter)
        .map(|idx| idx as i32)
        .unwrap_or(-1)
}

fn to_kind(letter: char) -> i32 {
    ['A', 'I', 'C', 'K', 'P']
        .iter()
        .position(|value| *value == letter)
        .map(|idx| idx as i32)
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::{parse_entry, HilogParser};

    #[test]
    fn parse_single_entry() {
        let raw = "1715334966.123 123 456 I A00001/TAG: hello world\n";
        let entry = parse_entry(raw).unwrap();

        assert_eq!(entry.pid, 123);
        assert_eq!(entry.tid, 456);
        assert_eq!(entry.level, 4);
        assert_eq!(entry.kind, 0);
        assert_eq!(entry.domain, "00001");
        assert_eq!(entry.tag, "TAG");
        assert_eq!(entry.message, "hello world");
    }

    #[test]
    fn parse_streamed_entries() {
        let mut parser = HilogParser::default();
        let out1 = parser.parse_chunk(
            b"1715334966.123 123 456 I A00001/TAG: hello\n1715334967.456 234 567 W I00002/NEXT: world\n",
        );
        // One complete entry is emitted; the trailing start remains buffered.
        assert_eq!(out1.len(), 1);

        let out2 = parser.parse_chunk(b"1715334968.111 345 678 E C00003/THIRD: done\n");
        assert_eq!(out2.len(), 1);
        assert_eq!(out2[0].tag, "NEXT");
    }
}
