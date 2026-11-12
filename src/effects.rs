use crate::document::validate_dimensions;

#[derive(Clone, Debug)]
pub struct LevelsSettings {
    pub input_black: u8,
    pub input_white: u8,
    pub gamma: f32,
    pub output_black: u8,
    pub output_white: u8,
}

impl Default for LevelsSettings {
    fn default() -> Self {
        Self {
            input_black: 0,
            input_white: 255,
            gamma: 1.0,
            output_black: 0,
            output_white: 255,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveChannel {
    Rgb,
    Red,
    Green,
    Blue,
    Luminosity,
}

#[derive(Clone, Debug)]
pub struct CurvesSettings {
    pub channel: CurveChannel,
    pub points: Vec<(u8, u8)>,
}

impl Default for CurvesSettings {
    fn default() -> Self {
        Self {
            channel: CurveChannel::Rgb,
            points: vec![(0, 0), (255, 255)],
        }
    }
}

pub fn apply(
    pixels: &[u8],
    width: u32,
    height: u32,
    name: &str,
    value: f32,
    second: f32,
) -> Result<Vec<u8>, String> {
    validate_pixels(pixels, width, height)?;
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
            let multiplier = 2.0f32.powf(value.clamp(-200.0, 200.0) / 100.0);
            map_rgb(pixels, |rgb| rgb.map(|channel| channel * multiplier))
        }
        "highlights_shadows" => {
            let highlights = value.clamp(-100.0, 100.0) / 100.0;
            let shadows = second.clamp(-100.0, 100.0) / 100.0;
            map_rgb(pixels, |rgb| {
                let level = luminance(rgb) / 255.0;
                let amount = highlights * level * level + shadows * (1.0 - level).powi(2);
                let adjusted = if amount >= 0.0 {
                    level + (1.0 - level) * amount
                } else {
                    level * (1.0 + amount)
                };
                set_luminance(rgb, adjusted * 255.0)
            })
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

pub fn apply_levels(
    pixels: &[u8],
    width: u32,
    height: u32,
    settings: &LevelsSettings,
) -> Result<Vec<u8>, String> {
    validate_pixels(pixels, width, height)?;
    if settings.input_black >= settings.input_white {
        return Err("input black must be below input white".into());
    }
    if settings.output_black > settings.output_white {
        return Err("output black must not exceed output white".into());
    }
    if !settings.gamma.is_finite() || !(0.1..=10.0).contains(&settings.gamma) {
        return Err("gamma must be between 0.1 and 10".into());
    }
    let input_range = (settings.input_white - settings.input_black) as f32;
    let output_range = (settings.output_white - settings.output_black) as f32;
    let mut table = [0u8; 256];
    for (value, output) in table.iter_mut().enumerate() {
        let normalized =
            ((value as f32 - settings.input_black as f32) / input_range).clamp(0.0, 1.0);
        *output = byte(
            settings.output_black as f32 + normalized.powf(1.0 / settings.gamma) * output_range,
        );
    }
    Ok(map_rgb(pixels, |rgb| {
        rgb.map(|channel| table[channel as usize] as f32)
    }))
}

pub fn apply_curves(
    pixels: &[u8],
    width: u32,
    height: u32,
    settings: &CurvesSettings,
) -> Result<Vec<u8>, String> {
    validate_pixels(pixels, width, height)?;
    let points = &settings.points;
    if points.len() < 2 || points.len() > 256 {
        return Err("curves require between 2 and 256 control points".into());
    }
    if points[0].0 != 0 || points[points.len() - 1].0 != 255 {
        return Err("curve points must start at input 0 and end at input 255".into());
    }
    if points.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err("curve inputs must be unique and in increasing order".into());
    }
    let table = curve_table(points);
    Ok(map_rgb(pixels, |rgb| match settings.channel {
        CurveChannel::Rgb => rgb.map(|channel| curve_value(&table, channel)),
        CurveChannel::Red => [curve_value(&table, rgb[0]), rgb[1], rgb[2]],
        CurveChannel::Green => [rgb[0], curve_value(&table, rgb[1]), rgb[2]],
        CurveChannel::Blue => [rgb[0], rgb[1], curve_value(&table, rgb[2])],
        CurveChannel::Luminosity => set_luminance(rgb, curve_value(&table, luminance(rgb))),
    }))
}

fn validate_pixels(pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    if pixels.len() != validate_dimensions(width, height)? {
        return Err("effect pixel data does not match the image dimensions".into());
    }
    Ok(())
}

fn curve_table(points: &[(u8, u8)]) -> [f32; 1024] {
    let count = points.len();
    let mut derivatives = vec![0.0f64; count];
    let mut right_hand = vec![0.0f64; count];
    // natural cubic interpolation keeps the curve smooth through every control point.
    for index in 1..count - 1 {
        let left = (points[index].0 - points[index - 1].0) as f64;
        let right = (points[index + 1].0 - points[index].0) as f64;
        let weight = left / (left + right);
        let pivot = weight * derivatives[index - 1] + 2.0;
        let left_slope = (points[index].1 as f64 - points[index - 1].1 as f64) / left;
        let right_slope = (points[index + 1].1 as f64 - points[index].1 as f64) / right;
        derivatives[index] = (weight - 1.0) / pivot;
        right_hand[index] = (6.0 * (right_slope - left_slope) / (left + right)
            - weight * right_hand[index - 1])
            / pivot;
    }
    for index in (0..count - 1).rev() {
        derivatives[index] = derivatives[index] * derivatives[index + 1] + right_hand[index];
    }
    let mut table = [0.0; 1024];
    let mut segment = 0;
    for (index, output) in table.iter_mut().enumerate() {
        let x = index as f64 * 255.0 / 1023.0;
        while segment + 2 < count && x > points[segment + 1].0 as f64 {
            segment += 1;
        }
        let width = (points[segment + 1].0 - points[segment].0) as f64;
        let right = (x - points[segment].0 as f64) / width;
        let left = 1.0 - right;
        let value = left * points[segment].1 as f64
            + right * points[segment + 1].1 as f64
            + ((left * left * left - left) * derivatives[segment]
                + (right * right * right - right) * derivatives[segment + 1])
                * width
                * width
                / 6.0;
        *output = value.clamp(0.0, 255.0) as f32;
    }
    table
}

fn curve_value(table: &[f32; 1024], input: f32) -> f32 {
    let position = input.clamp(0.0, 255.0) * 1023.0 / 255.0;
    let left = position.floor() as usize;
    let right = (left + 1).min(1023);
    table[left] + (table[right] - table[left]) * (position - left as f32)
}

fn set_luminance(rgb: [f32; 3], target: f32) -> [f32; 3] {
    let source = luminance(rgb);
    let target = target.clamp(0.0, 255.0);
    let mut scale = 1.0f32;
    // compress chroma only when the new brightness would put a channel outside the gamut.
    for channel in rgb {
        let difference = channel - source;
        if difference < 0.0 {
            scale = scale.min(target / -difference);
        } else if difference > 0.0 {
            scale = scale.min((255.0 - target) / difference);
        }
    }
    rgb.map(|channel| target + (channel - source) * scale)
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
            "exposure",
            "temperature_tint",
            "highlights_shadows",
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
            ("exposure", 0.0, 0.0),
            ("temperature_tint", 0.0, 0.0),
            ("highlights_shadows", 0.0, 0.0),
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

    #[test]
    fn exposure_uses_hundredths_and_preserves_alpha() {
        let pixels = [20, 40, 80, 117];
        assert_eq!(
            run(&pixels, 1, 1, "exposure", 100.0, 0.0),
            [40, 80, 160, 117]
        );
        assert_eq!(
            run(&pixels, 1, 1, "exposure", -100.0, 0.0),
            [10, 20, 40, 117]
        );
        assert_eq!(
            run(&pixels, 1, 1, "exposure", 900.0, 0.0),
            [80, 160, 255, 117]
        );
    }

    #[test]
    fn highlights_and_shadows_target_opposite_tonal_ranges() {
        let pixels = [32, 32, 32, 91, 224, 224, 224, 230];
        let highlights = run(&pixels, 2, 1, "highlights_shadows", -50.0, 0.0);
        let shadows = run(&pixels, 2, 1, "highlights_shadows", 0.0, 50.0);
        assert!(32 - highlights[0] < 224 - highlights[4]);
        assert!(shadows[0] - 32 > shadows[4] - 224);
        for result in [highlights, shadows] {
            assert_eq!(result[3], 91);
            assert_eq!(result[7], 230);
            assert_eq!(result[0], result[1]);
            assert_eq!(result[4], result[5]);
        }
    }

    #[test]
    fn levels_identity_and_gamma_preserve_alpha() {
        let pixels: Vec<u8> = (0..=255)
            .flat_map(|value| [value, 255 - value, value, value])
            .collect();
        assert_eq!(
            apply_levels(&pixels, 256, 1, &LevelsSettings::default()).unwrap(),
            pixels
        );
        let settings = LevelsSettings {
            gamma: 2.0,
            ..LevelsSettings::default()
        };
        assert_eq!(
            apply_levels(&[0, 64, 255, 73], 1, 1, &settings).unwrap(),
            [0, 128, 255, 73]
        );
    }

    #[test]
    fn levels_clip_input_and_remap_output_bounds() {
        let settings = LevelsSettings {
            input_black: 64,
            input_white: 192,
            gamma: 1.0,
            output_black: 16,
            output_white: 240,
        };
        let pixels = [0, 64, 128, 17, 192, 255, 128, 250];
        assert_eq!(
            apply_levels(&pixels, 2, 1, &settings).unwrap(),
            [16, 16, 128, 17, 240, 240, 128, 250]
        );
        let settings = LevelsSettings {
            output_black: 77,
            output_white: 77,
            ..LevelsSettings::default()
        };
        assert_eq!(
            apply_levels(&pixels, 2, 1, &settings).unwrap(),
            [77, 77, 77, 17, 77, 77, 77, 250]
        );
    }

    #[test]
    fn levels_reject_invalid_dimensions_ranges_and_gamma() {
        assert!(apply_levels(&[0; 3], 1, 1, &LevelsSettings::default()).is_err());
        assert!(apply_levels(&[], 0, 1, &LevelsSettings::default()).is_err());
        for settings in [
            LevelsSettings {
                input_black: 128,
                input_white: 128,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                input_black: 200,
                input_white: 128,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                output_black: 200,
                output_white: 128,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                gamma: f32::NAN,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                gamma: f32::INFINITY,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                gamma: 0.0,
                ..LevelsSettings::default()
            },
            LevelsSettings {
                gamma: 10.1,
                ..LevelsSettings::default()
            },
        ] {
            assert!(apply_levels(&[0; 4], 1, 1, &settings).is_err());
        }
    }

    #[test]
    fn curves_identity_preserves_every_channel_and_alpha() {
        let pixels: Vec<u8> = (0u8..=255)
            .flat_map(|value| [value, 255 - value, value.wrapping_mul(41), value])
            .collect();
        for channel in [
            CurveChannel::Rgb,
            CurveChannel::Red,
            CurveChannel::Green,
            CurveChannel::Blue,
            CurveChannel::Luminosity,
        ] {
            let settings = CurvesSettings {
                channel,
                ..CurvesSettings::default()
            };
            assert_eq!(
                apply_curves(&pixels, 256, 1, &settings).unwrap(),
                pixels,
                "{channel:?}"
            );
        }
    }

    #[test]
    fn curves_channel_selection_and_endpoint_edits() {
        let mut settings = CurvesSettings {
            channel: CurveChannel::Red,
            points: vec![(0, 255), (255, 0)],
        };
        let pixels = [20, 40, 80, 117];
        assert_eq!(
            apply_curves(&pixels, 1, 1, &settings).unwrap(),
            [235, 40, 80, 117]
        );
        settings.channel = CurveChannel::Green;
        assert_eq!(
            apply_curves(&pixels, 1, 1, &settings).unwrap(),
            [20, 215, 80, 117]
        );
        settings.channel = CurveChannel::Blue;
        assert_eq!(
            apply_curves(&pixels, 1, 1, &settings).unwrap(),
            [20, 40, 175, 117]
        );
        settings.channel = CurveChannel::Rgb;
        settings.points = vec![(0, 30), (255, 210)];
        assert_eq!(
            apply_curves(&[0, 255, 0, 117], 1, 1, &settings).unwrap(),
            [30, 210, 30, 117]
        );
    }

    #[test]
    fn curves_interpolate_multiple_points_with_smooth_bends() {
        let points = vec![(0, 0), (64, 120), (128, 64), (192, 220), (255, 255)];
        let pixels: Vec<u8> = points
            .iter()
            .flat_map(|&(input, _)| [input, input, input, 83])
            .collect();
        let settings = CurvesSettings {
            channel: CurveChannel::Rgb,
            points: points.clone(),
        };
        let result = apply_curves(&pixels, points.len() as u32, 1, &settings).unwrap();
        for (pixel, &(_, output)) in result.chunks_exact(4).zip(&points) {
            assert!((pixel[0] as i16 - output as i16).abs() <= 1);
            assert_eq!(pixel[3], 83);
        }
        let settings = CurvesSettings {
            channel: CurveChannel::Rgb,
            points: vec![(0, 0), (128, 200), (255, 255)],
        };
        let middle = apply_curves(&[64, 64, 64, 255], 1, 1, &settings).unwrap();
        assert!(
            middle[0] > 110,
            "the segment should bend rather than use straight lines"
        );
    }

    #[test]
    fn luminosity_curves_retain_color_and_hit_requested_brightness() {
        let settings = CurvesSettings {
            channel: CurveChannel::Luminosity,
            points: vec![(0, 128), (255, 128)],
        };
        let pixels = [255, 0, 0, 71, 0, 0, 255, 190, 0, 0, 0, 250];
        let result = apply_curves(&pixels, 3, 1, &settings).unwrap();
        for (pixel, original) in result.chunks_exact(4).zip(pixels.chunks_exact(4)) {
            assert!(
                (luminance([pixel[0] as f32, pixel[1] as f32, pixel[2] as f32]) - 128.0).abs()
                    <= 1.0
            );
            assert_eq!(pixel[3], original[3]);
        }
        assert!(result[0] > result[1]);
        assert_eq!(result[1], result[2]);
        assert!(result[6] > result[4]);
        assert_eq!(&result[8..12], &[128, 128, 128, 250]);
    }

    #[test]
    fn curves_reject_missing_endpoints_and_repeated_or_unsorted_inputs() {
        assert!(apply_curves(&[0; 3], 1, 1, &CurvesSettings::default()).is_err());
        assert!(apply_curves(&[], 0, 1, &CurvesSettings::default()).is_err());
        for points in [
            vec![],
            vec![(0, 0)],
            vec![(1, 0), (255, 255)],
            vec![(0, 0), (254, 255)],
            vec![(0, 0), (128, 100), (128, 200), (255, 255)],
            vec![(0, 0), (128, 100), (64, 200), (255, 255)],
            vec![(0, 0); 257],
        ] {
            let settings = CurvesSettings {
                channel: CurveChannel::Rgb,
                points,
            };
            assert!(apply_curves(&[0; 4], 1, 1, &settings).is_err());
        }
    }
}
