struct ClearPressure { value: f32, }
@group(0) @binding(0) var<uniform> clear: ClearPressure;
@group(1) @binding(1) var pressure: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(pressure))) { return; }
    textureStore(pressure, id.xy, vec4<f32>(clear.value, 0.0, 0.0, 0.0));
}
