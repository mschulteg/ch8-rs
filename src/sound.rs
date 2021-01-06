use anyhow::Context;
use blip_buf::BlipBuf;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, SendError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct Sound {
    fs_input: f64,
    audio_stream: Option<AudioStream>,
}

pub struct AudioStream {
    blip: Arc<Mutex<BlipBuf>>,
    tx_stop: SyncSender<()>,
    thread: thread::JoinHandle<Result<(), anyhow::Error>>,
}

impl Sound {
    pub fn new(fs_input: f64) -> Self {
        Self {
            fs_input,
            audio_stream: None,
        }
    }

    pub fn start(&mut self) -> Result<(), anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("failed to find a default output device");
        let config = device
            .default_output_config()
            .context("Could not find default output config")?;

        // setup blip with enough sample space for the maximum tone duration of 255/60 seconds.
        let mut blip = BlipBuf::new(config.sample_rate().0 * 256 / 60);
        blip.set_rates(self.fs_input, config.sample_rate().0 as f64);
        let blip = Arc::new(Mutex::new(blip));

        let (tx_stop, rx_stop) = mpsc::sync_channel::<()>(1);
        // Create second sender to stop stream thread from cpal error callback function
        let tx_stop2 = tx_stop.clone();

        let thread = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                self._run::<f32>(device, config.into(), rx_stop, tx_stop2, blip.clone())
            }
            cpal::SampleFormat::I16 => {
                self._run::<i16>(device, config.into(), rx_stop, tx_stop2, blip.clone())
            }
            cpal::SampleFormat::U16 => {
                self._run::<u16>(device, config.into(), rx_stop, tx_stop2, blip.clone())
            }
        }?;
        self.audio_stream = Some(AudioStream {
            blip,
            tx_stop,
            thread,
        });
        Ok(())
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) -> Result<(), anyhow::Error> {
        let audio_stream = std::mem::replace(&mut self.audio_stream, None);
        if let Some(audio_stream) = audio_stream {
            match audio_stream.tx_stop.send(()) {
                Ok(..) => {}
                Err(SendError(..)) => {}
            };
            audio_stream.thread.join().unwrap()?;
        }
        Ok(())
    }

    pub fn play_samples_1bit(&mut self, samples: &[u8], duration: Duration) {
        let mut samples_conv = [0i16; 16 * 8];
        for (batch, inp) in samples_conv.chunks_mut(8).zip(samples.iter()) {
            for (i, outp) in batch.iter_mut().enumerate() {
                *outp = (((*inp >> (7 - i)) & 0x1) as i16 * 2 - 1) * 10000;
            }
        }
        self.play_samples(&samples_conv[..], duration);
    }

    pub fn play_samples(&mut self, samples: &[i16], duration: Duration) {
        let audio_stream = self.audio_stream.as_ref().unwrap();
        let mut blip = audio_stream.blip.lock().unwrap();

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

    fn _run<T>(
        &mut self,
        device: cpal::Device,
        config: cpal::StreamConfig,
        rx_stop: Receiver<()>,
        tx_stop: SyncSender<()>,
        blip: Arc<Mutex<BlipBuf>>,
    ) -> Result<thread::JoinHandle<Result<(), anyhow::Error>>, anyhow::Error>
    where
        T: cpal::Sample,
    {
        let err_fn = move |err| {
            match tx_stop.send(()) {
                Ok(..) => {}
                Err(SendError(..)) => {}
            };
            eprintln!("an error occurred on stream: {}", err)
        };

        let channels = config.channels as usize;

        let thread = thread::spawn(move || -> Result<(), anyhow::Error> {
            // Create stream in its own thread so that we can safe it in scope and do not
            // need to save it in Sound, which would make both Sound and CPU !Send
            let stream = device.build_output_stream(
                &config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    write_data(data, channels as usize, blip.clone())
                },
                err_fn,
            )?;
            stream.play()?;
            rx_stop.recv()?;
            Ok(())
        });
        Ok(thread)
    }
}

fn write_data<T>(output: &mut [T], channels: usize, blip: Arc<Mutex<BlipBuf>>)
where
    T: cpal::Sample,
{
    let mut blip = blip.lock().unwrap();

    let mut buf = vec![0i16; output.len() / 2];
    let mut read = 0usize;
    while blip.samples_avail() > 0 && !buf[read..].is_empty() {
        read += blip.read_samples(&mut buf[read..], false);
    }

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
