use crate::types::{
    format_bytes, format_duration, format_throughput, BackendName, BenchmarkResult, Operation,
};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use std::collections::BTreeMap;

/// Group results by scenario, then by backend.
fn group_results(
    results: &[BenchmarkResult],
) -> BTreeMap<String, BTreeMap<BackendName, &BenchmarkResult>> {
    let mut map: BTreeMap<String, BTreeMap<BackendName, &BenchmarkResult>> = BTreeMap::new();
    for r in results {
        let key = scenario_key(r.operation, r.file_size_bytes);
        map.entry(key).or_default().insert(r.backend, r);
    }
    map
}

/// Build a stable, sortable string key from an operation and file size.
fn scenario_key(op: Operation, file_size: u64) -> String {
    format!("{op}|{file_size}")
}

/// Parse a scenario key back into its components.
fn parse_scenario_key(key: &str) -> (String, String) {
    let parts: Vec<&str> = key.splitn(2, '|').collect();
    let op = parts[0].to_string();
    let size: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let size_str = if size == 0 {
        "N/A".to_string()
    } else {
        format_bytes(size)
    };
    (op, size_str)
}

/// Formatted statistics for a single backend in a scenario row.
struct FormattedStats {
    median: String,
    stddev: String,
    throughput: String,
}

/// Extract formatted stats from an optional benchmark result.
fn format_backend_stats(result: Option<&&BenchmarkResult>) -> FormattedStats {
    let dash = || "\u{2014}".to_string(); // em-dash
    FormattedStats {
        median: result
            .map(|r| format_duration(r.median_duration()))
            .unwrap_or_else(dash),
        stddev: result
            .map(|r| format_duration(r.std_dev()))
            .unwrap_or_else(dash),
        throughput: result
            .and_then(|r| r.throughput_bytes_per_sec())
            .map(format_throughput)
            .unwrap_or_else(dash),
    }
}

/// Compute the speedup ratio (libmtp_median / mtp_rs_median).
/// Returns ">1.00x" when mtp-rs is faster, "<1.00x" when libmtp is faster.
fn compute_speedup(mtp_rs: Option<&&BenchmarkResult>, libmtp: Option<&&BenchmarkResult>) -> String {
    match (mtp_rs, libmtp) {
        (Some(m), Some(l)) => {
            let m_secs = m.median_duration().as_secs_f64();
            let l_secs = l.median_duration().as_secs_f64();
            if m_secs > 0.0 {
                format!("{:.2}x", l_secs / m_secs)
            } else {
                "N/A".to_string()
            }
        }
        _ => "N/A".to_string(),
    }
}

/// Render results as a UTF-8 terminal table for display.
///
/// Shows one row per scenario (operation + file size) with columns for each backend's
/// median, std dev, and throughput.
pub fn render_terminal_table(results: &[BenchmarkResult]) -> String {
    let grouped = group_results(results);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Operation"),
            Cell::new("File Size"),
            Cell::new("mtp-rs Median"),
            Cell::new("mtp-rs Std Dev"),
            Cell::new("mtp-rs Throughput"),
            Cell::new("libmtp Median"),
            Cell::new("libmtp Std Dev"),
            Cell::new("libmtp Throughput"),
            Cell::new("Speedup"),
        ]);

    for (key, backends) in &grouped {
        let (op, size_str) = parse_scenario_key(key);

        let mtp_rs = backends.get(&BackendName::MtpRs);
        let libmtp = backends.get(&BackendName::Libmtp);
        let rs = format_backend_stats(mtp_rs);
        let lm = format_backend_stats(libmtp);
        let speedup = compute_speedup(mtp_rs, libmtp);

        table.add_row(vec![
            Cell::new(&op),
            Cell::new(&size_str),
            Cell::new(&rs.median),
            Cell::new(&rs.stddev),
            Cell::new(&rs.throughput),
            Cell::new(&lm.median),
            Cell::new(&lm.stddev),
            Cell::new(&lm.throughput),
            Cell::new(&speedup),
        ]);
    }

    table.to_string()
}

/// Render results as a Markdown table suitable for pasting into GitHub issues or READMEs.
pub fn render_markdown_table(results: &[BenchmarkResult]) -> String {
    let grouped = group_results(results);

    let mut lines = Vec::new();
    lines.push(
        "| Operation | File Size | mtp-rs Median | mtp-rs Std Dev | mtp-rs Throughput | libmtp Median | libmtp Std Dev | libmtp Throughput | Speedup |"
            .to_string(),
    );
    lines.push(
        "|-----------|-----------|---------------|----------------|-------------------|---------------|----------------|-------------------|---------|"
            .to_string(),
    );

    for (key, backends) in &grouped {
        let (op, size_str) = parse_scenario_key(key);

        let mtp_rs = backends.get(&BackendName::MtpRs);
        let libmtp = backends.get(&BackendName::Libmtp);
        let rs = format_backend_stats(mtp_rs);
        let lm = format_backend_stats(libmtp);
        let speedup = compute_speedup(mtp_rs, libmtp);

        lines.push(format!(
            "| {op} | {size_str} | {} | {} | {} | {} | {} | {} | {speedup} |",
            rs.median, rs.stddev, rs.throughput, lm.median, lm.stddev, lm.throughput,
        ));
    }

    lines.join("\n")
}

/// Render results as CSV text.
///
/// Columns: operation, file_size_bytes, file_size_human, backend, median_secs, stddev_secs,
/// throughput_bytes_per_sec
pub fn render_csv(results: &[BenchmarkResult]) -> String {
    let mut lines = Vec::new();
    lines.push(
        "operation,file_size_bytes,file_size_human,backend,runs,median_secs,stddev_secs,throughput_bytes_per_sec"
            .to_string(),
    );

    for r in results {
        let tp = r
            .throughput_bytes_per_sec()
            .map(|v| format!("{:.2}", v))
            .unwrap_or_default();

        lines.push(format!(
            "{},{},{},{},{},{:.6},{:.6},{}",
            r.operation,
            r.file_size_bytes,
            format_bytes(r.file_size_bytes),
            r.backend,
            r.durations.len(),
            r.median_duration().as_secs_f64(),
            r.std_dev().as_secs_f64(),
            tp,
        ));
    }

    lines.join("\n")
}

/// Render results as a JSON string.
pub fn render_json(results: &[BenchmarkResult]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Operation;
    use std::time::Duration;

    fn sample_results() -> Vec<BenchmarkResult> {
        vec![
            BenchmarkResult {
                backend: BackendName::MtpRs,
                operation: Operation::Download,
                file_size_bytes: 10_000_000,
                durations: vec![
                    Duration::from_millis(500),
                    Duration::from_millis(520),
                    Duration::from_millis(480),
                ],
            },
            BenchmarkResult {
                backend: BackendName::Libmtp,
                operation: Operation::Download,
                file_size_bytes: 10_000_000,
                durations: vec![
                    Duration::from_millis(800),
                    Duration::from_millis(850),
                    Duration::from_millis(780),
                ],
            },
            BenchmarkResult {
                backend: BackendName::MtpRs,
                operation: Operation::ListFiles,
                file_size_bytes: 0,
                durations: vec![
                    Duration::from_millis(50),
                    Duration::from_millis(55),
                    Duration::from_millis(48),
                ],
            },
        ]
    }

    #[test]
    fn test_csv_output() {
        let csv = render_csv(&sample_results());
        let lines: Vec<&str> = csv.lines().collect();
        // Header + 3 data rows
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("operation,"));
        assert!(lines[1].contains("mtp-rs"));
        assert!(lines[2].contains("libmtp"));
    }

    #[test]
    fn test_markdown_output() {
        let md = render_markdown_table(&sample_results());
        assert!(md.contains("| Operation |"));
        assert!(md.contains("|--------"));
        assert!(md.contains("download"));
    }

    #[test]
    fn test_terminal_table_output() {
        let table = render_terminal_table(&sample_results());
        // Should contain header text
        assert!(table.contains("Operation"));
        assert!(table.contains("mtp-rs Median"));
    }

    #[test]
    fn test_json_output() {
        let json = render_json(&sample_results()).unwrap();
        let parsed: Vec<BenchmarkResult> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
    }
}
