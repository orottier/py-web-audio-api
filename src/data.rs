use super::*;
use std::sync::atomic::{AtomicU8, Ordering};

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

#[pyclass(skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct Blob {
    data: Vec<u8>,
    type_: String,
}

#[pyclass(extends = Event)]
pub(crate) struct BlobEvent {
    blob: Blob,
    timecode: f64,
}

#[pyclass(extends = Event)]
pub(crate) struct ErrorEvent {
    message: String,
}

#[derive(Clone, Copy)]
enum MediaRecorderState {
    Inactive = 0,
    Recording = 1,
}

impl MediaRecorderState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Recording => "recording",
        }
    }
}

#[pyclass(extends = EventTarget)]
pub(crate) struct MediaRecorder {
    inner: Arc<web_audio_api_rs::media_recorder::MediaRecorder>,
    stream: MediaStream,
    mime_type: String,
    state: Arc<AtomicU8>,
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

fn media_recorder_state_name(state: &Arc<AtomicU8>) -> &'static str {
    match state.load(Ordering::SeqCst) {
        1 => MediaRecorderState::Recording.as_str(),
        _ => MediaRecorderState::Inactive.as_str(),
    }
}

fn blob_event_py(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    blob: web_audio_api_rs::media_recorder::Blob,
    timecode: f64,
) -> PyResult<Py<PyAny>> {
    let owner = EventTarget::owner_for_registry(py, registry);
    let type_ = blob.type_().to_owned();
    let event = Py::new(
        py,
        PyClassInitializer::from(Event::new_dispatched(
            "dataavailable",
            owner.as_ref().map(|owner| owner.clone_ref(py)),
            owner.as_ref().map(|owner| owner.clone_ref(py)),
        ))
        .add_subclass(BlobEvent {
            blob: Blob {
                data: blob.data,
                type_,
            },
            timecode,
        }),
    )?;
    Ok(event.into_any())
}

fn error_event_py(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    message: String,
) -> PyResult<Py<PyAny>> {
    let owner = EventTarget::owner_for_registry(py, registry);
    let event = Py::new(
        py,
        PyClassInitializer::from(Event::new_dispatched(
            "error",
            owner.as_ref().map(|owner| owner.clone_ref(py)),
            owner.as_ref().map(|owner| owner.clone_ref(py)),
        ))
        .add_subclass(ErrorEvent { message }),
    )?;
    Ok(event.into_any())
}

impl MediaRecorder {
    fn options(
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<web_audio_api_rs::media_recorder::MediaRecorderOptions> {
        let Some(options) = options else {
            return Ok(Default::default());
        };

        let options = options.cast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("MediaRecorderOptions must be a dict")
        })?;

        let mut parsed = web_audio_api_rs::media_recorder::MediaRecorderOptions::default();
        if let Some(mime_type) = options.get_item("mimeType")? {
            parsed.mime_type = mime_type.extract()?;
        }
        Ok(parsed)
    }

    fn install_callbacks(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        self.inner.clear_ondataavailable();
        self.inner.clear_onstop();
        self.inner.clear_onerror();

        let data_registry = Arc::clone(&registry);
        self.inner.set_ondataavailable(move |event| {
            Python::attach(|py| {
                match blob_event_py(py, &data_registry, event.blob, event.timecode) {
                    Ok(event) => {
                        if let Err(err) = EventTarget::dispatch_event_object(
                            py,
                            &data_registry,
                            "dataavailable",
                            event,
                        ) {
                            err.print(py);
                        }
                    }
                    Err(err) => err.print(py),
                }
            });
        });

        let stop_registry = Arc::clone(&registry);
        let stop_state = Arc::clone(&self.state);
        self.inner.set_onstop(move |_| {
            stop_state.store(MediaRecorderState::Inactive as u8, Ordering::SeqCst);
            Python::attach(|py| {
                if let Err(err) =
                    EventTarget::dispatch_from_registry(py, &stop_registry, "stop", None, None)
                {
                    err.print(py);
                }
            });
        });

        let error_registry = registry;
        let error_state = Arc::clone(&self.state);
        self.inner.set_onerror(move |event| {
            error_state.store(MediaRecorderState::Inactive as u8, Ordering::SeqCst);
            Python::attach(
                |py| match error_event_py(py, &error_registry, event.message) {
                    Ok(event) => {
                        if let Err(err) =
                            EventTarget::dispatch_event_object(py, &error_registry, "error", event)
                        {
                            err.print(py);
                        }
                    }
                    Err(err) => err.print(py),
                },
            );
        });
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
impl Blob {
    #[getter]
    pub(crate) fn size(&self) -> usize {
        self.data.len()
    }

    #[getter]
    pub(crate) fn r#type(&self) -> &str {
        &self.type_
    }

    pub(crate) fn bytes(&self) -> Vec<u8> {
        self.data.clone()
    }
}

#[pymethods]
impl BlobEvent {
    #[getter]
    pub(crate) fn data(&self) -> Blob {
        self.blob.clone()
    }

    #[getter]
    pub(crate) fn timecode(&self) -> f64 {
        self.timecode
    }
}

#[pymethods]
impl ErrorEvent {
    #[getter]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[pymethods]
impl MediaRecorder {
    #[new]
    #[pyo3(signature = (stream, options=None))]
    pub(crate) fn new(
        stream: PyRef<'_, MediaStream>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = Self::options(options)?;
        let mime_type = if options.mime_type.is_empty() {
            "audio/wav".to_owned()
        } else {
            options.mime_type.clone()
        };
        let inner = Arc::new(catch_web_audio_panic_result(|| {
            web_audio_api_rs::media_recorder::MediaRecorder::new(&stream.0, options)
        })?);
        Ok(
            PyClassInitializer::from(EventTarget::new()).add_subclass(Self {
                inner,
                stream: stream.clone(),
                mime_type,
                state: Arc::new(AtomicU8::new(MediaRecorderState::Inactive as u8)),
            }),
        )
    }

    #[staticmethod]
    #[pyo3(name = "isTypeSupported")]
    pub(crate) fn is_type_supported(mime_type: &str) -> bool {
        web_audio_api_rs::media_recorder::MediaRecorder::is_type_supported(mime_type)
    }

    #[getter]
    pub(crate) fn stream(&self) -> MediaStream {
        self.stream.clone()
    }

    #[getter(mimeType)]
    pub(crate) fn mime_type(&self) -> &str {
        &self.mime_type
    }

    #[getter]
    pub(crate) fn state(&self) -> &'static str {
        media_recorder_state_name(&self.state)
    }

    #[getter]
    pub(crate) fn ondataavailable(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().event_handler(py, "dataavailable")
    }

    #[setter]
    pub(crate) fn set_ondataavailable(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().set_owner(owner);
        let registry = slf.as_super().registry();
        slf.as_super().set_event_handler("dataavailable", value);
        slf.install_callbacks(registry);
    }

    #[getter]
    pub(crate) fn onstop(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().event_handler(py, "stop")
    }

    #[setter]
    pub(crate) fn set_onstop(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().set_owner(owner);
        let registry = slf.as_super().registry();
        slf.as_super().set_event_handler("stop", value);
        slf.install_callbacks(registry);
    }

    #[getter]
    pub(crate) fn onerror(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().event_handler(py, "error")
    }

    #[setter]
    pub(crate) fn set_onerror(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().set_owner(owner);
        let registry = slf.as_super().registry();
        slf.as_super().set_event_handler("error", value);
        slf.install_callbacks(registry);
    }

    pub(crate) fn start(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<()> {
        slf.as_super()
            .set_owner(EventTarget::owner_from_ptr(py, slf.as_ptr()));
        slf.install_callbacks(slf.as_super().registry());
        catch_web_audio_panic(|| slf.inner.start())?;
        slf.state
            .store(MediaRecorderState::Recording as u8, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn stop(&self) -> PyResult<()> {
        catch_web_audio_panic(|| self.inner.stop())?;
        self.state
            .store(MediaRecorderState::Inactive as u8, Ordering::SeqCst);
        Ok(())
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
