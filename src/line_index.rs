use tower_lsp::lsp_types::Position;

#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    text_len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = Vec::with_capacity(128);
        line_starts.push(0);
        for (idx, b) in text.as_bytes().iter().enumerate() {
            if *b == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self {
            line_starts,
            text_len: text.len(),
        }
    }

    pub fn offset_of(&self, position: Position) -> Option<usize> {
        let line = usize::try_from(position.line).ok()?;
        let character = usize::try_from(position.character).ok()?;
        let line_start = *self.line_starts.get(line)?;
        let next_line_start = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text_len);
        let line_end = next_line_start.min(self.text_len);
        let offset = line_start.saturating_add(character);
        (offset <= line_end).then_some(offset)
    }

    pub fn position_of(&self, offset: usize) -> Position {
        let clamped = offset.min(self.text_len);
        let line = match self.line_starts.binary_search(&clamped) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts.get(line).copied().unwrap_or(0);
        Position {
            line: line as u32,
            character: (clamped - line_start) as u32,
        }
    }
}
