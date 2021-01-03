use blip_buf::BlipBuf;
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Sound {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    blip: Arc<Mutex<BlipBuf>>,
    stream: Option<cpal::Stream>,
    fs_input: f64,
}

impl Sound {
    pub fn new(fs_input: f64) -> Self {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("failed to find a default output device");
        let config = device.default_output_config().unwrap();
        println!("{:?}", config.sample_rate());

        let mut blip = BlipBuf::new(config.sample_rate().0);
        blip.set_rates(fs_input, config.sample_rate().0 as f64);
        let blip = Arc::new(Mutex::new(blip));

        Self {
            device,
            config,
            blip,
            stream: None,
            fs_input,
        }
    }

    pub fn start(&mut self) {
        let result = match self.config.sample_format() {
            cpal::SampleFormat::F32 => self._run::<f32>().unwrap(),
            cpal::SampleFormat::I16 => self._run::<i16>().unwrap(),
            cpal::SampleFormat::U16 => self._run::<u16>().unwrap(),
        };
        self.stream = Some(result);
    }

    pub fn stop(&mut self) {
        self.stream = None;
    }

    pub fn play_samples(&mut self, samples: &[i16], duration: Duration) {
        let mut blip = self.blip.lock().unwrap();

        blip.clear();
        let mut time = 0usize; // takes count of how many samples were written in the current frame
        let samples_needed = (duration.as_secs_f64() * self.fs_input) as usize;
        let samples_chunksize = (0.00166 * self.fs_input) as usize;
        let mut samples_written = 0usize;

        while samples_written < samples_needed {
            while time < samples_chunksize && samples_written < samples_needed {
                blip.add_delta(time as u32, samples[samples_written % samples.len()] as i32);
                time += 1;
                samples_written += 1;
            }
            blip.end_frame(time as u32);
            time = 0;
        }
    }

    fn _run<T>(&mut self) -> Result<cpal::Stream, anyhow::Error>
    where
        T: cpal::Sample,
    {
        let config: &cpal::StreamConfig = &self.config.clone().into();
        let channels = config.channels as usize;

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let blip = self.blip.clone();
        let stream = self.device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, blip.clone())
            },
            err_fn,
        )?;
        Ok(stream)
    }
}

fn main() {
    let mut sound = Sound::new(4000.0);
    sound.start();
    loop {
        let samples: [i16; 4] = [-10000, -10000, 10000, 10000];
        sound.play_samples(&samples[..], Duration::from_millis(500));
        std::thread::sleep(Duration::from_millis(1000));
    }
}

fn write_data<T>(output: &mut [T], channels: usize, blip: Arc<Mutex<BlipBuf>>)
where
    T: cpal::Sample,
{
    let mut blip = blip.lock().unwrap();

    let mut buf = vec![0i16; output.len() / 2];
    let mut read = 0usize;
    while blip.samples_avail() > 0 && buf[read..].len() > 0 {
        read += blip.read_samples(&mut buf[read..], false);
    }

    println!(
        "samples_avail: {}; buf[read..].len(): {}",
        blip.samples_avail(),
        buf[read..].len()
    );

    output
        .chunks_mut(channels)
        .zip(buf.iter())
        .for_each(|(out, in_buf)| {
            let sample: T = cpal::Sample::from::<i16>(in_buf);
            for ch in out {
                *ch = sample;
            }
        });
}
