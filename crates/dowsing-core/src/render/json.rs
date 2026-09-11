use crate::types::ScanResult;
use std::io::Write;

/// Render scan results as pretty-printed JSON.
pub fn render_json<W: Write>(result: &ScanResult, writer: &mut W) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(result).map_err(std::io::Error::other)?;
    writeln!(writer, "{json}")
}

/// Render scan results as JSON Lines (one JSON object per line).
///
/// First line: header object with metadata and statistics.
/// Subsequent lines: one line per cluster.
pub fn render_jsonl<W: Write>(result: &ScanResult, writer: &mut W) -> std::io::Result<()> {
    // Header line
    let header = serde_json::json!({
        "type": "header",
        "schema_version": result.schema_version,
        "tool_version": result.tool_version,
        "repository": result.repository,
        "statistics": result.statistics,
        "parse_errors": result.parse_errors,
    });
    let header_json = serde_json::to_string(&header).map_err(std::io::Error::other)?;
    writeln!(writer, "{header_json}")?;

    // One line per cluster
    for cluster in &result.clusters {
        // Include referenced function info inline for self-contained lines
        let member_functions: Vec<_> = cluster
            .function_indices
            .iter()
            .filter_map(|&idx| result.functions.get(idx))
            .collect();

        let cluster_line = serde_json::json!({
            "type": "cluster",
            "cluster": cluster,
            "functions": member_functions,
        });
        let line_json = serde_json::to_string(&cluster_line).map_err(std::io::Error::other)?;
        writeln!(writer, "{line_json}")?;
    }

    Ok(())
}
