use crate::document::validate_dimensions;

pub fn apply(
    pixels: &[u8],
    width: u32,
    height: u32,
    name: &str,
    value: f32,
    second: f32,
) -> Result<Vec<u8>, String> {
    let length = validate_dimensions(width, height)?;
    if pixels.len() != length {
        return Err("effect pixel data does not match the image dimensions".into());
    }
    if !value.is_finite() || !second.is_finite() {
        return Err("effect parameters must be finite numbers".into());
    }
    let result = match name {
        "invert" => map_rgb(pixels, |rgb| rgb.map(|channel| 255.0 - channel)),
        "invert_alpha" => {
            let mut result = pixels.to_vec();
            for pixel in result.chunks_exact_mut(4) {
                pixel[3] = 255 - pixel[3];
            }
            result
        }
        "grayscale" => map_rgb(pixels, |rgb| [luminance(rgb); 3]),
        "sepia" => map_rgb(pixels, |[r, g, b]| {
            [
                0.393 * r + 0.769 * g + 0.189 * b,
                0.349 * r + 0.686 * g + 0.168 * b,
                0.272 * r + 0.534 * g + 0.131 * b,
            ]
        }),
        "auto_levels" => auto_levels(pixels),
        "brightness_contrast" => {
            let brightness = value.clamp(-100.0, 100.0) * 2.55;
            let contrast = second.clamp(-100.0, 100.0) * 2.55;
            let factor = 259.0 * (contrast + 255.0) / (255.0 * (259.0 - contrast));
            map_rgb(pixels, |rgb| {
                rgb.map(|channel| factor * (channel - 127.5) + 127.5 + brightness)
            })
        }
        "hue_saturation" => {
            let hue = value.clamp(-180.0, 180.0) / 60.0;
            let saturation = second.clamp(0.0, 200.0) / 100.0;
            map_rgb(pixels, |rgb| adjust_hsv(rgb, hue, saturation))
        }
        "exposure" => {
            let multiplier = 2.0f32.powf(value.clamp(-5.0, 5.0));
            map_rgb(pixels, |rgb| rgb.map(|channel| channel * multiplier))
        }
        "temperature_tint" => {
            let temperature = value.clamp(-100.0, 100.0) * 0.64;
            let tint = second.clamp(-100.0, 100.0) * 0.64;
            map_rgb(pixels, |[r, g, b]| {
                [
                    r + temperature + tint / 2.0,
                    g - tint,
                    b - temperature + tint / 2.0,
                ]
            })
        }
        "posterize" => {
            let levels = value.round().clamp(2.0, 64.0) - 1.0;
            map_rgb(pixels, |rgb| {
                rgb.map(|channel| (channel / 255.0 * levels).round() / levels * 255.0)
            })
        }
        "gaussian_blur" => gaussian_blur(pixels, width, height, value.clamp(0.0, 50.0)),
        "sharpen" => {
            let amount = value.clamp(0.0, 5.0);
            let blurred = gaussian_blur(pixels, width, height, 2.0);
            let mut result = pixels.to_vec();
            for (target, blur) in result.chunks_exact_mut(4).zip(blurred.chunks_exact(4)) {
                for channel in 0..3 {
                    target[channel] = byte(
                        target[channel] as f32
                            + amount * (target[channel] as f32 - blur[channel] as f32),
                    );
                }
            }
            result
        }
        "pixelate" => pixelate(
            pixels,
            width,
            height,
            value.round().clamp(1.0, 100.0) as u32,
        ),
        "noise" => {
            let strength = value.clamp(0.0, 100.0) * 2.55;
            let mut index = 0u32;
            map_rgb(pixels, |rgb| {
                let mut result = rgb;
                for channel in &mut result {
                    *channel += (random(index) * 2.0 - 1.0) * strength;
                    index = index.wrapping_add(1);
                }
                result
            })
        }
        "emboss" => convolve(
            pixels,
            width,
            height,
            [-2.0, -1.0, 0.0, -1.0, 1.0, 1.0, 0.0, 1.0, 2.0],
            128.0,
            true,
        ),
        "edge_detect" => convolve(
            pixels,
            width,
            height,
            [-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
            0.0,
            false,
        ),
        "glow" => {
            let blurred = gaussian_blur(pixels, width, height, value.clamp(0.0, 50.0));
            let strength = second.clamp(0.0, 200.0) / 100.0;
            let mut result = pixels.to_vec();
            for (pixel, blur) in result.chunks_exact_mut(4).zip(blurred.chunks_exact(4)) {
                for channel in 0..3 {
                    let base = pixel[channel] as f32;
                    pixel[channel] =
                        byte(base + (255.0 - base) * blur[channel] as f32 / 255.0 * strength);
                }
            }
            result
        }
        "motion_blur" => motion_blur(
            pixels,
            width,
            height,
            value.clamp(0.0, 50.0),
            second.to_radians(),
        ),
        "median" => median(pixels, width, height, value.round().clamp(0.0, 5.0) as i32),
        "vignette" => vignette(pixels, width, height, value.clamp(0.0, 100.0) / 100.0),
        "frosted_glass" => frost(pixels, width, height, value.clamp(0.0, 50.0)),
        "twist" => distort(pixels, width, height, value.clamp(-180.0, 180.0), false),
        "bulge" => distort(
            pixels,
            width,
            height,
            value.clamp(-100.0, 100.0) / 100.0,
            true,
        ),
        "zoom_blur" => spin_blur(pixels, width, height, value.clamp(0.0, 100.0) / 100.0, true),
        "radial_blur" => spin_blur(
            pixels,
            width,
            height,
            value.clamp(0.0, 180.0).to_radians(),
            false,
        ),
        "pencil_sketch" => sketch(pixels, width, height, value.clamp(1.0, 20.0), false),
        "ink_sketch" => sketch(pixels, width, height, value.clamp(1.0, 20.0), true),
        _ => return Err(format!("unknown effect: {name}")),
    };
    Ok(result)
}

fn byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn luminance([r, g, b]: [f32; 3]) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn map_rgb(pixels: &[u8], mut operation: impl FnMut([f32; 3]) -> [f32; 3]) -> Vec<u8> {
    let mut result = pixels.to_vec();
    for pixel in result.chunks_exact_mut(4) {
        let mapped = operation([pixel[0] as f32, pixel[1] as f32, pixel[2] as f32]);
        for channel in 0..3 {
            pixel[channel] = byte(mapped[channel]);
        }
    }
    result
}

fn auto_levels(pixels: &[u8]) -> Vec<u8> {
    let mut minimum = [255u8; 3];
    let mut maximum = [0u8; 3];
    for pixel in pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0) {
        for channel in 0..3 {
            minimum[channel] = minimum[channel].min(pixel[channel]);
            maximum[channel] = maximum[channel].max(pixel[channel]);
        }
    }
    map_rgb(pixels, |rgb| {
        let mut result = rgb;
        for channel in 0..3 {
            if maximum[channel] > minimum[channel] {
                result[channel] = (rgb[channel] - minimum[channel] as f32) * 255.0
                    / (maximum[channel] - minimum[channel]) as f32;
            }
        }
        result
    })
}

fn adjust_hsv(rgb: [f32; 3], hue_shift: f32, saturation: f32) -> [f32; 3] {
    let [r, g, b] = rgb.map(|channel| channel / 255.0);
    let value = r.max(g).max(b);
    let minimum = r.min(g).min(b);
    let delta = value - minimum;
    if delta < f32::EPSILON || value < f32::EPSILON {
        return rgb;
    }
    let hue = if value == r {
        (g - b) / delta
    } else if value == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    let hue = (hue + hue_shift).rem_euclid(6.0);
    let chroma = value * (delta / value * saturation).clamp(0.0, 1.0);
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let matched = value - chroma;
    let result = match hue.floor() as u8 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    result.map(|channel| (channel + matched) * 255.0)
}

fn gaussian_blur(pixels: &[u8], width: u32, height: u32, radius: f32) -> Vec<u8> {
    if radius < 0.01 {
        return pixels.to_vec();
    }
    let extent = radius.ceil() as i32;
    let sigma = (radius / 2.0).max(0.5);
    let mut kernel: Vec<f32> = (-extent..=extent)
        .map(|offset| (-(offset * offset) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    for weight in &mut kernel {
        *weight /= sum;
    }
    let horizontal = blur_axis(pixels, width, height, &kernel, true);
    blur_axis(&horizontal, width, height, &kernel, false)
}

fn blur_axis(pixels: &[u8], width: u32, height: u32, kernel: &[f32], horizontal: bool) -> Vec<u8> {
    let mut result = vec![0; pixels.len()];
    let extent = kernel.len() as i32 / 2;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 4];
            for (index, &weight) in kernel.iter().enumerate() {
                let offset = index as i32 - extent;
                let sample_x = (x as i32 + if horizontal { offset } else { 0 })
                    .clamp(0, width as i32 - 1) as u32;
                let sample_y = (y as i32 + if horizontal { 0 } else { offset })
                    .clamp(0, height as i32 - 1) as u32;
                accumulate(
                    &mut sum,
                    pixel_at(pixels, width, sample_x, sample_y),
                    weight,
                );
            }
            let index = ((y * width + x) * 4) as usize;
            result[index..index + 4].copy_from_slice(&straight(sum));
        }
    }
    result
}

fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> &[u8] {
    let index = ((y * width + x) * 4) as usize;
    &pixels[index..index + 4]
}

fn accumulate(sum: &mut [f32; 4], pixel: &[u8], weight: f32) {
    // filters accumulate premultiplied colors so hidden RGB cannot bleed into edges.
    let alpha = pixel[3] as f32 / 255.0;
    for channel in 0..3 {
        sum[channel] += pixel[channel] as f32 * alpha * weight;
    }
    sum[3] += alpha * weight;
}

fn straight(sum: [f32; 4]) -> [u8; 4] {
    if sum[3] <= 0.0 {
        return [0; 4];
    }
    [
        byte(sum[0] / sum[3]),
        byte(sum[1] / sum[3]),
        byte(sum[2] / sum[3]),
        byte(sum[3] * 255.0),
    ]
}

fn sample(pixels: &[u8], width: u32, height: u32, x: f32, y: f32) -> [f32; 4] {
    let x = x.clamp(0.0, width as f32 - 1.0);
    let y = y.clamp(0.0, height as f32 - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let dx = x - x0 as f32;
    let dy = y - y0 as f32;
    let mut result = [0.0; 4];
    accumulate(
        &mut result,
        pixel_at(pixels, width, x0, y0),
        (1.0 - dx) * (1.0 - dy),
    );
    accumulate(
        &mut result,
        pixel_at(pixels, width, x1, y0),
        dx * (1.0 - dy),
    );
    accumulate(
        &mut result,
        pixel_at(pixels, width, x0, y1),
        (1.0 - dx) * dy,
    );
    accumulate(&mut result, pixel_at(pixels, width, x1, y1), dx * dy);
    result
}

fn pixelate(pixels: &[u8], width: u32, height: u32, size: u32) -> Vec<u8> {
    if size == 1 {
        return pixels.to_vec();
    }
    let mut result = pixels.to_vec();
    for top in (0..height).step_by(size as usize) {
        for left in (0..width).step_by(size as usize) {
            let bottom = (top + size).min(height);
            let right = (left + size).min(width);
            let mut sum = [0.0; 4];
            let weight = 1.0 / ((bottom - top) * (right - left)) as f32;
            for y in top..bottom {
                for x in left..right {
                    accumulate(&mut sum, pixel_at(pixels, width, x, y), weight);
                }
            }
            let color = straight(sum);
            for y in top..bottom {
                for x in left..right {
                    let index = ((y * width + x) * 4) as usize;
                    result[index..index + 4].copy_from_slice(&color);
                }
            }
        }
    }
    result
}

fn convolve(
    pixels: &[u8],
    width: u32,
    height: u32,
    kernel: [f32; 9],
    bias: f32,
    gray: bool,
) -> Vec<u8> {
    let mut result = pixels.to_vec();
    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            let mut rgb = [bias; 3];
            for offset_y in -1..=1 {
                for offset_x in -1..=1 {
                    let sample_x = (x as i32 + offset_x).clamp(0, width as i32 - 1) as u32;
                    let sample_y = (y as i32 + offset_y).clamp(0, height as i32 - 1) as u32;
                    let neighbor = pixel_at(pixels, width, sample_x, sample_y);
                    let weight = kernel[((offset_y + 1) * 3 + offset_x + 1) as usize];
                    let alpha = neighbor[3] as f32 / 255.0;
                    for channel in 0..3 {
                        rgb[channel] += weight
                            * (neighbor[channel] as f32 * alpha
                                + pixels[index + channel] as f32 * (1.0 - alpha));
                    }
                }
            }
            if gray {
                rgb = [luminance(rgb); 3];
            }
            for channel in 0..3 {
                result[index + channel] = byte(rgb[channel]);
            }
        }
    }
    result
}

fn motion_blur(pixels: &[u8], width: u32, height: u32, radius: f32, angle: f32) -> Vec<u8> {
    if radius < 0.01 {
        return pixels.to_vec();
    }
    let count = (radius.ceil() as usize * 2 + 1).max(3);
    let (sine, cosine) = angle.sin_cos();
    let mut result = vec![0; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 4];
            for step in 0..count {
                let distance = (step as f32 / (count - 1) as f32 * 2.0 - 1.0) * radius;
                let sampled = sample(
                    pixels,
                    width,
                    height,
                    x as f32 + cosine * distance,
                    y as f32 + sine * distance,
                );
                for channel in 0..4 {
                    sum[channel] += sampled[channel] / count as f32;
                }
            }
            let index = ((y * width + x) * 4) as usize;
            result[index..index + 4].copy_from_slice(&straight(sum));
        }
    }
    result
}

fn median(pixels: &[u8], width: u32, height: u32, radius: i32) -> Vec<u8> {
    if radius == 0 {
        return pixels.to_vec();
    }
    let mut result = pixels.to_vec();
    let mut channels = [Vec::new(), Vec::new(), Vec::new()];
    for y in 0..height {
        for x in 0..width {
            for channel in &mut channels {
                channel.clear();
            }
            for sy in (y as i32 - radius).max(0)..=(y as i32 + radius).min(height as i32 - 1) {
                for sx in (x as i32 - radius).max(0)..=(x as i32 + radius).min(width as i32 - 1) {
                    let neighbor = pixel_at(pixels, width, sx as u32, sy as u32);
                    if neighbor[3] != 0 {
                        for channel in 0..3 {
                            channels[channel].push(neighbor[channel]);
                        }
                    }
                }
            }
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                if !channels[channel].is_empty() {
                    let middle = channels[channel].len() / 2;
                    channels[channel].select_nth_unstable(middle);
                    result[index + channel] = channels[channel][middle];
                }
            }
        }
    }
    result
}

fn random(mut seed: u32) -> f32 {
    // a fixed integer hash makes preview and final application identical.
    seed = seed.wrapping_add(0x9e37_79b9);
    seed = (seed ^ (seed >> 16)).wrapping_mul(0x21f0_aaad);
    seed = (seed ^ (seed >> 15)).wrapping_mul(0x735a_2d97);
    (seed ^ (seed >> 15)) as f64 as f32 / u32::MAX as f32
}

fn vignette(pixels: &[u8], width: u32, height: u32, amount: f32) -> Vec<u8> {
    let mut result = pixels.to_vec();
    for y in 0..height {
        for x in 0..width {
            let nx = (x as f32 + 0.5) * 2.0 / width as f32 - 1.0;
            let ny = (y as f32 + 0.5) * 2.0 / height as f32 - 1.0;
            let radius = (nx * nx + ny * ny) / 2.0;
            let attenuation = 1.0 - amount * radius * radius;
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                result[index + channel] = byte(pixels[index + channel] as f32 * attenuation);
            }
        }
    }
    result
}

fn frost(pixels: &[u8], width: u32, height: u32, amount: f32) -> Vec<u8> {
    if amount == 0.0 {
        return pixels.to_vec();
    }
    let mut result = vec![0; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let angle = random(index.wrapping_mul(2)) * std::f32::consts::TAU;
            let distance = random(index.wrapping_mul(2).wrapping_add(1)).sqrt() * amount;
            let sampled = sample(
                pixels,
                width,
                height,
                x as f32 + angle.cos() * distance,
                y as f32 + angle.sin() * distance,
            );
            result[index as usize * 4..index as usize * 4 + 4].copy_from_slice(&straight(sampled));
        }
    }
    result
}

fn distort(pixels: &[u8], width: u32, height: u32, amount: f32, bulge: bool) -> Vec<u8> {
    if amount == 0.0 {
        return pixels.to_vec();
    }
    let mut result = pixels.to_vec();
    let center_x = (width - 1) as f32 / 2.0;
    let center_y = (height - 1) as f32 / 2.0;
    let radius = width.min(height) as f32 / 2.0;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let distance = dx.hypot(dy);
            let normalized = distance / radius;
            if normalized >= 1.0 || distance == 0.0 {
                continue;
            }
            let (source_x, source_y) = if bulge {
                let exponent = 2.0f32.powf(amount * 2.0);
                let scale = normalized.powf(exponent) / normalized;
                (center_x + dx * scale, center_y + dy * scale)
            } else {
                let angle = dy.atan2(dx) + amount.to_radians() * (1.0 - normalized).powi(2);
                (
                    center_x + distance * angle.cos(),
                    center_y + distance * angle.sin(),
                )
            };
            let index = ((y * width + x) * 4) as usize;
            result[index..index + 4]
                .copy_from_slice(&straight(sample(pixels, width, height, source_x, source_y)));
        }
    }
    result
}

fn spin_blur(pixels: &[u8], width: u32, height: u32, amount: f32, zoom: bool) -> Vec<u8> {
    if amount == 0.0 {
        return pixels.to_vec();
    }
    let center_x = (width - 1) as f32 / 2.0;
    let center_y = (height - 1) as f32 / 2.0;
    let mut result = vec![0; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let mut sum = [0.0; 4];
            for step in 0..32 {
                let t = step as f32 / 31.0;
                let (source_x, source_y) = if zoom {
                    let scale = 1.0 - t * amount * 0.5;
                    (center_x + dx * scale, center_y + dy * scale)
                } else {
                    let angle = (t - 0.5) * amount;
                    let (sine, cosine) = angle.sin_cos();
                    (
                        center_x + dx * cosine - dy * sine,
                        center_y + dx * sine + dy * cosine,
                    )
                };
                let sampled = sample(pixels, width, height, source_x, source_y);
                for channel in 0..4 {
                    sum[channel] += sampled[channel] / 32.0;
                }
            }
            let index = ((y * width + x) * 4) as usize;
            result[index..index + 4].copy_from_slice(&straight(sum));
        }
    }
    result
}

fn sketch(pixels: &[u8], width: u32, height: u32, radius: f32, ink: bool) -> Vec<u8> {
    let gray = map_rgb(pixels, |rgb| [luminance(rgb); 3]);
    let inverse = map_rgb(&gray, |rgb| rgb.map(|channel| 255.0 - channel));
    let blurred = gaussian_blur(&inverse, width, height, radius);
    let mut result = pixels.to_vec();
    for ((output, base), blur) in result
        .chunks_exact_mut(4)
        .zip(gray.chunks_exact(4))
        .zip(blurred.chunks_exact(4))
    {
        let value = if blur[0] == 255 {
            255.0
        } else {
            (base[0] as f32 * 255.0 / (255 - blur[0]) as f32).min(255.0)
        };
        if ink {
            let coverage = if value < 180.0 { value / 180.0 } else { 1.0 };
            for channel in output.iter_mut().take(3) {
                *channel = byte(*channel as f32 * coverage);
            }
        } else {
            output[..3].fill(byte(value));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pixels: &[u8], width: u32, height: u32, name: &str, value: f32, second: f32) -> Vec<u8> {
        apply(pixels, width, height, name, value, second).unwrap()
    }

    #[test]
    fn rejects_malformed_images_and_parameters() {
        assert!(apply(&[], 0, 1, "invert", 0.0, 0.0).is_err());
        assert!(apply(&[0; 3], 1, 1, "invert", 0.0, 0.0).is_err());
        assert!(apply(&[0; 4], 1, 1, "unknown", 0.0, 0.0).is_err());
        assert!(apply(&[0; 4], 1, 1, "invert", f32::NAN, 0.0).is_err());
        assert!(apply(&[0; 4], 1, 1, "invert", 0.0, f32::INFINITY).is_err());
    }

    #[test]
    fn invert_round_trip_and_adjustment_alpha() {
        let pixels = [10, 40, 220, 17, 140, 70, 30, 255];
        let inverted = run(&pixels, 2, 1, "invert", 0.0, 0.0);
        assert_eq!(run(&inverted, 2, 1, "invert", 0.0, 0.0), pixels);
        for name in [
            "grayscale",
            "sepia",
            "auto_levels",
            "brightness_contrast",
            "hue_saturation",
            "posterize",
            "noise",
            "sharpen",
            "emboss",
            "edge_detect",
            "glow",
        ] {
            let result = run(&pixels, 2, 1, name, 2.0, 100.0);
            assert_eq!(result[3], 17, "{name}");
            assert_eq!(result[7], 255, "{name}");
        }
    }

    #[test]
    fn neutral_adjustments_are_identity() {
        let pixels = [23, 67, 198, 130, 234, 70, 150, 255];
        for (name, value, second) in [
            ("brightness_contrast", 0.0, 0.0),
            ("hue_saturation", 0.0, 100.0),
            ("gaussian_blur", 0.0, 0.0),
            ("motion_blur", 0.0, 0.0),
            ("sharpen", 0.0, 0.0),
            ("pixelate", 1.0, 0.0),
            ("noise", 0.0, 0.0),
            ("median", 0.0, 0.0),
        ] {
            assert_eq!(run(&pixels, 2, 1, name, value, second), pixels, "{name}");
        }
    }

    #[test]
    fn blur_does_not_leak_transparent_colors() {
        let pixels = [255, 0, 0, 0, 0, 0, 255, 255, 255, 0, 0, 0];
        for name in ["gaussian_blur", "motion_blur", "pixelate"] {
            let result = run(&pixels, 3, 1, name, 2.0, 0.0);
            for pixel in result.chunks_exact(4).filter(|pixel| pixel[3] != 0) {
                assert_eq!(pixel[0], 0, "{name}");
                assert_eq!(pixel[2], 255, "{name}");
            }
        }
    }

    #[test]
    fn spatial_filters_preserve_constant_images() {
        let pixels = [45, 90, 180, 192].repeat(20);
        for name in [
            "gaussian_blur",
            "motion_blur",
            "median",
            "pixelate",
            "sharpen",
            "zoom_blur",
            "radial_blur",
            "frosted_glass",
        ] {
            assert_eq!(run(&pixels, 5, 4, name, 3.0, 45.0), pixels, "{name}");
        }
    }

    #[test]
    fn hue_rotation_and_desaturation() {
        assert_eq!(
            run(&[255, 0, 0, 180], 1, 1, "hue_saturation", 120.0, 100.0),
            [0, 255, 0, 180]
        );
        assert_eq!(
            run(&[255, 0, 0, 180], 1, 1, "hue_saturation", 0.0, 0.0),
            [255, 255, 255, 180]
        );
    }

    #[test]
    fn median_removes_isolated_outlier() {
        let mut pixels = [20, 30, 40, 255].repeat(9);
        pixels[16..20].copy_from_slice(&[255, 255, 255, 128]);
        let filtered = run(&pixels, 3, 3, "median", 1.0, 0.0);
        assert_eq!(&filtered[16..20], &[20, 30, 40, 128]);
    }

    #[test]
    fn auto_levels_ignores_invisible_pixels() {
        let pixels = [0, 255, 0, 0, 50, 100, 20, 255, 150, 200, 120, 255];
        let result = run(&pixels, 3, 1, "auto_levels", 0.0, 0.0);
        assert_eq!(&result[4..8], &[0, 0, 0, 255]);
        assert_eq!(&result[8..12], &[255, 255, 255, 255]);
    }

    #[test]
    fn noise_is_repeatable_and_changes_pixels() {
        let pixels = [128, 128, 128, 255].repeat(25);
        let first = run(&pixels, 5, 5, "noise", 20.0, 0.0);
        assert_eq!(first, run(&pixels, 5, 5, "noise", 20.0, 0.0));
        assert_ne!(first, pixels);
    }
}
