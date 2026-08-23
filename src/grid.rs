use ratatui::prelude::Rect;

/// Lays out a `cols` × `rows` board of near-square cells inside `area`.
/// A terminal cell is about twice as tall as it is wide, so a square is 2×1.
pub struct GridLayout {
    cols: u16,
    rows: u16,
    cell_width: u16,
    cell_height: u16,
    offset_x: u16,
    offset_y: u16,
}

impl GridLayout {
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

        let used_width = cell_width.saturating_mul(cols);
        let used_height = cell_height.saturating_mul(rows);

        let offset_x = area.x + area.width.saturating_sub(used_width) / 2;
        let offset_y = area.y + area.height.saturating_sub(used_height) / 2;

        Self {
            cols,
            rows,
            cell_width,
            cell_height,
            offset_x,
            offset_y,
        }
    }

    pub fn with_border(area: Rect, cols: u16, rows: u16) -> Self {
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        Self::new(inner, cols, rows)
    }

    pub fn used_rect(&self) -> Rect {
        Rect {
            x: self.offset_x,
            y: self.offset_y,
            width: self.cell_width.saturating_mul(self.cols),
            height: self.cell_height.saturating_mul(self.rows),
        }
    }

    pub fn border_rect(&self) -> Rect {
        let used = self.used_rect();
        Rect {
            x: used.x.saturating_sub(1),
            y: used.y.saturating_sub(1),
            width: used.width.saturating_add(2),
            height: used.height.saturating_add(2),
        }
    }

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

    pub fn fits(&self, area: Rect) -> bool {
        let border = self.border_rect();
        border.intersection(area) == border
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn cells_tile_the_used_rect() {
        let grid = GridLayout::new(area(80, 24), 20, 20);
        let used = grid.used_rect();

        assert!(used.width <= 80);
        assert!(used.height <= 24);
        assert_eq!(used.width, grid.cell_width * 20);
        assert_eq!(used.height, grid.cell_height * 20);

        let first = grid.cell_rect(0, 0);
        assert_eq!(first.x, used.x);
        assert_eq!(first.y, used.y);

        let last = grid.cell_rect(19, 19);
        assert_eq!(last.x + last.width, used.x + used.width);
        assert_eq!(last.y + last.height, used.y + used.height);
    }

    #[test]
    fn border_matches_the_playable_grid() {
        let frame = area(80, 24);
        let grid = GridLayout::with_border(frame, 20, 20);
        let used = grid.used_rect();
        let border = grid.border_rect();

        assert_eq!(border.x, used.x - 1);
        assert_eq!(border.y, used.y - 1);
        assert_eq!(border.width, used.width + 2);
        assert_eq!(border.height, used.height + 2);
        assert!(border.x >= frame.x);
        assert!(border.y >= frame.y);
        assert!(border.right() <= frame.right());
        assert!(border.bottom() <= frame.bottom());
        assert!(used.x >= frame.x + 1);
        assert!(used.y >= frame.y + 1);
        assert!(grid.fits(frame));
    }

    #[test]
    fn does_not_fit_a_short_terminal() {
        let frame = area(172, 8);
        let grid = GridLayout::with_border(frame, 20, 20);
        assert!(!grid.fits(frame));
    }
}
