//! Terrain: 16×16-tile chunk prisms with vertex colours (spec 02 B1).
//!
//! Every land tile becomes a box column: top quad at `height ×
//! LEVEL_HEIGHT`, side walls down to the neighbour tile's top (or −0.5 at
//! the coast). Flat shading via per-face normals + hard vertex colours.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use cyclone_engine::InWorld;
use cyclone_worldgen::{Surface, WorldMap};

use crate::srgb_to_vec4;

/// World units per height level (default; the active `WorldConfig`
/// `level_height` is passed through to the builders).
pub const LEVEL_HEIGHT: f32 = 0.5;
const CHUNK: u32 = 16;
const SEA_FLOOR: f32 = -0.5;

#[derive(Default)]
pub struct QuadBuilder {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colors: Vec<Vec4>,
    indices: Vec<u32>,
}

impl QuadBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a CCW quad from an explicit ring and flat normal/colour.
    pub fn quad(&mut self, ring: [Vec3; 4], normal: Vec3, color: Vec4) {
        let base = self.positions.len() as u32;
        for v in ring {
            self.positions.push(v);
            self.normals.push(normal);
            self.colors.push(color);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// Per-(x,y,seed) brightness wobble so flat areas don't look painted (B1).
fn tile_tint(x: u32, y: u32, seed: u64) -> f32 {
    let mut h = (x as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(seed.wrapping_mul(0x1656_67B1_9E37_79F9));
    h ^= h >> 31;
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 29;
    ((h % 61) as f32 / 60.0 - 0.5) * 0.07 + 1.0
}

fn surface_rgb(surface: Surface) -> Option<(f32, f32, f32)> {
    Some(match surface {
        Surface::Sea | Surface::Shallow => return None,
        Surface::Sand => (0.855, 0.769, 0.545),
        Surface::Grass => (0.372, 0.639, 0.310),
        Surface::Rock => (0.518, 0.522, 0.553),
    })
}

const ROCK: (f32, f32, f32) = (0.463, 0.455, 0.470);

pub fn spawn_terrain(
    root: &mut ChildSpawnerCommands,
    map: &WorldMap,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    lh: f32,
) {
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.92,
        ..default()
    });

    let (w, h) = map.size;
    let seed = map.seed.0;

    let mut y0 = 0u32;
    while y0 < h {
        let y1 = (y0 + CHUNK).min(h);
        let mut x0 = 0u32;
        while x0 < w {
            let x1 = (x0 + CHUNK).min(w);
            let mut b = QuadBuilder::new();
            for ty in y0..y1 {
                for tx in x0..x1 {
                    build_tile(&mut b, map, tx, ty, seed, x0, y0, lh);
                }
            }
            if !b.positions.is_empty() {
                root.spawn((
                    InWorld,
                    Mesh3d(meshes.add(b.into_mesh())),
                    MeshMaterial3d(mat.clone()),
                    // Chunk-local vertices; anchor at chunk centre.
                    Transform::from_xyz(
                        x0 as f32 + (x1 - x0) as f32 * 0.5,
                        0.0,
                        y0 as f32 + (y1 - y0) as f32 * 0.5,
                    ),
                ));
            }
            x0 = x1;
        }
        y0 = y1;
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mesh vertex args; splitting adds no clarity"
)]
fn build_tile(
    b: &mut QuadBuilder,
    map: &WorldMap,
    tx: u32,
    ty: u32,
    seed: u64,
    cx: u32,
    cy: u32,
    lh: f32,
) {
    let level = map.height.raw(tx, ty);
    if level == 0 {
        return; // sea — the sea plane covers it
    }
    // Chunk-local coordinates.
    let x = tx - cx;
    let y = ty - cy;
    let (x0, z0, x1, z1) = (x as f32, y as f32, x as f32 + 1.0, y as f32 + 1.0);
    let top = level as f32 * lh;
    let tint = tile_tint(tx, ty, seed);
    let surf = map.surface.raw(tx, ty);
    let Some((r, g, bch)) = surface_rgb(surf) else {
        return; // land tile with sea surface shouldn't happen
    };
    let top_col = srgb_to_vec4(r * tint, g * tint, bch * tint);

    // Top face (CCW seen from +Y): ring with +Y normal.
    b.quad(
        [
            Vec3::new(x0, top, z0),
            Vec3::new(x0, top, z1),
            Vec3::new(x1, top, z1),
            Vec3::new(x1, top, z0),
        ],
        Vec3::Y,
        top_col,
    );

    // Walls toward lower neighbours. Neighbour "height" for sea tiles is 0.
    let cliff = map.cliff.raw(tx, ty);
    for (dir, (nx, ny)) in [
        (0u8, (tx, ty.wrapping_sub(1))), // N (−Z)
        (1, (tx + 1, ty)),               // E (+X)
        (2, (tx, ty + 1)),               // S (+Z)
        (3, (tx.wrapping_sub(1), ty)),   // W (−X)
    ] {
        // Skip wrapped edge lookups (x-1 underflow on column 0 etc.).
        if (dir == 0 && ty == 0)
            || (dir == 3 && tx == 0)
            || (dir == 1 && tx + 1 >= map.size.0)
            || (dir == 2 && ty + 1 >= map.size.1)
        {
            continue;
        }
        let nb = map.height.raw(nx, ny);
        if nb >= level {
            continue;
        }
        let bottom = if nb == 0 { SEA_FLOOR } else { nb as f32 * lh };
        let rocky = cliff & (1 << dir) != 0;
        let col = if rocky {
            srgb_to_vec4(ROCK.0 * tint, ROCK.1 * tint, ROCK.2 * tint)
        } else {
            // Darken the surface colour for exposed banks.
            srgb_to_vec4(r * 0.60, g * 0.60, bch * 0.60)
        };
        let (ring, n) = match dir {
            0 => (
                [
                    Vec3::new(x0, bottom, z0),
                    Vec3::new(x0, top, z0),
                    Vec3::new(x1, top, z0),
                    Vec3::new(x1, bottom, z0),
                ],
                Vec3::new(0.0, 0.0, -1.0),
            ),
            1 => (
                [
                    Vec3::new(x1, bottom, z0),
                    Vec3::new(x1, top, z0),
                    Vec3::new(x1, top, z1),
                    Vec3::new(x1, bottom, z1),
                ],
                Vec3::X,
            ),
            2 => (
                [
                    Vec3::new(x1, bottom, z1),
                    Vec3::new(x1, top, z1),
                    Vec3::new(x0, top, z1),
                    Vec3::new(x0, bottom, z1),
                ],
                Vec3::new(0.0, 0.0, 1.0),
            ),
            _ => (
                [
                    Vec3::new(x0, bottom, z1),
                    Vec3::new(x0, top, z1),
                    Vec3::new(x0, top, z0),
                    Vec3::new(x0, bottom, z0),
                ],
                Vec3::NEG_X,
            ),
        };
        b.quad(ring, n, col);
    }
}
