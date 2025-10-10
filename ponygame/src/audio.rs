use rodio::{buffer, mixer::Mixer, OutputStream, Source};

enum AudioBackend {
    Open(OutputStream),
    None,

    /// State where the AudioBackend is currently waiting for a gesture from
    /// the user before it will start playing. Mainly relevant on web.
    WaitForGesture,
}

pub struct Audio {
    backend: AudioBackend,
}

/// Represents a single Sound asset that can be played. Used for sounds where
/// the sample data is loaded entirely ahead-of-time; primarily useful for sound
/// effects.
pub struct Sound {
    buffer: rodio::buffer::SamplesBuffer,
}

impl Sound {
    pub fn from_data(data: &'static [u8]) -> Sound {
        let cursor = std::io::Cursor::new(data);
        let decoder = rodio::Decoder::try_from(cursor).unwrap();
        let buffer = rodio::buffer::SamplesBuffer::new(
            decoder.channels(),
            decoder.sample_rate(),
            decoder.collect::<Vec<f32>>()
        );

        Sound { buffer }
    }
}

impl Audio {
    pub fn initial() -> Self {
        if cfg!(target_arch = "wasm32") {
            return Audio { backend: AudioBackend::WaitForGesture };
        }
        return Self::new()
    }

    pub fn resume_on_gesture(&mut self) {
        if matches!(self.backend, AudioBackend::WaitForGesture) {
            *self = Self::new()
        }
    }

    fn new() -> Self {
        if let Ok(stream_handle) = rodio::OutputStreamBuilder::open_default_stream() {
            log::info!("audio: using backend: {:?}", stream_handle.config());

            return Audio {
                backend: AudioBackend::Open(stream_handle)
            }
        }

        log::info!("audio: no backend available");
        return Audio {
            backend: AudioBackend::None
        }
    }

    pub fn play(&self, sound: &Sound) {
        let AudioBackend::Open(handle) = &self.backend else { return };

        // Cloning should be fast here because the internal data is reference
        // counted.
        handle.mixer().add(sound.buffer.clone());
    }
}