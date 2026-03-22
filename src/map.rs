use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::events::{Click, Pointer};
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
pub mod clip_plane;
mod genesis;
mod voxel_material;

pub const CHUNK_SIZE: usize = 16;

// --- Clip plane ---

// --- Voxel ---

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Voxel {
    #[default]
    Air = 0,
    Soil = 1,
    Stone = 2,
}

// --- Chunk data ---

#[derive(Component, Clone)]
pub struct Chunk {
    pub voxels: Box<[Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            voxels: Box::new([Voxel::Air; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]),
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[x + z * CHUNK_SIZE + y * CHUNK_SIZE * CHUNK_SIZE]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        self.voxels[x + z * CHUNK_SIZE + y * CHUNK_SIZE * CHUNK_SIZE] = voxel;
    }

    fn neighbor_is_air(&self, x: usize, y: usize, z: usize, dx: i32, dy: i32, dz: i32) -> bool {
        let (nx, ny, nz) = (x as i32 + dx, y as i32 + dy, z as i32 + dz);
        let s = CHUNK_SIZE as i32;
        if nx < 0 || ny < 0 || nz < 0 || nx >= s || ny >= s || nz >= s {
            return true;
        }
        self.get(nx as usize, ny as usize, nz as usize) == Voxel::Air
    }
}

// --- Chunk coordinate ---

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkCoord(pub IVec3);

impl ChunkCoord {
    pub fn world_origin(&self) -> Vec3 {
        self.0.as_vec3() * CHUNK_SIZE as f32
    }
}

// --- Dirty marker ---

/// When added to a chunk entity, its mesh will be rebuilt next frame.
#[derive(Component)]
pub struct ChunkDirty;

// --- Cap entity ---

// --- Public edit API ---

/// Modify a single voxel and mark the chunk for mesh rebuild.
pub fn edit_voxel(
    commands: &mut Commands,
    entity: Entity,
    chunk: &mut Chunk,
    x: usize,
    y: usize,
    z: usize,
    voxel: Voxel,
) {
    chunk.set(x, y, z, voxel);
    commands.entity(entity).insert(ChunkDirty);
}

// --- Plugin ---

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MeshPickingPlugin,
            MaterialPlugin::<voxel_material::VoxelMaterial>::default(),
            clip_plane::ClipPlanePlugin,
        ))
        .add_systems(Startup, (genesis::build_map, spawn_light))
        .add_systems(Update, rebuild_dirty_chunks);
    }
}

fn spawn_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.4, 0.0)),
    ));
}

// --- Sync clip plane to shader ---

// --- Rebuild dirty chunks ---

// todo: move to chunk
fn rebuild_dirty_chunks(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &ChunkCoord,
            &Chunk,
            &mut Mesh3d,
            Option<&clip_plane::ChunkCapEntity>,
        ),
        With<ChunkDirty>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    clip: Res<clip_plane::ClipPlane>,
    mut cap_query: Query<&mut Mesh3d, Without<Chunk>>,
) {
    for (entity, coord, chunk, mut mesh3d, cap_entity) in &mut query {
        let origin = coord.world_origin();
        mesh3d.0 = meshes.add(build_chunk_mesh(chunk, origin));
        if let Some(clip_plane::ChunkCapEntity(cap)) = cap_entity {
            if let Ok(mut cap_mesh3d) = cap_query.get_mut(*cap) {
                cap_mesh3d.0 = meshes.add(clip_plane::build_cap_mesh(chunk, origin, clip.y));
            }
        }
        commands.entity(entity).remove::<ChunkDirty>();
    }
}

// --- Mesh building ---

// move to render
/// Build a single mesh for an entire chunk. Only exposed faces are emitted.
fn build_chunk_mesh(chunk: &Chunk, origin: Vec3) -> Mesh {
    // Each entry: (neighbour direction, face normal, 4 CCW corner offsets)
    const FACES: [([i32; 3], [f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0, 1, 0],
            [0., 1., 0.],
            [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
        ), // +Y
        (
            [0, -1, 0],
            [0., -1., 0.],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        ), // -Y
        (
            [1, 0, 0],
            [1., 0., 0.],
            [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
        ), // +X
        (
            [-1, 0, 0],
            [-1., 0., 0.],
            [[0., 0., 0.], [0., 1., 0.], [0., 1., 1.], [0., 0., 1.]],
        ), // -X
        (
            [0, 0, 1],
            [0., 0., 1.],
            [[1., 0., 1.], [0., 0., 1.], [0., 1., 1.], [1., 1., 1.]],
        ), // +Z
        (
            [0, 0, -1],
            [0., 0., -1.],
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        ), // -Z
    ];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let voxel = chunk.get(x, y, z);
                if voxel == Voxel::Air {
                    continue;
                }

                let type_f = voxel as u8 as f32;
                let ox = origin.x + x as f32;
                let oy = origin.y + y as f32;
                let oz = origin.z + z as f32;

                for ([dx, dy, dz], normal, corners) in &FACES {
                    if !chunk.neighbor_is_air(x, y, z, *dx, *dy, *dz) {
                        continue;
                    }

                    let base = positions.len() as u32;
                    for [cx, cy, cz] in corners {
                        positions.push([ox + cx, oy + cy, oz + cz]);
                        normals.push(*normal);
                        colors.push([type_f, 0., 0., 1.]);
                    }
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// --- Click handler ---

fn on_chunk_click(
    trigger: On<Pointer<Click>>,
    mut commands: Commands,
    mut chunks: Query<(&ChunkCoord, &mut Chunk)>,
) {
    if trigger.event().button != PointerButton::Primary {
        return;
    }
    let hit = &trigger.event().event.hit;
    let (Some(pos), Some(normal)) = (hit.position, hit.normal) else {
        return;
    };

    let voxel_world = (pos - normal * 0.001).floor().as_ivec3();
    let entity = trigger.event_target();

    if let Ok((coord, mut chunk)) = chunks.get_mut(entity) {
        let local = voxel_world - coord.0 * CHUNK_SIZE as i32;
        if local.clamp(IVec3::ZERO, IVec3::splat(CHUNK_SIZE as i32 - 1)) == local {
            edit_voxel(
                &mut commands,
                entity,
                &mut chunk,
                local.x as usize,
                local.y as usize,
                local.z as usize,
                Voxel::Air,
            );
        }
    }
}
