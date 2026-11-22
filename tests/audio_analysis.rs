//! Checks `analyze_audio` and `compare_audio` against signals with known levels.

use std::{f32::consts::TAU, path::PathBuf};

use hound::{SampleFormat, WavSpec, WavWriter};
use reaper_mcp_rs::audio;

const RATE: u32 = 48_000;

struct TempWav(PathBuf);

impl Drop for TempWav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Writes a stereo sine; `right_gain` scales the right channel relative to the left.
fn sine(freq: f32, amplitude: f32, seconds: f32, right_gain: f32, float: bool) -> TempWav {
    let path = std::env::temp_dir().join(format!("reaper-mcp-{}.wav", uuid::Uuid::new_v4()));
    let spec = WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: if float { 32 } else { 16 },
        sample_format: if float {
            SampleFormat::Float
        } else {
            SampleFormat::Int
        },
    };
    let mut writer = WavWriter::create(&path, spec).unwrap();
    for n in 0..(seconds * RATE as f32) as usize {
        let v = amplitude * (TAU * freq * n as f32 / RATE as f32).sin();
        for sample in [v, v * right_gain] {
            if float {
                writer.write_sample(sample).unwrap();
            } else {
                writer
                    .write_sample((sample * i16::MAX as f32).round() as i16)
                    .unwrap();
            }
        }
    }
    writer.finalize().unwrap();
    TempWav(path)
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

#[test]
fn sine_levels_match_theory() {
    let wav = sine(1000.0, 0.5, 10.0, 1.0, true);
    let a = audio::analyze(&audio::read(&wav.0, None, None).unwrap());
    near(a.seconds, 10.0, 1e-3);
    assert_eq!(a.sample_rate, RATE);
    assert_eq!(a.channels, 2);
    // Amplitude 0.5: peak -6.02 dBFS, RMS 3.01 dB lower, crest factor of a sine.
    near(a.peak_dbfs, -6.02, 0.01);
    near(a.rms_dbfs, -9.03, 0.01);
    near(a.crest_db, 3.01, 0.01);
    // BS.1770: a 1 kHz sine at -6.02 dBFS in both channels reads about -6 LUFS.
    near(a.integrated_lufs, -6.0, 0.5);
    near(a.loudness_range_lu, 0.0, 0.5);
    assert_eq!(a.overloaded_samples, 0);
    assert!(a.warnings.is_empty(), "{:?}", a.warnings);
    // Identical channels have no side signal.
    assert!(a.side_to_mid_db < -60.0);
    // The energy sits in the 1 kHz third-octave band.
    let loudest = a
        .third_octave
        .iter()
        .max_by(|x, y| x.db.total_cmp(&y.db))
        .unwrap();
    assert_eq!(loudest.name, "1000");
}

#[test]
fn a_time_range_reads_only_that_part() {
    let wav = sine(440.0, 0.25, 6.0, 1.0, false);
    let part = audio::read(&wav.0, Some(2.0), Some(4.5)).unwrap();
    near(part.seconds(), 2.5, 1e-3);
    near(audio::analyze(&part).peak_dbfs, -12.04, 0.05);
}

#[test]
fn float_overloads_are_counted_and_reported() {
    let wav = sine(1000.0, 1.5, 2.0, 1.0, true);
    let a = audio::analyze(&audio::read(&wav.0, None, None).unwrap());
    near(a.peak_dbfs, 3.52, 0.01);
    assert!(a.overloaded_samples > 0);
    assert!(a.warnings.iter().any(|w| w.contains("overloads")));
}

#[test]
fn compare_separates_loudness_from_tone() {
    let loud = sine(1000.0, 0.5, 5.0, 1.0, true);
    let quiet = sine(1000.0, 0.25, 5.0, 1.0, true);
    let c = audio::compare(
        &audio::read(&loud.0, None, None).unwrap(),
        &audio::read(&quiet.0, None, None).unwrap(),
    );
    near(c.loudness_difference_lu, -6.02, 0.1);
    // After matching loudness the same tone shows no band difference.
    let band = c.third_octave.iter().find(|b| b.name == "1000").unwrap();
    near(band.difference_db, 0.0, 0.1);
}

#[test]
fn stereo_width_shows_in_side_to_mid() {
    // Right channel inverted: all side, no mid.
    let wide = sine(1000.0, 0.5, 2.0, -1.0, true);
    let a = audio::analyze(&audio::read(&wide.0, None, None).unwrap());
    assert!(a.side_to_mid_db > 60.0);
}
