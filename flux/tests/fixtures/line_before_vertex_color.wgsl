struct LineUniforms {
  aspect: f32,
  zoom: f32,
  line_width: f32,
  line_length: f32,
  line_begin_offset: f32,
  line_variance: f32,
  line_noise_scale: vec2<f32>,
  line_noise_offset_1: f32,
  line_noise_offset_2: f32,
  line_noise_blend_factor: f32,
  color_mode: u32,
  delta_time: f32,
  brightness_scale: f32,
}

@group(0) @binding(0) var<uniform> uniforms: LineUniforms;
@group(1) @binding(0) var<uniform> view_matrix: mat4x4<f32>;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) f_vertex: vec2<f32>,
  @location(1) f_color: vec4<f32>,
  @location(2) f_line_offset: f32,
}

@vertex
fn main_vs(
  @location(0) endpoint: vec2<f32>, // 0
  @location(1) velocity: vec2<f32>, // 8
  @location(2) color: vec4<f32>, // 16
  @location(3) color_velocity: vec3<f32>, // 32
  @location(4) width: f32, // 44
  @location(5) basepoint: vec2<f32>, // 48
  @location(6) vertex: vec2<f32>, // 56
) -> VertexOutput { // 64
  var x_basis = vec2<f32>(-endpoint.y, endpoint.x);
  x_basis /= max(length(x_basis), 1e-10); // safely normalize

  var point = vec2<f32>(uniforms.aspect, 1.0) * uniforms.zoom * (basepoint * 2.0 - 1.0)
    + endpoint * vertex.y
    + uniforms.line_width * width * x_basis * vertex.x;

  point.x /= uniforms.aspect;

  let short_line_boost = 1.0 + ((uniforms.line_width * width) / length(endpoint));
  let line_offset = uniforms.line_begin_offset / short_line_boost;

  let transformed_point = view_matrix * vec4<f32>(point, 0.0, 1.0);

  return VertexOutput(
    transformed_point,
    vertex,
    color,
    line_offset,
  );
}

// Convert RGB to HSL
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
  let max_c = max(max(rgb.r, rgb.g), rgb.b);
  let min_c = min(min(rgb.r, rgb.g), rgb.b);
  let delta = max_c - min_c;

  var h: f32 = 0.0;
  var s: f32 = 0.0;
  let l = (max_c + min_c) * 0.5;

  if (delta > 0.0) {
    s = delta / (1.0 - abs(2.0 * l - 1.0));
    if (max_c == rgb.r) {
      h = ((rgb.g - rgb.b) / delta) % 6.0;
    } else if (max_c == rgb.g) {
      h = (rgb.b - rgb.r) / delta + 2.0;
    } else {
      h = (rgb.r - rgb.g) / delta + 4.0;
    }
    h /= 6.0;
    if (h < 0.0) {
      h += 1.0;
    }
  }

  return vec3<f32>(h, s, l);
}

// Convert HSL to RGB
fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
  let h = hsl.x;
  let s = hsl.y;
  let l = hsl.z;

  let c = (1.0 - abs(2.0 * l - 1.0)) * s;
  let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
  let m = l - c * 0.5;

  var rgb: vec3<f32>;
  let h6 = h * 6.0;

  if (h6 < 1.0) {
    rgb = vec3<f32>(c, x, 0.0);
  } else if (h6 < 2.0) {
    rgb = vec3<f32>(x, c, 0.0);
  } else if (h6 < 3.0) {
    rgb = vec3<f32>(0.0, c, x);
  } else if (h6 < 4.0) {
    rgb = vec3<f32>(0.0, x, c);
  } else if (h6 < 5.0) {
    rgb = vec3<f32>(x, 0.0, c);
  } else {
    rgb = vec3<f32>(c, 0.0, x);
  }

  return rgb + m;
}

// Cap saturation and luminance to reduce eye strain
fn cap_brightness(rgb: vec3<f32>) -> vec3<f32> {
  var hsl = rgb_to_hsl(rgb);
  // Cap saturation at 25% for very muted colors
  hsl.y = min(hsl.y, 0.25);
  // Cap luminance at 30% - much darker
  hsl.z = min(hsl.z, 0.30);
  return hsl_to_rgb(hsl);
}

@fragment
fn main_fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  let fade = smoothstep(fs_input.f_line_offset, 1.0, fs_input.f_vertex.y);

  let x_offset = abs(fs_input.f_vertex.x);
  let smooth_edges = 1.0 - smoothstep(0.5 - fwidth(x_offset), 0.5, x_offset);

  // Apply brightness capping to reduce eye strain
  let capped_color = cap_brightness(fs_input.f_color.rgb);
  // Scale color by brightness_scale (based on line count) to normalize across displays
  // Base multiplier 0.3 for darker overall look, then scale by line count
  let scaled_color = capped_color * uniforms.brightness_scale * 0.3;
  return vec4<f32>(scaled_color, fs_input.f_color.a * fade * smooth_edges);
}
