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
  @builtin(position) f_position: vec4<f32>,
  @location(0) f_vertex: vec2<f32>,
  @location(1) f_mindpoint_vector: vec2<f32>,
  @location(2) f_top_color: vec4<f32>,
  @location(3) f_bottom_color: vec4<f32>,
};

// TODO: you can use storage buffers for this instead of messing around with vertex buffers.
@vertex
fn main_vs(
  @location(0) endpoint: vec2<f32>, // 0
  @location(1) velocity: vec2<f32>, // 8
  @location(2) color: vec4<f32>, // 16
  @location(3) color_velocity: vec3<f32>, // 32
  @location(4) width: f32, // 44
  @location(5) basepoint: vec2<f32>, // 48
  @location(6) vertex: vec2<f32>, // 56
) -> VertexOutput {
  var point
    = vec2<f32>(uniforms.aspect, 1.0) * uniforms.zoom * (basepoint * 2.0 - 1.0)
    + endpoint
    + 0.5 * uniforms.line_width * width * vertex;

  point.x /= uniforms.aspect;

  let transformed_point = view_matrix * vec4<f32>(point, 0.0, 1.0);

  // Rotate the endpoint vector 90°. We use this to detect which side of the
  // endpoint we’re on in the fragment.
  let midpoint_vector = vec2<f32>(endpoint.y, -endpoint.x);

  // TODO: figure out option to expose here.
  let endpoint_threshold = 1.0;
  let endpoint_brightness = 1.0;
  // let endpoint_opacity = clamp(color.a + (1.0 - smoothstep(0.2, 0.9, color.a)), 0.0, 1.0);
  let endpoint_opacity = clamp(color.a + endpoint_brightness * max(0.0, endpoint_threshold - color.a), 0.0, 1.0);
  let top_color = vec4<f32>(color.rgb, endpoint_opacity);

  // The color of the lower half of the endpoint is less obvious. We’re
  // drawing over part of the line, so to match the color of the upper
  // endpoint, we have to do some math. Luckily, we know the premultiplied
  // color of the line underneath, so we can reverse the blend equation to get
  // the right color.
  //
  // GL_BLEND(SRC_ALPHA, ONE) = srcColor * srcAlpha + dstColor * srcAlpha
  // = vColor * vEndpointOpacity + vColor * vLineOpacity
  //
  // Remember, we’ve already premultiplied our colors! The opacity should be
  // 1.0 to disable more opacity blending!
  let premultiplied_color = color.rgb * color.a;
  let bottom_color = vec4<f32>(color.rgb * endpoint_opacity - premultiplied_color, 1.0);

  return VertexOutput(
    transformed_point,
    vertex,
    midpoint_vector,
    top_color,
    bottom_color,
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
  var color = fs_input.f_bottom_color;

  // Test which side of the endpoint we're on.
  let side
    = (fs_input.f_vertex.x - fs_input.f_mindpoint_vector.x) * (-fs_input.f_mindpoint_vector.y)
    - (fs_input.f_vertex.y - fs_input.f_mindpoint_vector.y) * (-fs_input.f_mindpoint_vector.x);

  if (side > 0.0) {
    color = fs_input.f_top_color;
  }

  let distance = length(fs_input.f_vertex);
  let smoothEdges = 1.0 - smoothstep(1.0 - fwidth(distance), 1.0, distance);

  // Apply brightness capping
  let capped_color = cap_brightness(color.rgb);
  // Scale color by brightness_scale (based on line count) to normalize across displays
  // Base multiplier 0.3 for darker overall look, then scale by line count
  let scaled_color = capped_color * uniforms.brightness_scale * 0.3;
  return vec4<f32>(scaled_color, color.a * smoothEdges);
}
