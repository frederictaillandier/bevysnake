use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::events::{Click, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
mod voxel_material;
mod genesis;

use voxel_material::VoxelMaterial;
use genesis::generate_chunk;

pub const CHUNK_SIZE: usize = 16;

// --- Clip plane ---

#[derive(Resource)]
pub struct ClipPlane {
    pub y: f32,
}

impl Default for ClipPlane {
    fn default() -> Self {
        Self {
            y: (CHUNK_SIZE / 2) as f32,
        }
    }
}

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
    pub fn from_world_pos(pos: Vec3) -> Self {
        ChunkCoord(IVec3::new(
            pos.x.floor() as i32 / CHUNK_SIZE as i32,
            pos.y.floor() as i32 / CHUNK_SIZE as i32,
            pos.z.floor() as i32 / CHUNK_SIZE as i32,
        ))
    }

    pub fn world_origin(&self) -> Vec3 {
        self.0.as_vec3() * CHUNK_SIZE as f32
    }
}

// --- Dirty marker ---

/// When added to a chunk entity, its mesh will be rebuilt next frame.
#[derive(Component)]
pub struct ChunkDirty;

// --- Cap entity ---

/// Holds the entity ID of this chunk's cap mesh entity.
#[derive(Component)]
pub struct ChunkCapEntity(pub Entity);

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

// --- Shared material handle ---

#[derive(Resource)]
pub struct SharedVoxelMaterial(pub Handle<VoxelMaterial>);

// --- Plugin ---

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MeshPickingPlugin,
            MaterialPlugin::<VoxelMaterial>::default(),
        ))
        .init_resource::<ClipPlane>()
        .add_systems(Startup, spawn_initial_chunks)
        .add_systems(Update, (rebuild_dirty_chunks, rebuild_caps_on_clip_change, sync_clip_plane, sync_cap_transforms));
    }
}


// --- Startup: spawn chunks with their mesh ---

fn spawn_initial_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    clip: Res<ClipPlane>,
) {
    let material = materials.add(VoxelMaterial::default());
    commands.insert_resource(SharedVoxelMaterial(material.clone()));

    for cx in -4..=4_i32 {
        for cz in -4..=4_i32 {
            let coord = ChunkCoord(IVec3::new(cx, 0, cz));
            let origin = coord.world_origin();
            let chunk = generate_chunk([origin.x as i32, origin.y as i32, origin.z as i32]);
            let mesh = meshes.add(build_chunk_mesh(&chunk, origin));
            let cap_mesh = meshes.add(build_cap_mesh(&chunk, origin, clip.y));

            let cap_entity = commands
                .spawn((
                    Mesh3d(cap_mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(Vec3::new(0., clip.y - 0.01, 0.)),
                    Visibility::default(),
                ))
                .id();

            commands
                .spawn((
                    coord,
                    chunk,
                    ChunkCapEntity(cap_entity),
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::default(),
                ))
                .observe(on_chunk_click);
        }
    }
}

// --- Sync clip plane to shader ---

fn sync_clip_plane(
    clip: Res<ClipPlane>,
    shared: Res<SharedVoxelMaterial>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    if !clip.is_changed() {
        return;
    }
    if let Some(mat) = materials.get_mut(&shared.0) {
        mat.clip_y = Vec4::new(clip.y, 0., 0., 0.);
    }
}

// --- Rebuild dirty chunks ---

fn rebuild_dirty_chunks(
    mut commands: Commands,
    mut query: Query<(Entity, &ChunkCoord, &Chunk, &mut Mesh3d, Option<&ChunkCapEntity>), With<ChunkDirty>>,
    mut meshes: ResMut<Assets<Mesh>>,
    clip: Res<ClipPlane>,
    mut cap_query: Query<&mut Mesh3d, Without<Chunk>>,
) {
    for (entity, coord, chunk, mut mesh3d, cap_entity) in &mut query {
        let origin = coord.world_origin();
        mesh3d.0 = meshes.add(build_chunk_mesh(chunk, origin));
        if let Some(ChunkCapEntity(cap)) = cap_entity {
            if let Ok(mut cap_mesh3d) = cap_query.get_mut(*cap) {
                cap_mesh3d.0 = meshes.add(build_cap_mesh(chunk, origin, clip.y));
            }
        }
        commands.entity(entity).remove::<ChunkDirty>();
    }
}

fn rebuild_caps_on_clip_change(
    clip: Res<ClipPlane>,
    mut last_cap_y: Local<i32>,
    chunk_query: Query<(&ChunkCoord, &Chunk, &ChunkCapEntity)>,
    mut cap_query: Query<&mut Mesh3d, Without<Chunk>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !clip.is_changed() {
        return;
    }
    let new_floor = clip.y.floor() as i32;
    if new_floor == *last_cap_y {
        return;
    }
    *last_cap_y = new_floor;

    for (coord, chunk, ChunkCapEntity(cap)) in &chunk_query {
        let new_mesh = meshes.add(build_cap_mesh(chunk, coord.world_origin(), clip.y));
        if let Ok(mut cap_mesh3d) = cap_query.get_mut(*cap) {
            cap_mesh3d.0 = new_mesh;
        }
    }
}

// --- Mesh building ---

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

fn sync_cap_transforms(
    clip: Res<ClipPlane>,
    chunks: Query<&ChunkCapEntity>,
    mut transforms: Query<&mut Transform>,
) {
    if !clip.is_changed() {
        return;
    }
    for ChunkCapEntity(cap) in &chunks {
        if let Ok(mut transform) = transforms.get_mut(*cap) {
            transform.translation.y = clip.y - 0.01;
        }
    }
}

/// Build the cap mesh for a chunk: top-face quads at `clip_y.floor()` for every
/// solid voxel in the layer that the clip plane cuts through.
/// Only emits geometry when the cut layer falls inside this chunk's Y range.
fn build_cap_mesh(chunk: &Chunk, origin: Vec3, clip_y: f32) -> Mesh {
    let cap_world_y = clip_y.floor();
    let local_cap_y = cap_world_y as i32 - origin.y as i32;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if local_cap_y >= 0 && local_cap_y < CHUNK_SIZE as i32 {
        let ly = local_cap_y as usize;
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let voxel = chunk.get(x, ly, z);
                if voxel == Voxel::Air {
                    continue;
                }
                let type_f = voxel as u8 as f32;
                let ox = origin.x + x as f32;
                let oz = origin.z + z as f32;
                let base = positions.len() as u32;
                // y = 0: the cap entity's Transform.translation.y drives the actual height
                positions.extend_from_slice(&[
                    [ox,      0., oz     ],
                    [ox,      0., oz + 1.],
                    [ox + 1., 0., oz + 1.],
                    [ox + 1., 0., oz     ],
                ]);
                for _ in 0..4 {
                    normals.push([0., 1., 0.]);
                    colors.push([type_f, 0., 0., 1.]);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
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
