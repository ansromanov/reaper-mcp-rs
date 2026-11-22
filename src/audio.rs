//! Offline analysis of rendered audio: loudness, peaks, overloads and a third-octave spectrum.
//!
//! The numbers follow what a mixing session needs to compare two renders: integrated loudness
//! (ITU-R BS.1770 K-weighting with gating), sample peak, samples above full scale, crest factor,
//! stereo width and the long-term power spectrum in third-octave bands.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use rustfft::{num_complex::Complex, FftPlanner};
use serde::Serialize;

/// Third-octave band centres from 20 Hz to 20 kHz.
pub const THIRD_OCTAVE: [f64; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0,
    630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0,
    10000.0, 12500.0, 16000.0, 20000.0,
];

/// Coarse bands used in mixing notes: name, low edge, high edge in Hz.
pub const MIX_BANDS: [(&str, f64, f64); 9] = [
    ("sub 20-60", 20.0, 60.0),
    ("bass 60-120", 60.0, 120.0),
    ("low 120-250", 120.0, 250.0),
    ("lowmid 250-500", 250.0, 500.0),
    ("mid 500-1k", 500.0, 1000.0),
    ("himid 1-2k", 1000.0, 2000.0),
    ("presence 2-4k", 2000.0, 4000.0),
    ("bright 4-8k", 4000.0, 8000.0),
    ("air 8-16k", 8000.0, 16000.0),
];

const WELCH_SIZE: usize = 1 << 15;

pub struct Audio {
    pub rate: u32,
    /// One vector per channel.
    pub channels: Vec<Vec<f32>>,
    /// True for files stored as integers: samples at full scale were cut off when written.
    pub integer: bool,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.rate)
    }

    fn mono(&self) -> Vec<f32> {
        let count = self.channels.len().max(1) as f32;
        (0..self.frames())
            .map(|i| self.channels.iter().map(|c| c[i]).sum::<f32>() / count)
            .collect()
    }
}

/// Reads a WAV file (16, 24 or 32-bit integer or 32-bit float), optionally only a time range.
pub fn read(path: &Path, start_seconds: Option<f64>, end_seconds: Option<f64>) -> Result<Audio> {
    let mut reader = hound::WavReader::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    let total = reader.duration() as u64;
    let start = (start_seconds.unwrap_or(0.0).max(0.0) * f64::from(spec.sample_rate)) as u64;
    let end = end_seconds
        .map(|e| (e * f64::from(spec.sample_rate)) as u64)
        .unwrap_or(total)
        .min(total);
    if start >= end {
        return Err(anyhow!("empty time range: file has {total} frames, range is {start}..{end}"));
    }
    reader.seek(start as u32).context("cannot seek in the file")?;
    let wanted = (end - start) as usize;
    let mut data: Vec<Vec<f32>> = vec![Vec::with_capacity(wanted); channels];
    let integer = spec.sample_format == hound::SampleFormat::Int;
    let mut index = 0usize;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>().take(wanted * channels) {
                data[index % channels].push(sample?);
                index += 1;
            }
        }
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            for sample in reader.samples::<i32>().take(wanted * channels) {
                data[index % channels].push(sample? as f32 * scale);
                index += 1;
            }
        }
    }
    Ok(Audio { rate: spec.sample_rate, channels: data, integer })
}

fn db(power: f64) -> f64 {
    10.0 * power.max(1e-20).log10()
}

#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
}

impl Biquad {
    fn run(&self, input: &[f32]) -> Vec<f64> {
        let (mut z1, mut z2) = (0.0f64, 0.0f64);
        input
            .iter()
            .map(|&x| {
                let x = f64::from(x);
                let y = self.b[0] * x + z1;
                z1 = self.b[1] * x - self.a[0] * y + z2;
                z2 = self.b[2] * x - self.a[1] * y;
                y
            })
            .collect()
    }
}

/// BS.1770 K-weighting: a high shelf followed by a high-pass.
fn k_weighting(rate: f64) -> [Biquad; 2] {
    let (f0, gain_db, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let vh = 10f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high_pass = Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0] };
    [shelf, high_pass]
}

/// Integrated loudness in LUFS and the loudness range in LU.
fn loudness(audio: &Audio) -> (f64, f64) {
    let rate = f64::from(audio.rate);
    let [shelf, high_pass] = k_weighting(rate);
    let weighted: Vec<Vec<f64>> = audio
        .channels
        .iter()
        .map(|c| {
            let first: Vec<f32> = shelf.run(c).into_iter().map(|v| v as f32).collect();
            high_pass.run(&first)
        })
        .collect();
    let frames = audio.frames();
    let block = (0.4 * rate) as usize;
    let step = (0.1 * rate) as usize;
    if frames < block {
        return (f64::NAN, 0.0);
    }
    let mut cumulative = vec![0.0f64; frames + 1];
    for i in 0..frames {
        cumulative[i + 1] = cumulative[i] + weighted.iter().map(|c| c[i] * c[i]).sum::<f64>();
    }
    let blocks: Vec<f64> = (0..=frames - block)
        .step_by(step)
        .map(|s| (cumulative[s + block] - cumulative[s]) / block as f64)
        .collect();
    let lufs = |power: f64| -0.691 + db(power);
    let absolute: Vec<f64> = blocks.iter().copied().filter(|&p| lufs(p) > -70.0).collect();
    if absolute.is_empty() {
        return (f64::NAN, 0.0);
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let relative_gate = lufs(mean(&absolute)) - 10.0;
    let gated: Vec<f64> = blocks.iter().copied().filter(|&p| lufs(p) > relative_gate).collect();
    let integrated = lufs(mean(&gated));
    // Loudness range from 3 s windows, one per second, above the relative gate.
    let window = (3.0 * rate) as usize;
    let mut shorts: Vec<f64> = Vec::new();
    let mut s = 0usize;
    while s + window <= frames {
        let power = (cumulative[s + window] - cumulative[s]) / window as f64;
        let l = lufs(power);
        if l > relative_gate {
            shorts.push(l);
        }
        s += rate as usize;
    }
    shorts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let percentile = |p: f64| shorts[((shorts.len() - 1) as f64 * p) as usize];
    let range = if shorts.len() > 2 { percentile(0.95) - percentile(0.10) } else { 0.0 };
    (integrated, range)
}

/// Long-term power spectrum (Welch, Hann, 50 % overlap) of the channel average. Returns the power
/// per bin and the bin width in Hz.
fn spectrum(audio: &Audio) -> (Vec<f64>, f64) {
    let mono = audio.mono();
    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(WELCH_SIZE);
    let window: Vec<f32> = (0..WELCH_SIZE)
        .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / (WELCH_SIZE as f32 - 1.0)).cos())
        .collect();
    let window_power: f64 = window.iter().map(|w| f64::from(*w * *w)).sum();
    let mut accumulated = vec![0.0f64; WELCH_SIZE / 2 + 1];
    let mut count = 0usize;
    let mut start = 0usize;
    let mut buffer = vec![Complex::new(0.0f32, 0.0); WELCH_SIZE];
    while start + WELCH_SIZE <= mono.len() {
        for n in 0..WELCH_SIZE {
            buffer[n] = Complex::new(mono[start + n] * window[n], 0.0);
        }
        fft.process(&mut buffer);
        for (bin, value) in buffer.iter().take(WELCH_SIZE / 2 + 1).enumerate() {
            accumulated[bin] += f64::from(value.norm_sqr()) / window_power;
        }
        count += 1;
        start += WELCH_SIZE / 2;
    }
    if count > 0 {
        for v in &mut accumulated {
            *v /= count as f64;
        }
    }
    (accumulated, f64::from(audio.rate) / WELCH_SIZE as f64)
}

/// Share of the signal power in a band, as linear power where a full-scale sine is 0.5 (-3 dBFS).
/// Each bin of the averaged spectrum holds |X|^2 / sum(w^2), which is the signal variance for white
/// noise, so a band's power is the sum over its bins divided by the FFT size, doubled for the
/// one-sided spectrum.
fn band_power(power: &[f64], bin_hz: f64, low: f64, high: f64) -> f64 {
    let first = (low / bin_hz).ceil() as usize;
    let last = ((high / bin_hz).ceil() as usize).min(power.len());
    2.0 * power[first.min(last)..last].iter().sum::<f64>() / WELCH_SIZE as f64
}

#[derive(Serialize)]
pub struct Band {
    pub name: String,
    pub db: f64,
}

#[derive(Serialize)]
pub struct Analysis {
    pub seconds: f64,
    pub sample_rate: u32,
    pub channels: usize,
    pub integrated_lufs: f64,
    pub loudness_range_lu: f64,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    pub crest_db: f64,
    /// Samples above full scale (float files) or at full scale (integer files, already cut off).
    pub overloaded_samples: usize,
    pub overloaded_percent: f64,
    /// Side energy relative to mid energy in dB (very negative is mono).
    pub side_to_mid_db: f64,
    pub bands: Vec<Band>,
    pub third_octave: Vec<Band>,
    pub warnings: Vec<String>,
}

pub fn analyze(audio: &Audio) -> Analysis {
    let frames = audio.frames().max(1);
    let all = audio.channels.iter().flatten();
    let peak = all.clone().fold(0.0f32, |m, v| m.max(v.abs())) as f64;
    let squares: f64 = all.clone().map(|v| f64::from(*v) * f64::from(*v)).sum();
    let total_samples = (frames * audio.channels.len().max(1)) as f64;
    let rms = (squares / total_samples).sqrt();
    let threshold = if audio.integer { 0.9999 } else { 1.0 };
    let over = all.filter(|v| v.abs() >= threshold && (audio.integer || v.abs() > threshold)).count();
    let side_to_mid = if audio.channels.len() >= 2 {
        let (mut mid, mut side) = (0.0f64, 0.0f64);
        for i in 0..frames {
            let (l, r) = (f64::from(audio.channels[0][i]), f64::from(audio.channels[1][i]));
            mid += ((l + r) / 2.0).powi(2);
            side += ((l - r) / 2.0).powi(2);
        }
        db(side) - db(mid)
    } else {
        f64::NEG_INFINITY
    };
    let (lufs, lra) = loudness(audio);
    let (power, bin_hz) = spectrum(audio);
    let bands = MIX_BANDS
        .iter()
        .map(|(name, low, high)| Band { name: (*name).to_string(), db: db(band_power(&power, bin_hz, *low, *high)) })
        .collect();
    let third_octave = THIRD_OCTAVE
        .iter()
        .map(|c| Band {
            name: format!("{c}"),
            db: db(band_power(&power, bin_hz, c / 2f64.powf(1.0 / 6.0), c * 2f64.powf(1.0 / 6.0))),
        })
        .collect();
    let peak_db = 20.0 * peak.max(1e-12).log10();
    let mut warnings = Vec::new();
    if over > 0 {
        warnings.push(format!(
            "{over} samples at or above full scale ({}); levels above 0 dBFS are {}",
            if audio.integer { "integer file" } else { "float file" },
            if audio.integer { "already cut off in this file, render as float32 to see the real peak" } else { "overloads" }
        ));
    }
    if peak_db > -0.1 {
        warnings.push(format!("peak {peak_db:.1} dBFS leaves no headroom"));
    }
    Analysis {
        seconds: audio.seconds(),
        sample_rate: audio.rate,
        channels: audio.channels.len(),
        integrated_lufs: lufs,
        loudness_range_lu: lra,
        peak_dbfs: peak_db,
        rms_dbfs: 20.0 * rms.max(1e-12).log10(),
        crest_db: peak_db - 20.0 * rms.max(1e-12).log10(),
        overloaded_samples: over,
        overloaded_percent: 100.0 * over as f64 / total_samples,
        side_to_mid_db: side_to_mid,
        bands,
        third_octave,
        warnings,
    }
}

#[derive(Serialize)]
pub struct BandDifference {
    pub name: String,
    pub a_db: f64,
    pub b_db: f64,
    /// b minus a after matching the loudness of b to a: positive means b has more energy there.
    pub difference_db: f64,
}

#[derive(Serialize)]
pub struct Comparison {
    pub a: Analysis,
    pub b: Analysis,
    pub loudness_difference_lu: f64,
    pub crest_difference_db: f64,
    pub side_to_mid_difference_db: f64,
    pub bands: Vec<BandDifference>,
    pub third_octave: Vec<BandDifference>,
}

fn differences(a: &[Band], b: &[Band], loudness_offset: f64) -> Vec<BandDifference> {
    a.iter()
        .zip(b)
        .map(|(x, y)| BandDifference {
            name: x.name.clone(),
            a_db: x.db,
            b_db: y.db,
            difference_db: y.db - x.db + loudness_offset,
        })
        .collect()
}

pub fn compare(a: &Audio, b: &Audio) -> Comparison {
    let (first, second) = (analyze(a), analyze(b));
    // b is turned to the loudness of a, so the differences describe tone and not volume.
    let offset = first.integrated_lufs - second.integrated_lufs;
    let offset = if offset.is_finite() { offset } else { 0.0 };
    Comparison {
        loudness_difference_lu: second.integrated_lufs - first.integrated_lufs,
        crest_difference_db: second.crest_db - first.crest_db,
        side_to_mid_difference_db: second.side_to_mid_db - first.side_to_mid_db,
        bands: differences(&first.bands, &second.bands, offset),
        third_octave: differences(&first.third_octave, &second.third_octave, offset),
        a: first,
        b: second,
    }
}
