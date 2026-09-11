struct Uniforms {
  palette: array<vec4<f32>, 6>,
  // aspect, view scale, brightness, fixed simulation timestep
  controls: vec4<f32>,
  // mode, color source, density, unused
  options: vec4<u32>,
  // UV offset and scale for a screen viewport
  viewport: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var noise_texture: texture_2d<f32>;
@group(0) @binding(3) var color_texture: texture_2d<f32>;
@group(1) @binding(0) var dye_texture: texture_2d<f32>;
@group(1) @binding(1) var out_dye: texture_storage_2d<rgba16float, write>;
@group(2) @binding(0) var velocity_texture: texture_2d<f32>;

fn palette(t: f32) -> vec3<f32> {
  if (u.options.y == 0u) {
    // Original Drift colors mapped around the same velocity-direction color wheel.
    let direction = vec2(cos(t * 6.2831853), sin(t * 6.2831853));
    return vec3(clamp(vec2(1.0, 0.66) * (0.5 + 0.5 * direction), vec2(0.0), vec2(1.0)), 0.5);
  }
  if (u.options.y == 2u) {
    return textureSampleLevel(color_texture, linear_sampler, vec2(fract(t), 0.5), 0.0).rgb;
  }
  let index = fract(t) * 6.0;
  return mix(u.palette[u32(index)].rgb, u.palette[(u32(index) + 1u) % 6u].rgb, fract(index));
}

fn field(uv: vec2<f32>) -> vec2<f32> {
  // Mirror the edges so wide view scales do not expose a clamped border.
  let p = 1.0 - abs(1.0 - 2.0 * fract(uv * 0.5));
  // Cubic B-spline filtering from four bilinear taps. Bilinear noise has kinks at
  // texel edges, which make contours and normals crawl as the field evolves.
  let size = vec2<f32>(textureDimensions(noise_texture));
  let texel = p * size - 0.5;
  let base = floor(texel);
  let f = texel - base;
  let f2 = f * f;
  let f3 = f2 * f;
  let w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
  let w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
  let w3 = f3 / 6.0;
  let g0 = w0 + w1;
  let h0 = (base - 0.5 + w1 / g0) / size;
  let h1 = (base + 1.5 + w3 / (1.0 - g0)) / size;
  let a = textureSampleLevel(noise_texture, linear_sampler, h0, 0.0).xy;
  let b = textureSampleLevel(noise_texture, linear_sampler, vec2(h1.x, h0.y), 0.0).xy;
  let c = textureSampleLevel(noise_texture, linear_sampler, vec2(h0.x, h1.y), 0.0).xy;
  let d = textureSampleLevel(noise_texture, linear_sampler, h1, 0.0).xy;
  let g1 = 1.0 - g0;
  return mix(mix(a, b, g1.x), mix(c, d, g1.x), g1.y);
}

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
}
@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOutput {
  let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));
  return VertexOutput(vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0), uv);
}

@fragment
fn fs(v: VertexOutput) -> @location(0) vec4<f32> {
  let screen_uv = u.viewport.xy + v.uv * u.viewport.zw;
  let uv = (screen_uv - 0.5) / u.controls.y + 0.5;
  let n = field(uv);
  let broad = field(uv * 0.43 + vec2(0.19, 0.27));
  var color: vec3<f32>;
  if (u.options.x == 1u) {
    // Long warped folds with a narrow satin highlight and fine woven fibers.
    let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
    let fold = p.y * 3.5 + p.x * 0.7 + broad.x * 2.8 + n.y * 0.55;
    let wave = 0.5 + 0.5 * sin(fold * 6.2831853);
    let satin = pow(wave, 14.0);
    let fibers_phase = fold * 85.0;
    let fibers = (0.5 + 0.5 * sin(fibers_phase)) * (1.0 - smoothstep(1.0, 3.0, fwidth(fibers_phase)));
    let tint = palette(0.38 + broad.y * 0.35 + p.x * 0.08);
    color = tint * (0.025 + 0.24 * wave + 0.32 * satin) * (0.86 + 0.14 * fibers);
  } else if (u.options.x == 2u) {
    let dye = textureSampleLevel(dye_texture, linear_sampler, uv, 0.0).rgb;
    // Dye stores three pigment weights so palette changes recolor existing ink.
    let pigment = dye.r * palette(0.0) + dye.g * palette(0.34) + dye.b * palette(0.67);
    color = vec3(0.003, 0.005, 0.009) + (1.0 - exp(-pigment * 1.5)) * 0.65;
  } else if (u.options.x == 4u) {
    // Three transparent curtains, with a bright lower edge and tall soft rays.
    color = palette(0.8) * 0.008;
    for (var layer = 0u; layer < 3u; layer++) {
      let l = f32(layer);
      let curtain = field(vec2(uv.x * 0.85 + l * 0.12, 0.33 + l * 0.17));
      let base = 0.56 + 0.12 * l + curtain.x * 0.40 + sin(uv.x * 5.0 + l) * 0.06;
      let height = base - uv.y;
      let falloff = exp(-max(height, 0.0) * (7.0 + l));
      let edge = exp(-abs(height) * 75.0);
      let ray_phase = uv.x * 125.0 + n.x * 6.0 + curtain.y * 12.0;
      let rays = 0.7 + 0.3 * sin(ray_phase);
      let mask = smoothstep(-0.015, 0.025, height);
      color += palette(0.18 + l * 0.23 + height * 0.75 + curtain.y * 0.12)
        * (0.42 * falloff * mask * rays + 0.25 * edge);
    }
  } else if (u.options.x == 5u) {
    // Two warped wave families form an inexpensive caustic-light approximation.
    let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
    let a = p.x * 4.0 + n.y * 2.8 + broad.x * 1.6;
    let b = p.y * 4.0 + n.x * 2.8 - broad.y * 1.6;
    let waves = sin(a * 6.2831853) * sin(b * 6.2831853);
    let ridge = pow(1.0 - abs(waves), 14.0);
    let glow = pow(1.0 - abs(waves), 3.0);
    color = palette(0.6 + broad.x * 0.35) * (0.035 + 0.055 * glow)
      + palette(0.15 + n.y * 0.25) * ridge * 0.40;
  } else if (u.options.x == 6u) {
    // Central differences provide resolution-independent surface normals.
    let e = 0.006;
    let dx = (field(uv + vec2(e, 0.0)).x - field(uv - vec2(e, 0.0)).x) / (2.0 * e * u.controls.x);
    let dy = (field(uv + vec2(0.0, e)).x - field(uv - vec2(0.0, e)).x) / (2.0 * e);
    let normal = normalize(vec3(-dx * 0.40, -dy * 0.40, 1.0));
    let reflection = reflect(vec3(0.0, 0.0, -1.0), normal);
    // Broad studio lights plus a thin softbox reflection, without an environment map.
    let softbox = exp(-pow((reflection.y - 0.24) * 6.0, 2.0));
    let strip = exp(-pow((reflection.x + reflection.y * 0.35 + 0.3) * 16.0, 2.0));
    let ambient = 0.5 + 0.5 * reflection.y;
    let fresnel = pow(1.0 - normal.z, 3.0);
    let tint = palette(0.5 + broad.y * 0.45 + reflection.x * 0.15);
    color = tint * (0.045 + 0.16 * ambient + 0.2 * fresnel)
      + mix(tint, vec3(0.85), 0.7) * (softbox * 0.55 + strip * 0.25);
  } else {
    let elevation = n.x * 0.9 + broad.y * 0.55;
    let levels = elevation * (240.0 / f32(max(u.options.z, 5u)));
    let distance = abs(fract(levels + 0.5) - 0.5);
    let aa = max(fwidth(levels), 0.0001);
    let contour = 1.0 - smoothstep(aa * 0.25, aa * 1.25, distance);
    let major_distance = abs(fract(levels / 5.0 + 0.5) - 0.5) * 5.0;
    let major = 1.0 - smoothstep(aa * 0.6, aa * 1.8, major_distance);
    let tint = palette(0.5 + elevation * 0.7);
    color = tint * (0.025 + 0.24 * contour + 0.20 * major);
  }
  let edge = length((screen_uv - 0.5) * vec2(1.0, 0.8));
  color *= (1.0 - 0.45 * smoothstep(0.2, 0.75, edge)) * u.controls.z;
  return vec4(clamp(color, vec3(0.0), vec3(0.85)), 1.0);
}

@compute @workgroup_size(16, 16)
fn ink(@builtin(global_invocation_id) id: vec3<u32>) {
  let size = textureDimensions(out_dye);
  if (any(id.xy >= size)) { return; }
  let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
  let velocity = textureSampleLevel(velocity_texture, linear_sampler, uv, 0.0).xy;
  // Midpoint backtrace reduces distortion compared with a single Euler sample.
  let dt = u.controls.w;
  let midpoint = uv - velocity * dt * 0.06;
  let mid_velocity = textureSampleLevel(velocity_texture, linear_sampler, midpoint, 0.0).xy;
  let source = uv - mid_velocity * dt * 0.12;
  let previous = textureSampleLevel(dye_texture, linear_sampler, source, 0.0).rgb;
  let n = field(uv);
  let aspect = vec2(u.controls.x, 1.0);
  // Distributed narrow sources leave room for dark water and flowing tendrils.
  let bend = vec2(n.x * 0.09, n.y * 0.09);
  let a = (uv - vec2(0.24, 0.62) - bend) * aspect;
  let b = (uv - vec2(0.55, 0.35) + bend) * aspect;
  let c = (uv - vec2(0.78, 0.67) - bend.yx) * aspect;
  let distance = vec3(dot(a,a), dot(b,b), dot(c,c));
  let injection = exp(-distance * 400.0);
  // Thin filaments feed the flow around each source instead of filling solid disks.
  let wisps = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(n.x * 38.0 + n.y * 24.0), 3.0);
  let pigment = previous * exp(-dt * 0.10) + injection * wisps * dt * 1.3;
  textureStore(out_dye, id.xy, vec4(min(pigment, vec3(4.0)), 1.0));
}
