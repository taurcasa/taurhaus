//! Small transcript-boundary helpers shared by task parsing and compact hooks.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use chrono::{DateTime, Utc};

use super::cli_tool::{spec, CliTool};

const COMPACT_HOOK_TRANSCRIPT_TAIL_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionSignalKind {
    Compacted,
    ContextCompacted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSignalBoundary {
    pub timestamp: DateTime<Utc>,
    pub jsonl_offset: u64,
    pub signal_kind: CompactionSignalKind,
}

fn parse_signal_boundary(line: &str, jsonl_offset: u64) -> Option<ParsedSignalBoundary> {
    spec(CliTool::Codex)
        .transcript_parser()?
        .parse_compaction_boundary(line, jsonl_offset)
}

pub fn latest_compaction_timestamp(jsonl_path: &Path) -> Option<DateTime<Utc>> {
    let file = File::open(jsonl_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let start_offset = file_len.saturating_sub(COMPACT_HOOK_TRANSCRIPT_TAIL_BYTES);
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(start_offset.saturating_sub(1)))
        .ok()?;
    if start_offset > 0 {
        let mut partial_line = String::new();
        reader.read_line(&mut partial_line).ok()?;
    }
    let mut latest = None;
    for line in reader.lines().map_while(Result::ok) {
        if let Some(boundary) = parse_signal_boundary(&line, 0) {
            latest = Some(boundary.timestamp);
        }
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn latest_compaction_timestamp_reads_only_the_bounded_transcript_tail() {
        // Regression: 6fe0aa3 parsed every JSONL record on each compact hook,
        // blocking session resume on transcripts hundreds of megabytes long.
        let tmp = tempfile::tempdir().expect("tempdir");
        let transcript = tmp.path().join("rollout.jsonl");
        fs::write(
            &transcript,
            "{\"timestamp\":\"2020-01-01T00:00:00.000Z\",\"type\":\"compacted\",\"payload\":{}}\n",
        )
        .expect("write old boundary");
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .expect("open transcript");
        file.set_len(COMPACT_HOOK_TRANSCRIPT_TAIL_BYTES + 1024)
            .expect("extend sparse transcript");
        use std::io::Write as _;
        file.write_all(b"{}\n").expect("append transcript tail");

        assert_eq!(latest_compaction_timestamp(&transcript), None);
    }
}
