use anyhow::anyhow;
use serde::Deserialize;

use super::tools::ChatToolCallDelta;
use crate::server::server_api::AIApiError;

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    delta: ChatDelta,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ChatDelta {
    pub(super) content: Option<String>,
    #[serde(default)]
    pub(super) tool_calls: Vec<ChatToolCallDelta>,
}

#[derive(Debug, Default)]
pub(super) struct SseDataParser {
    pending_line: Vec<u8>,
    pending_data: Vec<String>,
}

impl SseDataParser {
    pub(super) fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<String>, AIApiError> {
        let mut data_events = Vec::new();
        for byte in bytes {
            if *byte == b'\n' {
                if let Some(data) = self.finish_line()? {
                    data_events.push(data);
                }
            } else {
                self.pending_line.push(*byte);
            }
        }
        Ok(data_events)
    }

    fn finish_line(&mut self) -> Result<Option<String>, AIApiError> {
        if self.pending_line.last() == Some(&b'\r') {
            self.pending_line.pop();
        }
        let line_bytes = std::mem::take(&mut self.pending_line);
        let line =
            String::from_utf8(line_bytes).map_err(|error| AIApiError::Other(anyhow!(error)))?;

        if line.is_empty() {
            if self.pending_data.is_empty() {
                return Ok(None);
            }
            return Ok(Some(std::mem::take(&mut self.pending_data).join("\n")));
        }

        if let Some(data) = line.strip_prefix("data:") {
            self.pending_data.push(data.trim_start().to_string());
        }
        Ok(None)
    }
}

#[cfg(test)]
pub(crate) fn content_deltas_from_sse_data(data: &str) -> Result<Vec<String>, AIApiError> {
    Ok(completion_deltas_from_sse_data(data)?
        .into_iter()
        .filter_map(|delta| delta.content)
        .collect())
}

pub(super) fn completion_deltas_from_sse_data(data: &str) -> Result<Vec<ChatDelta>, AIApiError> {
    let chunk: ChatCompletionChunk = serde_json::from_str(data).map_err(|_| {
        AIApiError::Other(anyhow!(
            "Provider returned a malformed Chat Completions stream. Check OpenAI compatibility."
        ))
    })?;
    Ok(chunk
        .choices
        .into_iter()
        .map(|choice| choice.delta)
        .collect())
}
