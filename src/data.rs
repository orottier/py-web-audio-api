use super::*;

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct MediaStream(pub(crate) web_audio_api_rs::media_streams::MediaStream);

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct MediaStreamTrack(pub(crate) web_audio_api_rs::media_streams::MediaStreamTrack);

#[pyclass]
#[derive(Debug)]
pub(crate) struct MediaDeviceInfo {
    pub(crate) device_id: String,
    pub(crate) group_id: Option<String>,
    pub(crate) kind: String,
    pub(crate) label: String,
}

impl MediaDeviceInfo {
    pub(crate) fn from_rs(device: web_audio_api_rs::media_devices::MediaDeviceInfo) -> Self {
        Self {
            device_id: device.device_id().to_owned(),
            group_id: device.group_id().map(str::to_owned),
            kind: match device.kind() {
                web_audio_api_rs::media_devices::MediaDeviceInfoKind::VideoInput => {
                    "videoinput".to_owned()
                }
                web_audio_api_rs::media_devices::MediaDeviceInfoKind::AudioInput => {
                    "audioinput".to_owned()
                }
                web_audio_api_rs::media_devices::MediaDeviceInfoKind::AudioOutput => {
                    "audiooutput".to_owned()
                }
            },
            label: device.label().to_owned(),
        }
    }
}

#[pymethods]
impl MediaStream {
    #[pyo3(name = "getTracks")]
    pub(crate) fn get_tracks(&self) -> Vec<MediaStreamTrack> {
        self.0
            .get_tracks()
            .iter()
            .cloned()
            .map(MediaStreamTrack)
            .collect()
    }

    #[pyo3(name = "close")]
    pub(crate) fn close(&self) {
        for track in self.0.get_tracks() {
            track.close();
        }
    }
}

#[pymethods]
impl MediaStreamTrack {
    #[getter(readyState)]
    pub(crate) fn ready_state(&self) -> &'static str {
        match self.0.ready_state() {
            web_audio_api_rs::media_streams::MediaStreamTrackState::Live => "live",
            web_audio_api_rs::media_streams::MediaStreamTrackState::Ended => "ended",
        }
    }

    #[pyo3(name = "close")]
    pub(crate) fn close(&self) {
        self.0.close();
    }
}

#[pymethods]
impl MediaDeviceInfo {
    #[getter(deviceId)]
    pub(crate) fn device_id(&self) -> &str {
        &self.device_id
    }

    #[getter(groupId)]
    pub(crate) fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }

    #[getter]
    pub(crate) fn kind(&self) -> &str {
        &self.kind
    }

    #[getter]
    pub(crate) fn label(&self) -> &str {
        &self.label
    }
}

pub(crate) enum AudioBufferInner {
    Owned(web_audio_api_rs::AudioBuffer),
    AudioProcessing {
        event: Arc<Mutex<Option<web_audio_api_rs::AudioProcessingEvent>>>,
        which: AudioProcessingBufferKind,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum AudioProcessingBufferKind {
    Input,
    Output,
}

#[pyclass]
pub(crate) struct AudioBuffer {
    pub(crate) inner: AudioBufferInner,
}

impl AudioBuffer {
    pub(crate) fn owned(buffer: web_audio_api_rs::AudioBuffer) -> Self {
        Self {
            inner: AudioBufferInner::Owned(buffer),
        }
    }

    pub(crate) fn audio_processing(
        event: Arc<Mutex<Option<web_audio_api_rs::AudioProcessingEvent>>>,
        which: AudioProcessingBufferKind,
    ) -> Self {
        Self {
            inner: AudioBufferInner::AudioProcessing { event, which },
        }
    }

    pub(crate) fn snapshot(&self) -> PyResult<web_audio_api_rs::AudioBuffer> {
        self.with_buffer(|buffer| Ok(buffer.clone()))
    }

    fn invalid_audio_processing_buffer_err() -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(
            "AudioProcessingEvent buffer is no longer available after the callback returns",
        )
    }

    fn use_reentrant_panic_catch(&self) -> bool {
        matches!(self.inner, AudioBufferInner::AudioProcessing { .. })
    }

    fn with_buffer<T>(
        &self,
        f: impl FnOnce(&web_audio_api_rs::AudioBuffer) -> PyResult<T>,
    ) -> PyResult<T> {
        match &self.inner {
            AudioBufferInner::Owned(buffer) => f(buffer),
            AudioBufferInner::AudioProcessing { event, which } => {
                let guard = event.lock().unwrap();
                let processing_event = guard
                    .as_ref()
                    .ok_or_else(Self::invalid_audio_processing_buffer_err)?;
                let buffer = match which {
                    AudioProcessingBufferKind::Input => &processing_event.input_buffer,
                    AudioProcessingBufferKind::Output => &processing_event.output_buffer,
                };
                f(buffer)
            }
        }
    }

    fn with_buffer_mut<T>(
        &mut self,
        f: impl FnOnce(&mut web_audio_api_rs::AudioBuffer) -> PyResult<T>,
    ) -> PyResult<T> {
        match &mut self.inner {
            AudioBufferInner::Owned(buffer) => f(buffer),
            AudioBufferInner::AudioProcessing { event, which } => {
                let mut guard = event.lock().unwrap();
                let processing_event = guard
                    .as_mut()
                    .ok_or_else(Self::invalid_audio_processing_buffer_err)?;
                let buffer = match which {
                    AudioProcessingBufferKind::Input => &mut processing_event.input_buffer,
                    AudioProcessingBufferKind::Output => &mut processing_event.output_buffer,
                };
                f(buffer)
            }
        }
    }
}

#[pymethods]
impl AudioBuffer {
    #[new]
    pub(crate) fn new(options: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self::owned(web_audio_api_rs::AudioBuffer::new(
            audio_buffer_options(options)?,
        )))
    }

    #[getter(numberOfChannels)]
    pub(crate) fn number_of_channels(&self) -> PyResult<usize> {
        self.with_buffer(|buffer| Ok(buffer.number_of_channels()))
    }

    #[getter]
    pub(crate) fn length(&self) -> PyResult<usize> {
        self.with_buffer(|buffer| Ok(buffer.length()))
    }

    #[getter(sampleRate)]
    pub(crate) fn sample_rate(&self) -> PyResult<f32> {
        self.with_buffer(|buffer| Ok(buffer.sample_rate()))
    }

    #[getter]
    pub(crate) fn duration(&self) -> PyResult<f64> {
        self.with_buffer(|buffer| Ok(buffer.duration()))
    }

    #[pyo3(name = "getChannelData")]
    pub(crate) fn get_channel_data(&self, channel_number: usize) -> PyResult<Vec<f32>> {
        let use_reentrant = self.use_reentrant_panic_catch();
        self.with_buffer(|buffer| {
            if use_reentrant {
                catch_web_audio_panic_reentrant_result(|| {
                    buffer.get_channel_data(channel_number).to_vec()
                })
            } else {
                catch_web_audio_panic_result(|| buffer.get_channel_data(channel_number).to_vec())
            }
        })
    }

    #[pyo3(name = "copyFromChannel", signature = (destination, channel_number, buffer_offset=0))]
    pub(crate) fn copy_from_channel(
        &self,
        mut destination: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<Vec<f32>> {
        let use_reentrant = self.use_reentrant_panic_catch();
        self.with_buffer(|buffer| {
            if use_reentrant {
                catch_web_audio_panic_reentrant_result(|| {
                    buffer.copy_from_channel_with_offset(
                        &mut destination,
                        channel_number,
                        buffer_offset,
                    );
                })?;
            } else {
                catch_web_audio_panic(|| {
                    buffer.copy_from_channel_with_offset(
                        &mut destination,
                        channel_number,
                        buffer_offset,
                    );
                })?;
            }
            Ok(destination)
        })
    }

    #[pyo3(name = "copyToChannel", signature = (source, channel_number, buffer_offset=0))]
    pub(crate) fn copy_to_channel(
        &mut self,
        source: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<()> {
        let use_reentrant = self.use_reentrant_panic_catch();
        self.with_buffer_mut(|buffer| {
            if use_reentrant {
                catch_web_audio_panic_reentrant_result(|| {
                    buffer.copy_to_channel_with_offset(&source, channel_number, buffer_offset);
                })
            } else {
                catch_web_audio_panic(|| {
                    buffer.copy_to_channel_with_offset(&source, channel_number, buffer_offset);
                })
            }
        })
    }
}

pub(crate) fn audio_buffer_options(
    options: &Bound<'_, PyAny>,
) -> PyResult<web_audio_api_rs::AudioBufferOptions> {
    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions must be a dict"))?;

    let number_of_channels = options
        .get_item("numberOfChannels")?
        .map(|value| value.extract())
        .transpose()?
        .unwrap_or(1);
    let length = options
        .get_item("length")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions.length is required")
        })?
        .extract()?;
    let sample_rate = options
        .get_item("sampleRate")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions.sampleRate is required")
        })?
        .extract()?;

    Ok(web_audio_api_rs::AudioBufferOptions {
        number_of_channels,
        length,
        sample_rate,
    })
}

pub(crate) fn media_stream_constraints(
    constraints: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::media_devices::MediaStreamConstraints> {
    let Some(constraints) = constraints else {
        return Ok(web_audio_api_rs::media_devices::MediaStreamConstraints::Audio);
    };

    if let Ok(enabled) = constraints.extract::<bool>() {
        if enabled {
            return Ok(web_audio_api_rs::media_devices::MediaStreamConstraints::Audio);
        }

        return Err(pyo3::exceptions::PyTypeError::new_err(
            "getUserMediaSync(False) is not supported; pass True, None, or a constraints dict",
        ));
    }

    let constraints = constraints.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "getUserMediaSync constraints must be a bool or dict",
        )
    })?;

    let Some(audio) = constraints.get_item("audio")? else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "getUserMediaSync constraints must include an 'audio' entry",
        ));
    };

    if let Ok(enabled) = audio.extract::<bool>() {
        if enabled {
            return Ok(web_audio_api_rs::media_devices::MediaStreamConstraints::Audio);
        }

        return Err(pyo3::exceptions::PyTypeError::new_err(
            "getUserMediaSync({'audio': False}) is not supported",
        ));
    }

    let audio = audio.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "getUserMediaSync audio constraints must be True or a dict",
        )
    })?;

    let mut parsed = web_audio_api_rs::media_devices::MediaTrackConstraints::default();
    if let Some(sample_rate) = audio.get_item("sampleRate")? {
        parsed.sample_rate = Some(sample_rate.extract()?);
    }
    if let Some(latency) = audio.get_item("latency")? {
        parsed.latency = Some(latency.extract()?);
    }
    if let Some(channel_count) = audio.get_item("channelCount")? {
        parsed.channel_count = Some(channel_count.extract()?);
    }
    if let Some(device_id) = audio.get_item("deviceId")? {
        parsed.device_id = Some(device_id.extract()?);
    }

    Ok(web_audio_api_rs::media_devices::MediaStreamConstraints::AudioWithConstraints(parsed))
}
