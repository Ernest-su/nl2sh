/// Explicit marker inserted when bounded text omits content.
pub const TRUNCATION_LABEL: &str = "NL2SH OUTPUT TRUNCATED";

/// Keeps the beginning and end of UTF-8 text within `max_bytes` and inserts an
/// explicit marker so humans and models cannot mistake the result for complete.
pub fn truncate_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let omitted = text.len().saturating_sub(max_bytes);
    let marker = format!("\n[... {TRUNCATION_LABEL}: omitted at least {omitted} bytes; original {} bytes, limit {max_bytes} bytes ...]\n", text.len());
    if max_bytes <= marker.len() {
        return marker.chars().take(max_bytes).collect();
    }
    let available = max_bytes - marker.len();
    let head_budget = available / 2;
    let tail_budget = available - head_budget;
    let head_end = floor_char_boundary(text, head_budget);
    let tail_start = ceil_char_boundary(text, text.len().saturating_sub(tail_budget));
    format!("{}{}{}", &text[..head_end], marker, &text[tail_start..])
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

/// Streaming head/tail buffer with bounded memory.
pub struct BoundedText {
    limit: usize,
    total: usize,
    head: Vec<u8>,
    tail: Vec<u8>,
}

impl BoundedText {
    /// Creates a buffer capped at `limit` bytes.
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            total: 0,
            head: Vec::new(),
            tail: Vec::new(),
        }
    }

    /// Appends raw bytes without allowing retained memory to grow beyond the limit.
    pub fn push(&mut self, bytes: &[u8]) {
        self.total = self.total.saturating_add(bytes.len());
        let head_limit = self.limit / 2;
        let take = head_limit.saturating_sub(self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..take]);
        let tail_limit = self.limit.saturating_sub(head_limit);
        if tail_limit == 0 {
            return;
        }
        self.tail.extend_from_slice(&bytes[take..]);
        if self.tail.len() > tail_limit {
            self.tail.drain(..self.tail.len() - tail_limit);
        }
    }

    /// Returns complete text or an explicitly marked head/tail representation.
    pub fn finish(self) -> String {
        if self.total <= self.limit {
            let mut bytes = self.head;
            bytes.extend_from_slice(&self.tail);
            return String::from_utf8_lossy(&bytes).into_owned();
        }
        let marker = format!(
            "\n[... {TRUNCATION_LABEL}: output omitted; original {} bytes, limit {} bytes ...]\n",
            self.total, self.limit
        );
        if self.limit <= marker.len() {
            return marker.chars().take(self.limit).collect();
        }
        let available = self.limit - marker.len();
        let head = String::from_utf8_lossy(&self.head);
        let tail = String::from_utf8_lossy(&self.tail);
        let head_end = floor_char_boundary(&head, available / 2);
        let tail_start =
            ceil_char_boundary(&tail, tail.len().saturating_sub(available - available / 2));
        format!("{}{}{}", &head[..head_end], marker, &tail[tail_start..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_is_utf8_safe_and_marked() {
        let value = truncate_text(&"界".repeat(100), 120);
        assert!(value.len() <= 120);
        assert!(value.contains(TRUNCATION_LABEL));
    }

    #[test]
    fn streaming_buffer_retains_head_and_tail() {
        let mut value = BoundedText::new(128);
        value.push(b"start-");
        value.push(&vec![b'x'; 500]);
        value.push(b"-end");
        let result = value.finish();
        assert!(result.contains("start-"));
        assert!(result.contains("-end"));
        assert!(result.contains(TRUNCATION_LABEL));
    }
}
