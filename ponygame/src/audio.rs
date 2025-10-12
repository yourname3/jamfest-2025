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

    // Used to keep music around until we have a proper backend.
    stored_music: Option<Box<dyn Source + Send + 'static>>,

    current_music_sink: Option<rodio::Sink>,
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
            return Audio { backend: AudioBackend::WaitForGesture, stored_music: None, current_music_sink: None };
        }
        return Self::new(None)
    }

    pub fn resume_on_gesture(&mut self) {
        if matches!(self.backend, AudioBackend::WaitForGesture) {
            *self = Self::new(self.stored_music.take());
            if let Some(current_music) = self.stored_music.take() {
                let AudioBackend::Open(handle) = &self.backend else { return };

                log::info!("audio: resuming music on gesture");

                let sink = rodio::Sink::connect_new(handle.mixer());
                sink.append(current_music);
                sink.play();
                
                self.current_music_sink = Some(sink);
                //handle.mixer().add(current_music);
            }
        }
    }

    fn new(stored_music: Option<Box<dyn Source + Send + 'static>>) -> Self {
        if let Ok(stream_handle) = rodio::OutputStreamBuilder::open_default_stream() {
            log::info!("audio: using backend: {:?}", stream_handle.config());

            return Audio {
                backend: AudioBackend::Open(stream_handle),
                stored_music,
                current_music_sink: None,
            }
        }

        log::info!("audio: no backend available");
        return Audio {
            backend: AudioBackend::None,
            stored_music,
            current_music_sink: None,
        }
    }

    pub fn play(&self, sound: &Sound) {
        let AudioBackend::Open(handle) = &self.backend else { return };

        // Cloning should be fast here because the internal data is reference
        // counted.
        handle.mixer().add(sound.buffer.clone());
    }

    pub fn play_speed(&self, sound: &Sound, speed: f32) {
        let AudioBackend::Open(handle) = &self.backend else { return };

        // Cloning should be fast here because the internal data is reference
        // counted.
        handle.mixer().add(sound.buffer.clone().speed(speed));
    }

    pub fn play_music(&mut self, data: &'static [u8], amp: f32) {
        // Store music even if we don't have a backend, because we might get one
        // later.
        let cursor = std::io::Cursor::new(data);
        let decoder = rodio::decoder::Decoder::new_looped(cursor).unwrap();
        let music = decoder.amplify(amp);

        log::info!("playing music: {} {} {:?}", music.sample_rate(), music.channels(), music.total_duration());

        let AudioBackend::Open(handle) = &self.backend else {
            // If we don't have a handle *yet*, store the music for later.
            let music = Box::new(music);
            self.stored_music = Some(music);
            return;
        };

        handle.mixer().add(music);
    }
}