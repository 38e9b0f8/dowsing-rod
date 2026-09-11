pub mod ai;
pub mod human;
pub mod json;

use crate::types::{OutputFormat, ScanResult};
use std::io::Write;

/// Render a scan result in the specified format.
pub fn render<W: Write>(
    result: &ScanResult,
    format: OutputFormat,
    writer: &mut W,
    max_tokens: Option<usize>,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Human => human::render(result, writer),
        OutputFormat::Ai => ai::render(result, writer, max_tokens),
        OutputFormat::Json => json::render_json(result, writer),
        OutputFormat::Jsonl => json::render_jsonl(result, writer),
    }
}
