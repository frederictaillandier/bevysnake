use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub struct ClipPlanePlugin;

impl Plugin for ClipPlanePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClipPlane::default())
            .add_systems(
                Update,
                (
                    sync_clip_plane_material,
                    sync_cap_transforms,
                    rebuild_caps_on_clip_change,
                ),
            );
    }
}

#[derive(Resource)]
pub struct ClipPlane {
    pub y: f32,
}

impl Default for ClipPlane {
    fn default() -> Self {
        Self {
            y: (super::CHUNK_SIZE / 2) as f32,
        }
    }
}

pub fn sync_clip_plane_material(
    clip: Res<ClipPlane>,
    voxel_material: Res<super::voxel_material::SharedVoxelMaterial>,
    mut materials: ResMut<Assets<super::voxel_material::VoxelMaterial>>,
) {
    if !clip.is_changed() {
        return;
    }
    if let Some(mat) = materials.get_mut(&voxel_material.0) {
        mat.clip_y = Vec4::new(clip.y, 0., 0., 0.);
    }
}

fn sync_cap_transforms(
    clip: Res<ClipPlane>,
    chunks_caps: Query<&ChunkCapEntity>,
    mut transforms: Query<&mut Transform>,
) {
    if !clip.is_changed() {
        return;
    }
    for ChunkCapEntity(cap) in &chunks_caps {
        if let Ok(mut transform) = transforms.get_mut(*cap) {
            transform.translation.y = clip.y - 0.01;
        }
    }
}

/// Holds the entity ID of this chunk's cap mesh entity.
#[derive(Component)]
pub struct ChunkCapEntity(pub Entity);

fn rebuild_caps_on_clip_change(
    clip: Res<ClipPlane>,
    mut last_cap_y: Local<i32>,
    chunk_query: Query<(&super::ChunkCoord, &super::Chunk, &ChunkCapEntity)>,
    mut cap_query: Query<&mut Mesh3d, Without<super::Chunk>>,
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

/// Build the cap mesh for a chunk: top-face quads at `clip_y.floor()` for every
/// solid voxel in the layer that the clip plane cuts through.
/// Only emits geometry when the cut layer falls inside this chunk's Y range.
pub fn build_cap_mesh(chunk: &super::Chunk, origin: Vec3, clip_y: f32) -> Mesh {
    let cap_world_y = clip_y.floor();
    let local_cap_y = cap_world_y as i32 - origin.y as i32;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    if local_cap_y >= 0 && local_cap_y < super::CHUNK_SIZE as i32 {
        let ly = local_cap_y as usize;
        for z in 0..super::CHUNK_SIZE {
            for x in 0..super::CHUNK_SIZE {
                let voxel = chunk.get(x, ly, z);
                if voxel == super::Voxel::Air {
                    continue;
                }
                let type_f = voxel as u8 as f32;
                let ox = origin.x + x as f32;
                let oz = origin.z + z as f32;
                let base = positions.len() as u32;
                // y = 0: the cap entity's Transform.translation.y drives the actual height
                positions.extend_from_slice(&[
                    [ox, 0., oz],
                    [ox, 0., oz + 1.],
                    [ox + 1., 0., oz + 1.],
                    [ox + 1., 0., oz],
                ]);
                for _ in 0..4 {
                    normals.push([0., 1., 0.]);
                    colors.push([type_f, 0., 0., 1.]);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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
