use noise::{Fbm, NoiseFn, Perlin};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{CHUNK_SIZE, Chunk, Voxel};

// --- Constants ---

pub const SEED_WORD: &str = "maujart";

/// Horizontal scale of the height noise (larger = broader hills).
const HEIGHT_NOISE_SCALE: f64 = 0.03;
/// Amplitude of height variation in voxels around the base height.
const HEIGHT_AMPLITUDE: f64 = 4.0;
/// Base terrain height in voxels.
const HEIGHT_BASE: f64 = 4.0;
/// Scale of the low-frequency continental noise (smaller = broader regions).
const CONTINENT_NOISE_SCALE: f64 = 0.012;
/// How many voxels the cliff raises the terrain.
const CLIFF_HEIGHT: f64 = 10.0;
/// Depth of soil layer above stone.
const SOIL_DEPTH: usize = 2;
/// Scale of the 3D cave noise (larger = bigger caves).
const CAVE_NOISE_SCALE: f64 = 0.06;
/// Threshold above which a voxel is carved into a cave (range -1..1).
const CAVE_THRESHOLD: f64 = 0.3;
/// Caves only carve below this Y level (world space).
const CAVE_MAX_Y: i32 = 15;

// --- Seed ---

fn word_to_seed(word: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    word.hash(&mut hasher);
    hasher.finish() as u32
}

// --- Math helpers ---

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// --- Terrain generation ---

/// Generate a chunk at `chunk_origin` (world-space voxel position of its (0,0,0) corner).
pub fn generate_chunk(chunk_origin: [i32; 3]) -> Chunk {
    let seed = word_to_seed(SEED_WORD);
    let height_noise: Fbm<Perlin> = Fbm::<Perlin>::new(seed);
    let continent_noise: Fbm<Perlin> = Fbm::<Perlin>::new(seed.wrapping_add(2));
    let cave_noise: Fbm<Perlin> = Fbm::<Perlin>::new(seed.wrapping_add(1));

    let mut chunk = Chunk::empty();

    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let world_x = chunk_origin[0] + x as i32;
            let world_z = chunk_origin[2] + z as i32;

            // 2D height map
            let nx = world_x as f64 * HEIGHT_NOISE_SCALE;
            let nz = world_z as f64 * HEIGHT_NOISE_SCALE;
            let raw = height_noise.get([nx, nz]); // -1..1

            // Continental noise: sharp transition between lowlands and highlands
            let cnx = world_x as f64 * CONTINENT_NOISE_SCALE;
            let cnz = world_z as f64 * CONTINENT_NOISE_SCALE;
            let continent = continent_noise.get([cnx, cnz]); // -1..1
            let cliff = smoothstep(-0.15, 0.15, continent) * CLIFF_HEIGHT;

            let surface_y = (HEIGHT_BASE + cliff + raw * HEIGHT_AMPLITUDE).round() as i32;

            for y in 0..CHUNK_SIZE {
                let world_y = chunk_origin[1] + y as i32;

                if world_y > surface_y {
                    continue; // above surface → Air
                }

                // Cave carving (only underground)
                if world_y < surface_y && world_y <= CAVE_MAX_Y {
                    let cx = world_x as f64 * CAVE_NOISE_SCALE;
                    let cy = world_y as f64 * CAVE_NOISE_SCALE;
                    let cz = world_z as f64 * CAVE_NOISE_SCALE;
                    if cave_noise.get([cx, cy, cz]) > CAVE_THRESHOLD {
                        continue; // carved out → Air
                    }
                }

                let depth = surface_y - world_y;
                let voxel = if depth < SOIL_DEPTH as i32 {
                    Voxel::Soil
                } else {
                    Voxel::Stone
                };
                chunk.set(x, y, z, voxel);
            }
        }
    }

    chunk
}
