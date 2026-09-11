use crate::classification::classification_label;
use crate::types::ScanResult;
use std::io::Write;

/// Render scan results in a human-readable terminal format with Unicode tables.
pub fn render<W: Write>(result: &ScanResult, writer: &mut W) -> std::io::Result<()> {
    let is_tty = std::io::stdout().is_terminal();

    // ── Header ───────────────────────────────────────────────────────
    writeln!(writer)?;
    write_styled(writer, is_tty, "\x1b[1;36m", "  ⛏  Dowsing Rod")?;
    write_styled(
        writer,
        is_tty,
        "\x1b[0;90m",
        &format!("  v{}", result.tool_version),
    )?;
    writeln!(writer)?;
    writeln!(writer)?;

    // ── Summary ──────────────────────────────────────────────────────
    write_styled(writer, is_tty, "\x1b[1m", "  Repository Summary")?;
    writeln!(writer)?;
    writeln!(
        writer,
        "  ├── Path:            {}",
        result.repository.path.display()
    )?;
    writeln!(
        writer,
        "  ├── Files scanned:   {}",
        result.statistics.files_scanned
    )?;
    writeln!(
        writer,
        "  ├── Functions found: {}",
        result.statistics.functions_found
    )?;
    writeln!(
        writer,
        "  ├── Source tokens:    ~{}",
        format_number(result.statistics.estimated_source_tokens)
    )?;
    writeln!(
        writer,
        "  ├── Candidate pairs: {}",
        result.statistics.candidate_pairs_generated
    )?;
    writeln!(
        writer,
        "  ├── Clusters found:  {}",
        result.statistics.clusters_found
    )?;
    writeln!(
        writer,
        "  ├── High value:      {}",
        result.statistics.high_value_clusters
    )?;
    writeln!(
        writer,
        "  └── Scan time:       {}",
        format_duration(result.statistics.scan_duration_ms)
    )?;
    writeln!(writer)?;

    if result.statistics.cache_hits > 0 || result.statistics.cache_misses > 0 {
        writeln!(
            writer,
            "  Cache: {} hits, {} misses",
            result.statistics.cache_hits, result.statistics.cache_misses
        )?;
        writeln!(writer)?;
    }

    // ── Parse errors ─────────────────────────────────────────────────
    if !result.parse_errors.is_empty() {
        write_styled(writer, is_tty, "\x1b[1;33m", "  ⚠ Parse Errors")?;
        writeln!(writer)?;
        for err in &result.parse_errors {
            writeln!(writer, "    {err}")?;
        }
        writeln!(writer)?;
    }

    // ── No results ───────────────────────────────────────────────────
    if result.clusters.is_empty() {
        write_styled(
            writer,
            is_tty,
            "\x1b[0;32m",
            "  ✓ No refactoring opportunities found above the similarity threshold.",
        )?;
        writeln!(writer)?;
        writeln!(writer)?;
        return Ok(());
    }

    // ── Cluster overview table ───────────────────────────────────────
    write_styled(writer, is_tty, "\x1b[1m", "  Refactoring Opportunities")?;
    writeln!(writer)?;
    writeln!(writer)?;

    // Table header
    writeln!(
        writer,
        "  ┌──────┬──────────────────────────────┬───────┬─────────┬──────────┬────────────┐"
    )?;
    writeln!(
        writer,
        "  │  ID  │ Classification               │ Funcs │ Sim (%) │ Dup Tok  │ Value      │"
    )?;
    writeln!(
        writer,
        "  ├──────┼──────────────────────────────┼───────┼─────────┼──────────┼────────────┤"
    )?;

    for cluster in &result.clusters {
        let label = classification_label(&cluster.classification);
        let label_padded = format!("{:<28}", label);
        let sim_pct = format!("{:.0}", cluster.average_similarity * 100.0);
        let dup_tok = format_number(cluster.duplicated_tokens_estimate);
        let value = format!("{:.0}", cluster.refactoring_value);
        writeln!(
            writer,
            "  │ {:<4} │ {} │ {:>5} │ {:>7} │ {:>8} │ {:>10} │",
            cluster.id,
            label_padded,
            cluster.function_indices.len(),
            sim_pct,
            dup_tok,
            value,
        )?;
    }

    writeln!(
        writer,
        "  └──────┴──────────────────────────────┴───────┴─────────┴──────────┴────────────┘"
    )?;
    writeln!(writer)?;

    // ── Cluster details ──────────────────────────────────────────────
    let base_path = &result.repository.path;

    for cluster in &result.clusters {
        let label = classification_label(&cluster.classification);
        write_styled(
            writer,
            is_tty,
            "\x1b[1;35m",
            &format!("  ── {} ── {}", cluster.id, label),
        )?;
        write_styled(
            writer,
            is_tty,
            "\x1b[0;90m",
            &format!(
                "  (similarity {:.0}%, confidence {:.0}%)",
                cluster.average_similarity * 100.0,
                cluster.confidence * 100.0,
            ),
        )?;
        writeln!(writer)?;

        // Members
        for &idx in &cluster.function_indices {
            if let Some(f) = result.functions.get(idx) {
                writeln!(writer, "    • {}", f.display_location(base_path))?;
            }
        }

        // Common structure
        if !cluster.common_structure.is_empty() {
            writeln!(writer)?;
            write_styled(writer, is_tty, "\x1b[0;36m", "    Common pipeline: ")?;
            writeln!(writer, "{}", cluster.common_structure.join(" → "))?;
        }

        // Differences
        if !cluster.differences.is_empty() {
            write_styled(writer, is_tty, "\x1b[0;33m", "    Differences: ")?;
            writeln!(writer, "{}", cluster.differences.join(", "))?;
        }

        // Signals
        writeln!(writer)?;
        writeln!(
            writer,
            "    Signals: ast={:.2}  tokens={:.2}  calls={:.2}  control={:.2}  complexity={:.2}  params={:.2}",
            cluster.signals.ast,
            cluster.signals.tokens,
            cluster.signals.calls,
            cluster.signals.control_flow,
            cluster.signals.complexity,
            cluster.signals.params,
        )?;

        // Estimates
        writeln!(
            writer,
            "    Duplicated tokens: ~{}  |  Potential reduction: ~{}",
            format_number(cluster.duplicated_tokens_estimate),
            format_number(cluster.potential_reduction_estimate),
        )?;

        // Reason
        writeln!(writer)?;
        write_styled(
            writer,
            is_tty,
            "\x1b[0;90m",
            &format!("    {}", cluster.reason),
        )?;
        writeln!(writer)?;
        writeln!(writer)?;
    }

    Ok(())
}

/// Write with ANSI styling if connected to a TTY.
fn write_styled<W: Write>(
    writer: &mut W,
    is_tty: bool,
    ansi_code: &str,
    text: &str,
) -> std::io::Result<()> {
    if is_tty {
        write!(writer, "{ansi_code}{text}\x1b[0m")
    } else {
        write!(writer, "{text}")
    }
}

/// Format a large number with commas: 12345 → "12,345"
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Format milliseconds into a human-readable duration.
fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let secs = ms / 1_000;
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

/// Needed for `is_terminal()` — available since Rust 1.70.
use std::io::IsTerminal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(1_234_567), "1,234,567");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1_500), "1.5s");
        assert_eq!(format_duration(65_000), "1m 5s");
    }
}
