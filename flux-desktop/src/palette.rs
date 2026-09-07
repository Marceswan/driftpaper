use std::path::Path;

/// Convert density setting to grid_spacing value
/// Larger values = fewer lines = less memory usage
pub(crate) fn density_to_grid_spacing(density: u32) -> u32 {
    match density {
        0 => 25, // Sparse - fewer stems, lowest memory
        1 => 15, // Normal - balanced
        2 => 10, // Dense - more stems
        _ => 15,
    }
}

/// Get color preset from scheme index
pub(crate) fn scheme_to_color_mode(scheme: u32) -> flux::settings::ColorMode {
    use flux::settings::{ColorMode, ColorPreset};
    match scheme {
        0 => ColorMode::Preset(ColorPreset::Original),
        1 => ColorMode::Preset(ColorPreset::Plasma),
        2 => ColorMode::Preset(ColorPreset::Poolside),
        3 => ColorMode::Preset(ColorPreset::SpaceGrey),
        // 4 = Custom Image - use Original as placeholder; actual custom wheel is injected separately
        4 => ColorMode::Preset(ColorPreset::Original),
        _ => ColorMode::Preset(ColorPreset::Original),
    }
}

/// Convert HSL values to RGB floats (0.0-1.0)
pub(crate) fn hsl_to_rgb_f32(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue_to_rgb = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (r, g, b)
}

/// Extract 6 dominant colors from an image and return as [f32; 24] color wheel
pub(crate) fn extract_colors_from_image(path: &Path) -> Result<[f32; 24], String> {
    let img = image::open(path).map_err(|e| format!("Failed to open image: {}", e))?;

    // Downscale to max 200x200 for fast processing
    let thumb = img.thumbnail(200, 200);
    let rgb = thumb.to_rgb8();

    // Bin pixels into 12 hue buckets (30 degrees each)
    struct HueBucket {
        h_sum: f64,
        s_sum: f64,
        l_sum: f64,
        count: u64,
    }
    let mut buckets: Vec<HueBucket> = (0..12)
        .map(|_| HueBucket {
            h_sum: 0.0,
            s_sum: 0.0,
            l_sum: 0.0,
            count: 0,
        })
        .collect();

    for pixel in rgb.pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let l = (max + min) / 2.0;

        // Filter very dark, very light, and near-grey pixels
        if !(0.08..=0.92).contains(&l) || delta < 0.02 {
            continue;
        }

        let s = if l < 0.5 {
            delta / (max + min)
        } else {
            delta / (2.0 - max - min)
        };

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };

        let bucket_idx = ((h / 30.0) as usize).min(11);
        buckets[bucket_idx].h_sum += h as f64;
        buckets[bucket_idx].s_sum += s as f64;
        buckets[bucket_idx].l_sum += l as f64;
        buckets[bucket_idx].count += 1;
    }

    // Collect non-empty buckets with averages
    let mut candidates: Vec<(f32, f32, f32, u64)> = buckets
        .iter()
        .filter(|b| b.count > 0)
        .map(|b| {
            let n = b.count as f64;
            (
                (b.h_sum / n) as f32,
                (b.s_sum / n) as f32,
                (b.l_sum / n) as f32,
                b.count,
            )
        })
        .collect();

    // Sort by count descending, take top 6
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.3));

    // Handle monochrome edge case: if fewer than 6 buckets, spread lightness
    if candidates.len() < 6 {
        if candidates.is_empty() {
            // Completely monochrome or featureless - generate a neutral spread
            candidates = (0..6)
                .map(|i| (0.0, 0.0, 0.2 + (i as f32) * 0.12))
                .map(|c| (c.0, c.1, c.2, 1))
                .collect();
        } else {
            // Duplicate and vary lightness
            let base = candidates.clone();
            while candidates.len() < 6 {
                let src = &base[candidates.len() % base.len()];
                let offset = (candidates.len() as f32) * 0.08;
                let new_l = (src.2 + offset).min(0.85);
                candidates.push((src.0, src.1, new_l, 1));
            }
        }
    }
    candidates.truncate(6);

    // Sort by hue for smooth shader interpolation
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Convert HSL -> RGB, pack into [f32; 24]
    let mut wheel = [0.0f32; 24];
    for (i, (h, s, l, _)) in candidates.iter().enumerate() {
        let (r, g, b) = hsl_to_rgb_f32(*h / 360.0, *s, *l);
        wheel[i * 4] = r;
        wheel[i * 4 + 1] = g;
        wheel[i * 4 + 2] = b;
        wheel[i * 4 + 3] = 1.0;
    }

    log::info!(
        "Extracted {} colors from image: {:?}",
        candidates.len(),
        path
    );
    Ok(wheel)
}

/// Convert noise strength setting to noise_multiplier value
pub(crate) fn noise_strength_to_multiplier(strength: u32) -> f32 {
    match strength {
        0 => 0.15, // Low
        1 => 0.45, // Medium (default)
        2 => 0.75, // High
        3 => 1.0,  // Max
        _ => 0.45,
    }
}

/// Convert line length setting to line_length value
pub(crate) fn line_length_to_value(length: u32) -> f32 {
    match length {
        0 => 63.0,  // Short
        1 => 142.0, // Medium
        2 => 220.0, // Long
        3 => 315.0, // Extra Long
        _ => 142.0,
    }
}

/// Convert line width setting to line_width value
pub(crate) fn line_width_to_value(width: u32) -> f32 {
    match width {
        0 => 4.0,  // Thin
        1 => 9.0,  // Medium (default)
        2 => 16.0, // Thick
        _ => 9.0,
    }
}

/// Convert view scale setting to view_scale value
pub(crate) fn view_scale_to_value(scale: u32) -> f32 {
    match scale {
        0 => 1.0, // Compact
        1 => 1.6, // Normal (default)
        2 => 2.2, // Wide
        _ => 1.6,
    }
}

/// Convert brightness setting to multiplier value
pub(crate) fn brightness_to_multiplier(brightness: u32) -> f32 {
    match brightness {
        0 => 0.5, // Dim
        1 => 1.0, // Normal (default)
        2 => 2.0, // Bright
        3 => 3.5, // Vivid
        _ => 1.0,
    }
}
