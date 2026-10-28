//! Local audio device discovery, microphone metering, and output preview.

use crate::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Provides local audio diagnostics without requiring a network session.
#[derive(Debug, Default, Clone, Copy)]
pub struct AudioDevices;

impl AudioDevices {
    /// Lists input devices by the names reported by the operating system.
    pub fn inputs(&self) -> Result<Vec<String>> {
        let mut names = cpal::default_host()
            .input_devices()?
            .filter_map(|device| device.name().ok())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Lists output devices by the names reported by the operating system.
    pub fn outputs(&self) -> Result<Vec<String>> {
        let mut names = cpal::default_host()
            .output_devices()?
            .filter_map(|device| device.name().ok())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Starts a microphone level meter for the selected input device.
    pub fn monitor(&self, name: &str, gain_db: i8) -> Result<MicrophoneMonitor> {
        let device = input_device(name)?;
        let format = device.default_input_config()?;
        let level = Arc::new(AtomicU32::new(0));
        let gain = 10.0_f32.powf(f32::from(gain_db) / 20.0);
        let stream = match format.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(&device, &format.into(), level.clone(), gain)?,
            SampleFormat::I16 => input_stream::<i16>(&device, &format.into(), level.clone(), gain)?,
            SampleFormat::U16 => input_stream::<u16>(&device, &format.into(), level.clone(), gain)?,
            _ => return Err("microphone sample format is unsupported".into()),
        };
        stream.play()?;
        Ok(MicrophoneMonitor {
            _stream: stream,
            level,
        })
    }

    /// Plays a short test tone through the selected output device.
    pub fn preview(&self, name: &str, volume_db: i8) -> Result<()> {
        let device = output_device(name)?;
        let format = device.default_output_config()?;
        let stream = match format.sample_format() {
            SampleFormat::F32 => output_stream::<f32>(&device, &format.into(), volume_db)?,
            SampleFormat::I16 => output_stream::<i16>(&device, &format.into(), volume_db)?,
            SampleFormat::U16 => output_stream::<u16>(&device, &format.into(), volume_db)?,
            _ => return Err("output sample format is unsupported".into()),
        };
        stream.play()?;
        std::thread::sleep(Duration::from_millis(350));
        Ok(())
    }

    /// Routes the selected microphone to the selected output for local monitoring.
    pub fn listen(
        &self,
        microphone: &str,
        output: &str,
        mic_gain_db: i8,
        output_gain_db: i8,
    ) -> Result<MicrophoneLoopback> {
        let input = input_device(microphone)?;
        let output = output_device(output)?;
        let input_format = input.default_input_config()?;
        let output_format = output.default_output_config()?;
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let level = Arc::new(AtomicU32::new(0));
        let capacity = (input_format.sample_rate().0 / 5) as usize;
        let mic_gain = 10.0_f32.powf(f32::from(mic_gain_db) / 20.0);
        let input_stream = match input_format.sample_format() {
            SampleFormat::F32 => loopback_input::<f32>(
                &input,
                &input_format.config(),
                queue.clone(),
                level.clone(),
                capacity,
                mic_gain,
            )?,
            SampleFormat::I16 => loopback_input::<i16>(
                &input,
                &input_format.config(),
                queue.clone(),
                level.clone(),
                capacity,
                mic_gain,
            )?,
            SampleFormat::U16 => loopback_input::<u16>(
                &input,
                &input_format.config(),
                queue.clone(),
                level.clone(),
                capacity,
                mic_gain,
            )?,
            _ => return Err("microphone sample format is unsupported".into()),
        };
        let output_gain = 10.0_f32.powf(f32::from(output_gain_db) / 20.0);
        let input_rate = input_format.sample_rate().0;
        let output_stream = match output_format.sample_format() {
            SampleFormat::F32 => loopback_output::<f32>(
                &output,
                &output_format.config(),
                queue,
                output_gain,
                input_rate,
            )?,
            SampleFormat::I16 => loopback_output::<i16>(
                &output,
                &output_format.config(),
                queue,
                output_gain,
                input_rate,
            )?,
            SampleFormat::U16 => loopback_output::<u16>(
                &output,
                &output_format.config(),
                queue,
                output_gain,
                input_rate,
            )?,
            _ => return Err("output sample format is unsupported".into()),
        };
        input_stream.play()?;
        output_stream.play()?;
        Ok(MicrophoneLoopback {
            _input: input_stream,
            _output: output_stream,
            level,
        })
    }
}

fn input_device(name: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        host.default_input_device()
    } else {
        host.input_devices()?
            .find(|device| device.name().ok().as_deref() == Some(name))
    }
    .ok_or_else(|| "selected microphone is unavailable".into())
}

fn output_device(name: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|device| device.name().ok().as_deref() == Some(name))
    }
    .ok_or_else(|| "selected output device is unavailable".into())
}

/// Keeps an input stream open and exposes its latest peak level.
pub struct MicrophoneMonitor {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
}

impl MicrophoneMonitor {
    /// Returns the latest peak amplitude, clamped to the range 0.0 to 1.0.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }
}

/// Keeps both local monitoring streams alive until listening is stopped.
pub struct MicrophoneLoopback {
    _input: cpal::Stream,
    _output: cpal::Stream,
    level: Arc<AtomicU32>,
}

impl MicrophoneLoopback {
    /// Returns the latest microphone peak after input gain.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }
}

fn loopback_input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Arc<Mutex<VecDeque<f32>>>,
    level: Arc<AtomicU32>,
    capacity: usize,
    gain: f32,
) -> Result<cpal::Stream>
where
    T: SizedSample + Copy,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut peak = 0.0_f32;
            if let Ok(mut samples) = queue.lock() {
                for frame in data.chunks(channels) {
                    let value = frame
                        .iter()
                        .map(|sample| f32::from_sample(*sample))
                        .sum::<f32>()
                        / frame.len() as f32
                        * gain;
                    peak = peak.max(value.abs());
                    if samples.len() >= capacity {
                        samples.pop_front();
                    }
                    samples.push_back(value);
                }
            }
            level.store(peak.min(1.0).to_bits(), Ordering::Relaxed);
        },
        |_| {},
        None,
    )?)
}

fn loopback_output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Arc<Mutex<VecDeque<f32>>>,
    gain: f32,
    input_rate: u32,
) -> Result<cpal::Stream>
where
    T: SizedSample + Sample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let ratio = f64::from(input_rate) / f64::from(config.sample_rate.0);
    let mut reader = LinearReader::default();
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            if let Ok(mut samples) = queue.lock() {
                for frame in data.chunks_mut(channels) {
                    let value = T::from_sample(
                        (reader.read(&mut samples, ratio) * gain).clamp(-0.95, 0.95),
                    );
                    for sample in frame {
                        *sample = value;
                    }
                }
            }
        },
        |_| {},
        None,
    )?)
}

#[derive(Default)]
struct LinearReader {
    current: f32,
    next: f32,
    fraction: f64,
    primed: bool,
}

impl LinearReader {
    fn read(&mut self, samples: &mut VecDeque<f32>, ratio: f64) -> f32 {
        if !self.primed {
            if samples.len() < 2 {
                return 0.0;
            }
            self.current = samples.pop_front().unwrap_or(0.0);
            self.next = samples.pop_front().unwrap_or(0.0);
            self.primed = true;
        }
        let value = self.current + (self.next - self.current) * self.fraction as f32;
        self.fraction += ratio;
        while self.fraction >= 1.0 {
            self.current = self.next;
            self.next = samples.pop_front().unwrap_or(0.0);
            self.fraction -= 1.0;
        }
        value
    }
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    level: Arc<AtomicU32>,
    gain: f32,
) -> Result<cpal::Stream>
where
    T: SizedSample + Copy,
    f32: FromSample<T>,
{
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let peak = data
                .iter()
                .map(|sample| f32::from_sample(*sample).abs() * gain)
                .fold(0.0_f32, f32::max)
                .min(1.0);
            level.store(peak.to_bits(), Ordering::Relaxed);
        },
        |_| {},
        None,
    )?)
}

fn output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    volume_db: i8,
) -> Result<cpal::Stream>
where
    T: SizedSample + Sample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let step = 440.0 * std::f32::consts::TAU / config.sample_rate.0 as f32;
    let amplitude = (0.12 * 10.0_f32.powf(f32::from(volume_db) / 20.0)).min(0.8);
    let mut phase = 0.0_f32;
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(channels) {
                let value = T::from_sample(phase.sin() * amplitude);
                for sample in frame {
                    *sample = value;
                }
                phase = (phase + step) % std::f32::consts::TAU;
            }
        },
        |_| {},
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SettingsStore;

    #[test]
    fn interpolates_different_audio_rates() {
        let mut reader = LinearReader::default();
        let mut samples = VecDeque::from([0.0, 1.0, 0.0]);
        assert_eq!(reader.read(&mut samples, 0.5), 0.0);
        assert_eq!(reader.read(&mut samples, 0.5), 0.5);
        assert_eq!(reader.read(&mut samples, 0.5), 1.0);
    }

    #[test]
    #[ignore = "requires local microphone and output hardware"]
    fn opens_selected_microphone_loopback() {
        let settings = SettingsStore::discover().unwrap().load().unwrap();
        let loopback = AudioDevices
            .listen(
                &settings.microphone_device,
                &settings.output_device,
                settings.microphone_gain_db,
                settings.output_volume_db,
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        println!("Microphone peak: {}", loopback.level());
    }
}
