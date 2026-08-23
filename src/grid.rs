use ratatui::prelude::Rect;

/// Layout helper for grid-based games.
/// Computes how large each cell should be to fit a given grid
/// inside the available terminal area.
///
/// Terminal characters are roughly half as wide as they are tall,
/// so a visually square cell is approximately 2 columns x 1 row.
#[allow(dead_code)]
pub struct GridLayout {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
    pub offset_x: u16,
    pub offset_y: u16,
}

impl GridLayout {
    /// Fit a `cols x rows` grid into `area`.
    /// Each cell is sized to look as close to a square as possible
    /// given the terminal's character aspect ratio.
    pub fn new(area: Rect, cols: u16, rows: u16) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);

        let max_cell_width = area.width / cols;
        let max_cell_height = area.height / rows;

        let (cell_width, cell_height) = if max_cell_width == 0 || max_cell_height == 0 {
            (1, 1)
        } else {
            let mut cell_height = max_cell_height;
            let mut cell_width = cell_height * 2;

            if cell_width > max_cell_width {
                cell_width = max_cell_width;
                cell_height = cell_width / 2;
                if cell_height == 0 {
                    cell_height = 1;
                }
            }

            if cell_height > max_cell_height {
                cell_height = max_cell_height;
                cell_width = cell_height * 2;
            }

            (cell_width, cell_height)
        };

        let used_width = cell_width * cols;
        let used_height = cell_height * rows;

        let offset_x = area.x + (area.width - used_width) / 2;
        let offset_y = area.y + (area.height - used_height) / 2;

        Self {
            cols,
            rows,
            cell_width,
            cell_height,
            offset_x,
            offset_y,
        }
    }

    /// Return the terminal `Rect` occupied by cell `(col, row)`.
    pub fn cell_rect(&self, col: u16, row: u16) -> Rect {
        let x = self.offset_x + col * self.cell_width;
        let y = self.offset_y + row * self.cell_height;
        Rect {
            x,
            y,
            width: self.cell_width,
            height: self.cell_height,
        }
    }
}
