#import bevy_pbr::forward_io::VertexOutput

struct VoxelMaterial {
    clip_y: vec4<f32>, // .x holds the clip y value
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: VoxelMaterial;

// --- Noise ---

fn hash3(p: vec3<f32>) -> f32 {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yxz + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash3(i),                     hash3(i + vec3(1.,0.,0.)), u.x),
            mix(hash3(i + vec3(0.,1.,0.)),    hash3(i + vec3(1.,1.,0.)), u.x), u.y),
        mix(mix(hash3(i + vec3(0.,0.,1.)),    hash3(i + vec3(1.,0.,1.)), u.x),
            mix(hash3(i + vec3(0.,1.,1.)),    hash3(i + vec3(1.,1.,1.)), u.x), u.y),
        u.z
    );
}

fn fbm(p: vec3<f32>) -> f32 {
    return noise(p) * 0.5 + noise(p * 2.1) * 0.25 + noise(p * 4.3) * 0.125;
}

// --- Fragment ---

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    if world_pos.y > material.clip_y.x { discard; }
    let normal    = normalize(in.world_normal);

    // Voxel type is encoded in vertex color red channel (1.0=Soil, 2.0=Stone)
    var voxel_type: u32 = 1u;
#ifdef VERTEX_COLORS
    voxel_type = u32(in.color.r + 0.5);
#endif

    var base: vec3<f32>;
    if voxel_type == 1u {
        // Grass: sharp variation
        let n = fbm(world_pos * 3.0);
        base = mix(vec3(0.12, 0.28, 0.06), vec3(0.20, 0.38, 0.10), n);
    } else if voxel_type == 2u {
        // Stone: soft variation
        let n = fbm(world_pos * 1.5);
        base = mix(vec3(0.35, 0.35, 0.36), vec3(0.58, 0.57, 0.55), n);
    } else {
        base = vec3(1.0, 0.0, 1.0); // magenta = unknown type
    }

    // Simple directional + ambient lighting
    let light_dir = normalize(vec3(0.5, 1.0, 0.3));
    let ambient   = 0.35;
    let diffuse   = max(dot(normal, light_dir), 0.0) * 0.65;

    return vec4(base * (ambient + diffuse), 1.0);
}
