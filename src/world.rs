//! World state: tiles, islands, objects and the physics queries built on
//! them (pure logic — no macroquad).

use crate::iso::View;
use crate::tuning::*;

// ---------------------------------------------------------------- tiny RNG
/// splitmix64 — tiny, fast, reproducible everywhere.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    /// f32 in [0,1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// Deterministic value-noise sample in [0,1) for a tile + seed.
pub fn noise2(tx: i32, ty: i32, seed: u64) -> f32 {
    let mut h = (tx as u64).wrapping_mul(0x9E3779B97F4A7C15)
        ^ (ty as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ seed;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 33;
    h = h.wrapping_mul(0x94D049BB133111EB);
    ((h >> 40) as f32) / (1u64 << 24) as f32
}

// ---------------------------------------------------------------- types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjKind {
    Hut,
    Tower,
    Lighthouse,
    Tree,
}

impl ObjKind {
    pub fn height_units(self) -> u8 {
        match self {
            ObjKind::Hut => 2,
            ObjKind::Tower => 4,
            ObjKind::Lighthouse => 5,
            ObjKind::Tree => 2,
        }
    }
    /// Half-footprint in tiles (for collision/spacing).
    pub fn footprint(self) -> f32 {
        match self {
            ObjKind::Hut => 0.5,
            ObjKind::Tower | ObjKind::Lighthouse => 0.35,
            ObjKind::Tree => 0.25,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Obj {
    /// Centre in world units.
    pub x: f32,
    pub y: f32,
    pub base_h: u8,
    pub kind: ObjKind,
}

impl Obj {
    pub fn top(&self) -> f32 {
        self.base_h as f32 + self.kind.height_units() as f32
    }
}

#[derive(Debug, Clone)]
pub struct Island {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    pub is_base: bool,
    pub helipad: Option<(u32, u32)>,
    /// Land tile coords belonging to this island (spawn candidates).
    pub tiles: Vec<(u32, u32)>,
    /// Number of objects (buildings/trees) standing on this island.
    pub obj_count: usize,
}

#[derive(Debug, Clone)]
pub struct World {
    pub seed: u64,
    pub h: Vec<u8>, // WORLD_TILES², 0 = sea, 1..=6 land
    pub islands: Vec<Island>,
    pub objects: Vec<Obj>,
}

impl World {
    pub fn tile_h(&self, tx: u32, ty: u32) -> u8 {
        if tx >= WORLD_TILES || ty >= WORLD_TILES {
            0
        } else {
            self.h[(ty * WORLD_TILES + tx) as usize]
        }
    }
    pub fn set_tile_h(&mut self, tx: u32, ty: u32, h: u8) {
        self.h[(ty * WORLD_TILES + tx) as usize] = h;
    }
    /// Height of the tile containing a world-space point.
    pub fn h_at(&self, x: f32, y: f32) -> u8 {
        let tx = (x / TILE) as i32;
        let ty = (y / TILE) as i32;
        if tx < 0 || ty < 0 {
            return 0;
        }
        self.tile_h(tx as u32, ty as u32)
    }
    pub fn tile_of(x: f32) -> u32 {
        (x / TILE).clamp(0.0, WORLD_TILES as f32 - 0.001) as u32
    }
}

// ---------------------------------------------------------------- queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Sea,
    Sand,
    Grass,
    Rock,
}

impl World {
    /// Visual classification: sand hugs the sea, rock crowns the hills.
    pub fn kind(&self, tx: u32, ty: u32) -> TileKind {
        let h = self.tile_h(tx, ty);
        if h == 0 {
            return TileKind::Sea;
        }
        let touches_sea = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(dx, dy)| {
                let nx = tx as i32 + dx;
                let ny = ty as i32 + dy;
                nx < 0 || ny < 0 || self.tile_h(nx as u32, ny as u32) == 0
            });
        if touches_sea && h <= 2 {
            TileKind::Sand
        } else if h >= 5 {
            TileKind::Rock
        } else {
            TileKind::Grass
        }
    }

    /// Flat, object-free land of equal height in a 3×3 neighbourhood.
    /// Landing here at low speed is safe; anything else crashes.
    pub fn landing_ok(&self, x: f32, y: f32) -> bool {
        let tx = World::tile_of(x);
        let ty = World::tile_of(y);
        let h = self.tile_h(tx, ty);
        if h == 0 {
            return false;
        }
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let nx = tx as i32 + dx;
                let ny = ty as i32 + dy;
                if nx < 0 || ny < 0 {
                    return false;
                }
                if self.tile_h(nx as u32, ny as u32) != h {
                    return false; // slope
                }
            }
        }
        // objects under the helicopter's own footprint (centre tile) make the
        // spot rough: landing on a building or tree crashes, but a tree one
        // tile away is fine.
        for o in &self.objects {
            let ox = o.x / TILE;
            let oy = o.y / TILE;
            let r = 0.5 + o.kind.footprint();
            if (ox - (tx as f32 + 0.5)).abs() < r && (oy - (ty as f32 + 0.5)).abs() < r {
                return false;
            }
        }
        true
    }

    pub fn on_helipad(&self, x: f32, y: f32) -> bool {
        self.on_helipad_tile(World::tile_of(x), World::tile_of(y))
    }

    pub fn on_helipad_tile(&self, tx: u32, ty: u32) -> bool {
        self.islands.iter().any(|i| match i.helipad {
            Some((px, py)) => {
                (px as i32 - tx as i32).abs() <= 1 && (py as i32 - ty as i32).abs() <= 1
            }
            None => false,
        })
    }

    /// Flight collision: true if point (x,y) at altitude z is inside an
    /// object's footprint and below its top.
    pub fn hits_object(&self, x: f32, y: f32, z: f32) -> bool {
        self.objects.iter().any(|o| {
            let r = (o.kind.footprint() + 0.2) * TILE;
            (o.x - x).abs() < r && (o.y - y).abs() < r && z < o.top()
        })
    }
}

/// Does any object hide the ground spot (x,y) of ground height `h` from the

// ---------------------------------------------------------------- round items
#[derive(Debug, Clone, Copy)]
pub struct Crate {
    pub x: f32,
    pub y: f32,
    pub island: usize,
    /// Which view direction cannot see it (the side it is hidden behind).
    pub hidden_by: Option<View>,
}

#[derive(Debug, Clone, Copy)]
pub struct Refugee {
    pub x: f32,
    pub y: f32,
}

/// Does any object hide the ground spot (x,y) of ground height `h` from the
/// given view? Iso camera axis: north occludes toward +x+y, south toward −x−y.
pub fn blocking_obj(w: &World, x: f32, y: f32, h: u8, view: View) -> Option<&Obj> {
    let sgn = match view {
        View::North => 1.0,
        View::South => -1.0,
    };
    w.objects.iter().find(|o| {
        if o.kind == ObjKind::Tree && o.top() < h as f32 + 1.0 {
            return false; // small trees don't hide crates
        }
        let dx = o.x - x;
        let dy = o.y - y;
        let adx = dx * sgn;
        let ady = dy * sgn;
        adx >= -0.1 && ady >= -0.1 && (adx + ady) > 0.1 && (dx - dy).abs() <= TILE
    })
}

pub fn near_object(w: &World, x: f32, y: f32, tiles: f32) -> bool {
    w.objects
        .iter()
        .any(|o| (o.x - x).abs() + (o.y - y).abs() < tiles * TILE)
}

pub(crate) fn flat_3x3(w: &World, tx: u32, ty: u32, h: u8) -> bool {
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = tx as i32 + dx;
            let ny = ty as i32 + dy;
            if nx < 0 || ny < 0 {
                return false;
            }
            if w.tile_h(nx as u32, ny as u32) != h {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{gen_world, place_round};

    #[test]
    fn helipads_are_landable() {
        let w = gen_world(1984);
        let mut n = 0;
        for isl in &w.islands {
            if let Some((px, py)) = isl.helipad {
                let x = (px as f32 + 0.5) * TILE;
                let y = (py as f32 + 0.5) * TILE;
                assert!(w.landing_ok(x, y), "pad on island must be landable");
                assert!(w.on_helipad(x, y));
                n += 1;
            }
        }
        assert!(n >= 4, "base + 3 extra pads");
    }

    #[test]
    fn sea_and_objects_reject_landing() {
        let w = gen_world(42);
        assert!(!w.landing_ok(2.0, 2.0), "open sea corner");
        let o = w
            .objects
            .iter()
            .find(|o| o.kind != ObjKind::Tree && w.h_at(o.x, o.y) > 0)
            .unwrap();
        assert!(!w.landing_ok(o.x, o.y), "cannot land on a building");
    }

    #[test]
    fn object_collision_altitude() {
        let w = gen_world(1984);
        let o = w.objects.iter().find(|o| o.kind != ObjKind::Tree).unwrap();
        assert!(w.hits_object(o.x, o.y, o.top() - 0.5));
        assert!(!w.hits_object(o.x, o.y, o.top() + 0.5));
    }

    #[test]
    fn hidden_crates_hide_one_side_only() {
        for seed in [7u64, 42, 1984] {
            let w = gen_world(seed);
            let (crates, refs) = place_round(&w, seed, 1);
            assert_eq!(crates.len(), NUM_CRATES as usize);
            assert!(!crates.iter().any(|c| w.islands[c.island].is_base));
            let uniq: std::collections::HashSet<usize> = crates.iter().map(|c| c.island).collect();
            assert_eq!(uniq.len(), NUM_CRATES as usize);
            let hidden = crates.iter().filter(|c| c.hidden_by.is_some()).count();
            assert!(hidden >= MIN_HIDDEN_CRATES as usize, "seed {seed}");
            assert!(!refs.is_empty());
            for c in crates.iter().filter(|c| c.hidden_by.is_some()) {
                let h = w.h_at(c.x, c.y);
                let v = c.hidden_by.unwrap();
                assert!(blocking_obj(&w, c.x, c.y, h, v).is_some(), "must hide");
                assert!(
                    blocking_obj(&w, c.x, c.y, h, v.flipped()).is_none(),
                    "must show"
                );
            }
        }
    }
}
