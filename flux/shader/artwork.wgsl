struct Uniforms {
  palette: array<vec4<f32>, 6>,
  // aspect, view scale, brightness, fixed simulation timestep
  controls: vec4<f32>,
  // mode, color source, density, unused
  options: vec4<u32>,
  // UV offset and scale for a screen viewport
  viewport: vec4<f32>,
  // Speed-scaled elapsed seconds, wrapping at 1000; remaining lanes reserved.
  motion: vec4<f32>,
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

fn hash_cell(p: vec2<f32>) -> vec3<f32> {
  var h = fract(vec3(p.x, p.y, p.x) * vec3(0.1031, 0.1030, 0.0973));
  h += dot(h, h.yxz + 33.33);
  return fract((h.xxy + h.yzz) * h.zyx);
}

fn rain_background(uv: vec2<f32>) -> vec3<f32> {
  let fog = field(uv * 0.38 + vec2(0.21, 0.39));
  var color = palette(0.54 + fog.x * 1.8 + uv.y * 0.11) * 0.28;
  let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
  // Defocused lights behind the glass; deliberately broad and low contrast.
  for (var i = 0u; i < 5u; i++) {
    let h = hash_cell(vec2(f32(i), 9.0));
    let center = vec2((f32(i) - 2.0) * 0.27, (h.y - 0.5) * 0.5);
    let radius = 0.04 + 0.075 * h.z;
    let d = length(p - center);
    let light = 1.0 - smoothstep(radius * 0.2, radius + 0.14, d);
    color += mix(palette(h.z + fog.y * 0.3), vec3(0.65, 0.58, 0.42), 0.25) * light * 0.65;
  }
  return color;
}

// xy: refraction; z: glass highlight; w: wet coverage. Each drop grows,
// slides, and fades inside its cell before restarting with no visible jump.
fn rain_layer(p: vec2<f32>, scale: vec2<f32>, offset: vec2<f32>) -> vec4<f32> {
  let grid = p * scale + offset;
  let cell = floor(grid);
  let h = hash_cell(cell);
  let local = fract(grid) - 0.5;
  // Integral cycle counts over 1000 seconds keep the wrapped clock seamless.
  let phase = fract(u.motion.x * 0.02 * (1.0 + floor(h.z * 2.0)) + h.x);
  let slide = smoothstep(0.36, 0.96, phase);
  let fade = smoothstep(0.0, 0.12, phase) * (1.0 - smoothstep(0.88, 1.0, phase));
  let radius = 0.055 + 0.12 * smoothstep(0.0, 0.42, phase);
  let center = vec2((h.y - 0.5) * 0.48 + 0.035 * sin(slide * 9.0 + h.z * 6.28),
                    -0.25 + slide * 0.57);
  let q = (local - center) * vec2(1.0, 0.78);
  let distance = length(q);
  let aa = max(fwidth(distance), 0.001);
  let drop = (1.0 - smoothstep(radius - aa, radius + aa, distance)) * fade;
  let normal = q / max(radius, 0.01);
  let lens = max(0.0, 1.0 - dot(normal, normal));
  let trail_length = 0.42 * slide;
  let trail = (1.0 - smoothstep(0.009, 0.027, abs(local.x - center.x)))
    * smoothstep(center.y - trail_length - 0.02, center.y - trail_length + 0.03, local.y)
    * (1.0 - smoothstep(center.y - 0.03, center.y, local.y)) * slide * fade;
  let highlight = exp(-dot(normal - vec2(-0.36, -0.4), normal - vec2(-0.36, -0.4)) * 24.0);
  let rim = pow(clamp(distance / radius, 0.0, 1.0), 8.0);
  let refraction = normal * lens * drop * 0.10 + vec2(trail * 0.012, 0.0);
  return vec4(refraction, drop * (highlight * 0.30 + rim * 0.045), max(drop, trail * 0.4));
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
  } else if (u.options.x == 7u) {
    // Broad asymmetric sand ridges: diffuse light on the windward face and
    // a darker lee. Screen derivatives avoid extra noise reads for normals.
    let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
    let phase = p.y * 2.7 + p.x * 0.42
      + sin(p.x * 2.2 + broad.x * 10.0) * 0.36 + n.y * 2.2;
    let wave = 0.5 + 0.5 * sin(phase * 6.2831853);
    let height = pow(wave, 2.4);
    let slope = vec2(dpdx(height) / max(fwidth(p.x), 0.00001),
                     dpdy(height) / max(fwidth(p.y), 0.00001));
    let normal = normalize(vec3(-slope * 0.16, 1.0));
    let sun = max(dot(normal, normalize(vec3(-0.4, -0.65, 0.5))), 0.0);
    let lee = smoothstep(-1.5, 4.0, slope.y);
    let ripple_phase = phase * 170.0 + n.x * 4.0;
    let ripple = sin(ripple_phase) * (1.0 - smoothstep(0.7, 2.8, fwidth(ripple_phase)));
    let tint = mix(palette(0.15 + broad.y * 0.22 + height * 0.08), vec3(0.64, 0.48, 0.31), 0.3);
    color = tint * (0.25 + 0.85 * sun) * (1.0 - 0.42 * lee) * (0.97 + ripple * 0.03);
  } else if (u.options.x == 8u) {
    // Thin-film interference inside softly lit, frosted mineral layers.
    let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
    let layer = p.x * 1.7 + p.y * 0.95 + broad.x * 8.0 + n.y * 1.8;
    let sheen = 0.5 + 0.5 * sin(layer * 6.2831853);
    let interference = 0.5 + 0.5 * cos(vec3(0.0, 2.1, 4.2) + layer * 4.5 + broad.y * 3.0);
    let pearl = mix(palette(0.48 + layer * 0.12), interference, 0.36);
    let body = mix(pearl, vec3(0.68, 0.70, 0.72), 0.36);
    let glaze = pow(sheen, 7.0) * 0.12;
    let frost = (hash_cell(floor(screen_uv * vec2(1600.0, 1000.0))).x - 0.5) * 0.008;
    color = body * (0.45 + 0.35 * sheen) + pearl * glaze + vec3(frost);
  } else if (u.options.x == 9u) {
    let p = (uv - 0.5) * vec2(u.controls.x, 1.0);
    let drops = rain_layer(p, vec2(12.0, 6.0), vec2(0.0));
    let small = rain_layer(p, vec2(23.0, 12.0), vec2(2.7, 9.2));
    let refraction = (drops.xy + small.xy * 0.4) / vec2(u.controls.x, 1.0);
    let wet = max(drops.w, small.w * 0.65);
    color = rain_background(uv + refraction) * (1.0 - wet * 0.16)
      + vec3(drops.z + small.z * 0.45);
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
