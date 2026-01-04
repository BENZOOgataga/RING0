use vt::VtEvent;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ScreenSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ' }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("invalid screen size: cols={cols}, rows={rows}")]
    InvalidSize { cols: u16, rows: u16 },
}

pub struct Screen {
    size: ScreenSize,
    cursor: Cursor,
    cells: Vec<Cell>,
    scrollback: Vec<Vec<Cell>>,
    scroll_offset: usize,
}

impl Screen {
    pub fn new(size: ScreenSize) -> Result<Self, ScreenError> {
        validate_size(size)?;
        let cells = vec![Cell::default(); size.cols as usize * size.rows as usize];
        Ok(Self {
            size,
            cursor: Cursor { col: 0, row: 0 },
            cells,
            scrollback: Vec::new(),
            scroll_offset: 0,
        })
    }

    pub fn size(&self) -> ScreenSize {
        self.size
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.cursor = Cursor { col: 0, row: 0 };
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_view(&mut self, delta: i32) -> bool {
        let max_offset = self.scrollback.len() as i32;
        let current = self.scroll_offset as i32;
        let next = (current + delta).clamp(0, max_offset);
        if next != current {
            self.scroll_offset = next as usize;
            return true;
        }
        false
    }

    pub fn render_chars(&self, out: &mut Vec<char>) {
        out.clear();
        out.reserve(self.cells.len());

        let total_lines = self.scrollback.len() + self.size.rows as usize;
        let rows = self.size.rows as usize;
        let cols = self.size.cols as usize;
        let offset = self.scroll_offset.min(self.scrollback.len());
        let start_line = total_lines.saturating_sub(rows + offset);

        for row in 0..rows {
            let line_index = start_line + row;
            if line_index < self.scrollback.len() {
                let line = &self.scrollback[line_index];
                for cell in line.iter().take(cols) {
                    out.push(cell.ch);
                }
            } else {
                let screen_row = line_index - self.scrollback.len();
                let start = screen_row * cols;
                let end = start + cols;
                for cell in self.cells[start..end].iter() {
                    out.push(cell.ch);
                }
            }
        }
    }

    pub fn is_scrolled(&self) -> bool {
        self.scroll_offset > 0
    }

    pub fn resize(&mut self, size: ScreenSize) -> Result<(), ScreenError> {
        validate_size(size)?;
        let mut new_cells = vec![Cell::default(); size.cols as usize * size.rows as usize];
        let min_cols = self.size.cols.min(size.cols) as usize;
        let min_rows = self.size.rows.min(size.rows) as usize;

        for row in 0..min_rows {
            let old_start = row * self.size.cols as usize;
            let new_start = row * size.cols as usize;
            new_cells[new_start..new_start + min_cols]
                .copy_from_slice(&self.cells[old_start..old_start + min_cols]);
        }

        self.size = size;
        self.cells = new_cells;
        for line in &mut self.scrollback {
            if line.len() < size.cols as usize {
                line.resize(size.cols as usize, Cell::default());
            } else {
                line.truncate(size.cols as usize);
            }
        }
        if self.scroll_offset > self.scrollback.len() {
            self.scroll_offset = self.scrollback.len();
        }
        if self.cursor.col >= size.cols {
            self.cursor.col = size.cols.saturating_sub(1);
        }
        if self.cursor.row >= size.rows {
            self.cursor.row = size.rows.saturating_sub(1);
        }
        Ok(())
    }

    pub fn apply_event(&mut self, event: VtEvent) {
        match event {
            VtEvent::Print(ch) => self.print_char(ch),
            VtEvent::Newline => self.newline(),
            VtEvent::CarriageReturn => self.carriage_return(),
            VtEvent::Backspace => self.backspace(),
        }
    }

    pub fn apply_events(&mut self, events: &[VtEvent]) {
        for event in events {
            self.apply_event(*event);
        }
    }

    fn print_char(&mut self, ch: char) {
        let idx = self.index(self.cursor.col, self.cursor.row);
        if let Some(cell) = self.cells.get_mut(idx) {
            cell.ch = ch;
        }
        self.advance_cursor();
    }

    fn newline(&mut self) {
        self.cursor.row = self.cursor.row.saturating_add(1);
        if self.cursor.row >= self.size.rows {
            self.scroll_up();
            self.cursor.row = self.size.rows.saturating_sub(1);
        }
    }

    fn carriage_return(&mut self) {
        self.cursor.col = 0;
    }

    fn backspace(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            let idx = self.index(self.cursor.col, self.cursor.row);
            if let Some(cell) = self.cells.get_mut(idx) {
                cell.ch = ' ';
            }
        }
    }

    fn advance_cursor(&mut self) {
        self.cursor.col = self.cursor.col.saturating_add(1);
        if self.cursor.col >= self.size.cols {
            self.cursor.col = 0;
            self.newline();
        }
    }

    fn scroll_up(&mut self) {
        let cols = self.size.cols as usize;
        let rows = self.size.rows as usize;
        if rows == 0 || cols == 0 {
            return;
        }

        let top_line = self.cells[0..cols].to_vec();
        self.scrollback.push(top_line);
        if self.scrollback.len() > MAX_SCROLLBACK_LINES {
            self.scrollback.remove(0);
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
            }
        } else if self.scroll_offset > 0 {
            self.scroll_offset = (self.scroll_offset + 1).min(self.scrollback.len());
        }

        for row in 1..rows {
            let src = row * cols;
            let dst = (row - 1) * cols;
            let range = src..src + cols;
            self.cells.copy_within(range, dst);
        }

        let last_row_start = (rows - 1) * cols;
        for cell in &mut self.cells[last_row_start..last_row_start + cols] {
            *cell = Cell::default();
        }
    }

    fn index(&self, col: u16, row: u16) -> usize {
        row as usize * self.size.cols as usize + col as usize
    }
}

const MAX_SCROLLBACK_LINES: usize = 1000;

fn validate_size(size: ScreenSize) -> Result<(), ScreenError> {
    if size.cols == 0 || size.rows == 0 {
        return Err(ScreenError::InvalidSize {
            cols: size.cols,
            rows: size.rows,
        });
    }
    Ok(())
}
