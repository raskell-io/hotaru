//! The multi-electrode array under the culture: an 8×8 grid at 200 µm pitch
//! without its four corners, the classic 60-electrode layout.

pub const GRID: usize = 8;
pub const PITCH: f32 = 200.0;

/// One recording and stimulation site.
#[derive(Clone, Copy, Debug)]
pub struct Electrode {
    /// Centre in µm, dish centre at the origin.
    pub x: f32,
    pub y: f32,
    /// Column and row, 1-based, as in the usual "col·row" names (12 … 87).
    pub col: u8,
    pub row: u8,
}

pub fn layout() -> Vec<Electrode> {
    let mut out = Vec::with_capacity(GRID * GRID - 4);
    let last = GRID - 1;
    for row in 0..GRID {
        for col in 0..GRID {
            if (row == 0 || row == last) && (col == 0 || col == last) {
                continue;
            }
            let half = last as f32 / 2.0;
            out.push(Electrode {
                x: (col as f32 - half) * PITCH,
                y: (row as f32 - half) * PITCH,
                col: col as u8 + 1,
                row: row as u8 + 1,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_electrodes_centred() {
        let e = layout();
        assert_eq!(e.len(), 60);
        let (sx, sy) = e.iter().fold((0.0, 0.0), |(x, y), e| (x + e.x, y + e.y));
        assert!(sx.abs() < 1e-3 && sy.abs() < 1e-3);
        assert!(!e.iter().any(|e| (e.col, e.row) == (1, 1)));
    }
}
