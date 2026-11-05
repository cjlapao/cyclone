//! World↔screen projection, view flip and depth keys (pure, testable).
//! 2:1 dimetric: `sx = (x - y)`, `sy = (x + y) * 0.5 - z * k` (spec: Rendering).

use crate::tuning::*;

/// Which hemisphere we look from. North views the world as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    North,
    South,
}

impl View {
    pub fn flipped(self) -> View {
        match self {
            View::North => View::South,
            View::South => View::North,
        }
    }
}

/// Apply the view flip to a world position (South rotates 180°).
pub fn flip_xy(x: f32, y: f32, view: View) -> (f32, f32) {
    match view {
        View::North => (x, y),
        View::South => (WORLD_UNITS - x, WORLD_UNITS - y),
    }
}

/// Screen position of a world point relative to a camera centre (which is
/// given in *unflipped* world units; the camera follows the helicopter and is
/// transformed the same way as everything else).
pub fn project(x: f32, y: f32, z: f32, cam: (f32, f32), view: View) -> (f32, f32) {
    let (fx, fy) = flip_xy(x, y, view);
    let (cx, cy) = flip_xy(cam.0, cam.1, view);
    let rx = fx - cx;
    let ry = fy - cy;
    (rx - ry, (rx + ry) * 0.5 - z * HEIGHT_PX)
}

/// Inverse of `project` at a known altitude (z=0 for ground picking/tests).
pub fn unproject(sx: f32, sy: f32, z: f32, cam: (f32, f32), view: View) -> (f32, f32) {
    let sy = sy + z * HEIGHT_PX;
    // sx = rx - ry ; sy = (rx + ry)/2  =>  rx = (sx/2 + sy)/... solve:
    // rx = (sy*2 + sx)/2 ; ry = (sy*2 - sx)/2   (relative coords)
    let rx = (sy * 2.0 + sx) * 0.5;
    let ry = (sy * 2.0 - sx) * 0.5;
    let (cx, cy) = flip_xy(cam.0, cam.1, view);
    let f = (cx + rx, cy + ry);
    match view {
        View::North => f,
        View::South => (WORLD_UNITS - f.0, WORLD_UNITS - f.1),
    }
}

/// Back-to-front key: larger = nearer to camera. Sort by flipped (x+y).
pub fn depth(x: f32, y: f32, view: View) -> f32 {
    let (fx, fy) = flip_xy(x, y, view);
    fx + fy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_both_views() {
        for view in [View::North, View::South] {
            for (x, y, z) in [
                (100.0, 250.0, 0.0),
                (7.5, 900.0, 3.0),
                (1500.0, 1500.0, 8.0),
            ] {
                let cam = (300.0, 800.0);
                let (sx, sy) = project(x, y, z, cam, view);
                let (ux, uy) = unproject(sx, sy, z, cam, view);
                assert!(
                    (ux - x).abs() < 1e-2 && (uy - y).abs() < 1e-2,
                    "view {view:?}"
                );
            }
        }
    }

    #[test]
    fn flip_is_involution() {
        let (a, b) = flip_xy(123.0, 456.0, View::South);
        let (c, d) = flip_xy(a, b, View::South);
        assert!((c - 123.0).abs() < 1e-4 && (d - 456.0).abs() < 1e-4);
    }

    #[test]
    fn south_view_rotates_screen_position() {
        let cam = (WORLD_UNITS * 0.5, WORLD_UNITS * 0.5);
        // West corner (0, W) sits left of centre looking north, right looking south.
        let (sx1, sy1) = project(0.0, WORLD_UNITS, 0.0, cam, View::North);
        let (sx2, sy2) = project(0.0, WORLD_UNITS, 0.0, cam, View::South);
        assert!(sx1 < 0.0 && sx2 > 0.0);
        assert!((sy1 - sy2).abs() < 1e-3);
    }
}
