use super::*;
use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;

fn async_runtime_error(message: impl Into<String>) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(message.into())
}

fn async_boxed_error(err: Box<dyn std::error::Error + Send + Sync>) -> PyErr {
    async_runtime_error(err.to_string())
}

fn into_py_future<'py, F, T>(py: Python<'py>, fut: F) -> PyResult<Bound<'py, PyAny>>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: for<'py2> IntoPyObject<'py2> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py(py, fut)
}

pub(crate) enum BaseAudioContextInner {
    Realtime(Arc<web_audio_api_rs::context::AudioContext>),
    Offline(Arc<web_audio_api_rs::context::OfflineAudioContext>),
    Concrete(RsConcreteBaseAudioContext),
}

#[pyclass]
pub(crate) struct AudioListener(pub(crate) web_audio_api_rs::AudioListener);

#[pyclass(extends = Event)]
pub(crate) struct OfflineAudioCompletionEvent {
    rendered_buffer: web_audio_api_rs::AudioBuffer,
}

#[pyclass(extends = Event)]
pub(crate) struct AudioRenderCapacityEvent {
    timestamp: f64,
    average_load: f64,
    peak_load: f64,
    underrun_ratio: f64,
}

#[pymethods]
impl AudioRenderCapacityEvent {
    #[getter]
    pub(crate) fn timestamp(&self) -> f64 {
        self.timestamp
    }

    #[getter(averageLoad)]
    pub(crate) fn average_load(&self) -> f64 {
        self.average_load
    }

    #[getter(peakLoad)]
    pub(crate) fn peak_load(&self) -> f64 {
        self.peak_load
    }

    #[getter(underrunRatio)]
    pub(crate) fn underrun_ratio(&self) -> f64 {
        self.underrun_ratio
    }
}

#[pymethods]
impl OfflineAudioCompletionEvent {
    #[getter(renderedBuffer)]
    pub(crate) fn rendered_buffer(&self) -> AudioBuffer {
        AudioBuffer::owned(self.rendered_buffer.clone())
    }
}

#[pyclass(extends = EventTarget)]
pub(crate) struct AudioRenderCapacity {
    inner: Arc<web_audio_api_rs::AudioRenderCapacity>,
}

fn audio_render_capacity_event_py(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    event: web_audio_api_rs::AudioRenderCapacityEvent,
) -> PyResult<Py<PyAny>> {
    let owner = EventTarget::owner_for_registry(py, registry);
    let event = Py::new(
        py,
        PyClassInitializer::from(Event::new_dispatched(
            "update",
            owner.as_ref().map(|owner| owner.clone_ref(py)),
            owner,
        ))
        .add_subclass(AudioRenderCapacityEvent {
            timestamp: event.timestamp,
            average_load: event.average_load,
            peak_load: event.peak_load,
            underrun_ratio: event.underrun_ratio,
        }),
    )?;
    Ok(event.into_any())
}

fn audio_render_capacity_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::AudioRenderCapacityOptions> {
    let mut parsed = web_audio_api_rs::AudioRenderCapacityOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("AudioRenderCapacityOptions must be a dict")
    })?;
    if let Some(update_interval) = options.get_item("updateInterval")? {
        parsed.update_interval = update_interval.extract()?;
    }
    Ok(parsed)
}

impl AudioRenderCapacity {
    fn clear_onupdate(&self) {
        self.inner.clear_onupdate();
    }

    fn set_onupdate_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        self.inner.set_onupdate(move |event| {
            Python::attach(
                |py| match audio_render_capacity_event_py(py, &registry, event) {
                    Ok(event) => {
                        if let Err(err) =
                            EventTarget::dispatch_event_object(py, &registry, "update", event)
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

#[pyclass(extends = EventTarget, subclass)]
pub(crate) struct BaseAudioContext {
    inner: BaseAudioContextInner,
}

impl BaseAudioContext {
    pub(crate) fn new(inner: BaseAudioContextInner) -> Self {
        Self { inner }
    }

    #[cfg(test)]
    pub(crate) fn destination_inner(&self) -> AudioDestinationNode {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(ctx.as_ref()).0,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(ctx.as_ref()).0,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).0,
        }
    }

    #[cfg(test)]
    pub(crate) fn destination_audio_node(&self) -> AudioNode {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(ctx.as_ref()).1,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(ctx.as_ref()).1,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).1,
        }
    }

    pub(crate) fn listener_inner(&self) -> AudioListener {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => AudioListener(ctx.listener()),
            BaseAudioContextInner::Offline(ctx) => AudioListener(ctx.listener()),
            BaseAudioContextInner::Concrete(ctx) => AudioListener(ctx.listener()),
        }
    }

    pub(crate) fn clear_onstatechange(&self) {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.clear_onstatechange(),
            BaseAudioContextInner::Offline(ctx) => ctx.clear_onstatechange(),
            BaseAudioContextInner::Concrete(ctx) => ctx.clear_onstatechange(),
        }
    }

    pub(crate) fn set_onstatechange_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.set_onstatechange(move |_| {
                Python::attach(|py| {
                    if let Err(err) = EventTarget::dispatch_from_registry(
                        py,
                        &registry,
                        "statechange",
                        None,
                        None,
                    ) {
                        err.print(py);
                    }
                });
            }),
            BaseAudioContextInner::Offline(ctx) => ctx.set_onstatechange(move |_| {
                Python::attach(|py| {
                    if let Err(err) = EventTarget::dispatch_from_registry(
                        py,
                        &registry,
                        "statechange",
                        None,
                        None,
                    ) {
                        err.print(py);
                    }
                });
            }),
            BaseAudioContextInner::Concrete(ctx) => ctx.set_onstatechange(move |_| {
                Python::attach(|py| {
                    if let Err(err) = EventTarget::dispatch_from_registry(
                        py,
                        &registry,
                        "statechange",
                        None,
                        None,
                    ) {
                        err.print(py);
                    }
                });
            }),
        }
    }
}

fn decode_audio_data_future(
    inner: &BaseAudioContextInner,
    audio_data: Vec<u8>,
) -> Pin<Box<dyn Future<Output = PyResult<web_audio_api_rs::AudioBuffer>> + Send + 'static>> {
    match inner {
        BaseAudioContextInner::Realtime(ctx) => {
            let ctx = Arc::clone(ctx);
            Box::pin(async move {
                let input = Cursor::new(audio_data);
                ctx.decode_audio_data(input)
                    .await
                    .map_err(async_boxed_error)
            })
        }
        BaseAudioContextInner::Offline(ctx) => {
            let ctx = Arc::clone(ctx);
            Box::pin(async move {
                let input = Cursor::new(audio_data);
                ctx.decode_audio_data(input)
                    .await
                    .map_err(async_boxed_error)
            })
        }
        BaseAudioContextInner::Concrete(ctx) => {
            let ctx = ctx.clone();
            Box::pin(async move {
                let input = Cursor::new(audio_data);
                ctx.decode_audio_data(input)
                    .await
                    .map_err(async_boxed_error)
            })
        }
    }
}

fn decode_audio_data_input(audio_data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(bytes) = audio_data.extract::<Vec<u8>>() {
        return Ok(bytes);
    }

    let read = audio_data.getattr("read").map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "decodeAudioData expects bytes-like data or a file-like object with read()",
        )
    })?;

    let data = read.call0()?;
    data.extract::<Vec<u8>>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "decodeAudioData file-like read() must return bytes-like data",
        )
    })
}

fn offline_audio_completion_event_py(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    rendered_buffer: web_audio_api_rs::AudioBuffer,
) -> PyResult<Py<PyAny>> {
    let owner = EventTarget::owner_for_registry(py, registry);
    let event = Py::new(
        py,
        PyClassInitializer::from(Event::new_dispatched(
            "complete",
            owner.as_ref().map(|owner| owner.clone_ref(py)),
            owner,
        ))
        .add_subclass(OfflineAudioCompletionEvent { rendered_buffer }),
    )?;
    Ok(event.into_any())
}

pub(crate) fn new_realtime_context(
    options: web_audio_api_rs::context::AudioContextOptions,
) -> web_audio_api_rs::context::AudioContext {
    web_audio_api_rs::context::AudioContext::new(options)
}

pub(crate) fn audio_context_latency_category_from_value(
    value: &Bound<'_, PyAny>,
) -> PyResult<web_audio_api_rs::context::AudioContextLatencyCategory> {
    if let Ok(value) = value.extract::<f64>() {
        return Ok(web_audio_api_rs::context::AudioContextLatencyCategory::Custom(value));
    }

    match value.extract::<&str>()? {
        "balanced" => Ok(web_audio_api_rs::context::AudioContextLatencyCategory::Balanced),
        "interactive" => Ok(web_audio_api_rs::context::AudioContextLatencyCategory::Interactive),
        "playback" => Ok(web_audio_api_rs::context::AudioContextLatencyCategory::Playback),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "AudioContextOptions.latencyHint must be 'balanced', 'interactive', 'playback', or a number",
        )),
    }
}

pub(crate) fn audio_context_render_size_category_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::context::AudioContextRenderSizeCategory> {
    match value {
        "default" => Ok(web_audio_api_rs::context::AudioContextRenderSizeCategory::Default),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "AudioContextOptions.renderSizeHint must be 'default'",
        )),
    }
}

pub(crate) fn audio_context_state_to_str(
    value: web_audio_api_rs::context::AudioContextState,
) -> &'static str {
    match value {
        web_audio_api_rs::context::AudioContextState::Suspended => "suspended",
        web_audio_api_rs::context::AudioContextState::Running => "running",
        web_audio_api_rs::context::AudioContextState::Closed => "closed",
    }
}

fn audio_sink_id_from_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(sink_id) = value.extract::<String>() {
        return Ok(sink_id);
    }

    let sink_options = value.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "AudioContextOptions.sinkId must be a string or AudioSinkOptions dict",
        )
    })?;

    match sink_options
        .get_item("type")?
        .ok_or_else(|| pyo3::exceptions::PyTypeError::new_err("AudioSinkOptions.type is required"))?
        .extract::<&str>()?
    {
        "none" => Ok("none".to_owned()),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "AudioSinkOptions.type must be 'none'",
        )),
    }
}

pub(crate) fn audio_context_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::context::AudioContextOptions> {
    let mut parsed = web_audio_api_rs::context::AudioContextOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("AudioContextOptions must be a dict")
    })?;

    if let Some(latency_hint) = options.get_item("latencyHint")? {
        parsed.latency_hint = audio_context_latency_category_from_value(&latency_hint)?;
    }
    if let Some(sample_rate) = options.get_item("sampleRate")? {
        parsed.sample_rate = Some(sample_rate.extract()?);
    }
    if let Some(sink_id) = options.get_item("sinkId")? {
        parsed.sink_id = audio_sink_id_from_value(&sink_id)?;
    }
    if let Some(render_size_hint) = options.get_item("renderSizeHint")? {
        parsed.render_size_hint =
            audio_context_render_size_category_from_str(render_size_hint.extract::<&str>()?)?;
    }

    Ok(parsed)
}

fn offline_audio_context_options_from_dict(
    options: &Bound<'_, PyDict>,
) -> PyResult<(usize, usize, f32)> {
    let number_of_channels = options
        .get_item("numberOfChannels")?
        .map(|value| value.extract())
        .transpose()?
        .unwrap_or(1);
    let length = options
        .get_item("length")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err("OfflineAudioContextOptions.length is required")
        })?
        .extract()?;
    let sample_rate = options
        .get_item("sampleRate")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "OfflineAudioContextOptions.sampleRate is required",
            )
        })?
        .extract()?;

    if let Some(render_size_hint) = options.get_item("renderSizeHint")? {
        audio_context_render_size_category_from_str(render_size_hint.extract::<&str>()?)?;
    }

    Ok((number_of_channels, length, sample_rate))
}

impl AudioContext {
    pub(crate) fn clear_onsinkchange(&self) {
        self.0.clear_onsinkchange();
    }

    pub(crate) fn set_onsinkchange_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        self.0.set_onsinkchange(move |_| {
            Python::attach(|py| {
                if let Err(err) =
                    EventTarget::dispatch_from_registry(py, &registry, "sinkchange", None, None)
                {
                    err.print(py);
                }
            });
        });
    }
}

#[pymethods]
impl BaseAudioContext {
    #[getter]
    pub(crate) fn onstatechange(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().event_handler(py, "statechange")
    }

    #[setter]
    pub(crate) fn set_onstatechange(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().set_owner(owner);
        let registry = slf.as_super().registry();
        slf.as_super().set_event_handler("statechange", value);
        slf.clear_onstatechange();
        slf.set_onstatechange_registry(registry);
    }

    #[getter]
    pub(crate) fn destination(&self, py: Python<'_>) -> PyResult<Py<AudioDestinationNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_py(py, ctx.as_ref()),
            BaseAudioContextInner::Offline(ctx) => destination_node_py(py, ctx.as_ref()),
            BaseAudioContextInner::Concrete(ctx) => destination_node_py(py, ctx),
        }
    }

    #[getter]
    pub(crate) fn listener(&self) -> AudioListener {
        self.listener_inner()
    }

    #[getter(audioWorklet)]
    pub(crate) fn audio_worklet(&self, py: Python<'_>) -> PyResult<Py<AudioWorklet>> {
        AudioWorklet::new_py(py)
    }

    #[getter(sampleRate)]
    pub(crate) fn sample_rate(&self) -> f32 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.sample_rate(),
            BaseAudioContextInner::Offline(ctx) => ctx.sample_rate(),
            BaseAudioContextInner::Concrete(ctx) => ctx.sample_rate(),
        }
    }

    #[getter(currentTime)]
    pub(crate) fn current_time(&self) -> f64 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.current_time(),
            BaseAudioContextInner::Offline(ctx) => ctx.current_time(),
            BaseAudioContextInner::Concrete(ctx) => ctx.current_time(),
        }
    }

    #[getter]
    pub(crate) fn state(&self) -> String {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                audio_context_state_to_str(ctx.state()).to_owned()
            }
            BaseAudioContextInner::Offline(ctx) => {
                audio_context_state_to_str(ctx.state()).to_owned()
            }
            BaseAudioContextInner::Concrete(ctx) => {
                audio_context_state_to_str(ctx.state()).to_owned()
            }
        }
    }

    #[pyo3(name = "createBuffer")]
    pub(crate) fn create_buffer(
        &self,
        number_of_channels: usize,
        length: usize,
        sample_rate: f32,
    ) -> AudioBuffer {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                AudioBuffer::owned(ctx.create_buffer(number_of_channels, length, sample_rate))
            }
            BaseAudioContextInner::Offline(ctx) => {
                AudioBuffer::owned(ctx.create_buffer(number_of_channels, length, sample_rate))
            }
            BaseAudioContextInner::Concrete(ctx) => {
                AudioBuffer::owned(ctx.create_buffer(number_of_channels, length, sample_rate))
            }
        }
    }

    #[pyo3(name = "createOscillator")]
    pub(crate) fn create_oscillator(&self, py: Python<'_>) -> PyResult<Py<OscillatorNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => oscillator_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::OscillatorOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => oscillator_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::OscillatorOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => oscillator_node_py(
                py,
                ctx,
                web_audio_api_rs::node::OscillatorOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createConstantSource")]
    pub(crate) fn create_constant_source(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<ConstantSourceNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => constant_source_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::ConstantSourceOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => constant_source_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::ConstantSourceOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => constant_source_node_py(
                py,
                ctx,
                web_audio_api_rs::node::ConstantSourceOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createBufferSource")]
    pub(crate) fn create_buffer_source(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<AudioBufferSourceNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => audio_buffer_source_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::AudioBufferSourceOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => audio_buffer_source_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::AudioBufferSourceOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => audio_buffer_source_node_py(
                py,
                ctx,
                web_audio_api_rs::node::AudioBufferSourceOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createGain")]
    pub(crate) fn create_gain(&self, py: Python<'_>) -> PyResult<Py<GainNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => gain_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::GainOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => gain_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::GainOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => {
                gain_node_py(py, ctx, web_audio_api_rs::node::GainOptions::default())
            }
        }
    }

    #[pyo3(name = "createIIRFilter")]
    pub(crate) fn create_iir_filter(
        &self,
        py: Python<'_>,
        feedforward: Vec<f64>,
        feedback: Vec<f64>,
    ) -> PyResult<Py<IIRFilterNode>> {
        let options = web_audio_api_rs::node::IIRFilterOptions {
            audio_node_options: web_audio_api_rs::node::AudioNodeOptions::default(),
            feedforward,
            feedback,
        };
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => iir_filter_node_py(py, ctx.as_ref(), options),
            BaseAudioContextInner::Offline(ctx) => iir_filter_node_py(py, ctx.as_ref(), options),
            BaseAudioContextInner::Concrete(ctx) => iir_filter_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createPanner")]
    pub(crate) fn create_panner(&self, py: Python<'_>) -> PyResult<Py<PannerNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => panner_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::PannerOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => panner_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::PannerOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => {
                panner_node_py(py, ctx, web_audio_api_rs::node::PannerOptions::default())
            }
        }
    }

    #[pyo3(name = "createPeriodicWave", signature = (real, imag, constraints=None))]
    pub(crate) fn create_periodic_wave(
        &self,
        real: Vec<f32>,
        imag: Vec<f32>,
        constraints: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PeriodicWave> {
        let mut options = periodic_wave_constraints(constraints)?;
        options.real = Some(real);
        options.imag = Some(imag);

        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => Ok(PeriodicWave(
                web_audio_api_rs::PeriodicWave::new(ctx.as_ref(), options),
            )),
            BaseAudioContextInner::Offline(ctx) => Ok(PeriodicWave(
                web_audio_api_rs::PeriodicWave::new(ctx.as_ref(), options),
            )),
            BaseAudioContextInner::Concrete(ctx) => Ok(PeriodicWave(
                web_audio_api_rs::PeriodicWave::new(ctx, options),
            )),
        }
    }

    #[pyo3(
        name = "createScriptProcessor",
        signature = (buffer_size=0, number_of_input_channels=2, number_of_output_channels=2)
    )]
    pub(crate) fn create_script_processor(
        &self,
        py: Python<'_>,
        buffer_size: usize,
        number_of_input_channels: usize,
        number_of_output_channels: usize,
    ) -> PyResult<Py<ScriptProcessorNode>> {
        let options = web_audio_api_rs::node::ScriptProcessorOptions {
            buffer_size,
            number_of_input_channels,
            number_of_output_channels,
        };
        catch_web_audio_panic_result(|| match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                script_processor_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                script_processor_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => script_processor_node_py(py, ctx, options),
        })?
    }

    #[pyo3(name = "createDelay", signature = (max_delay_time=1.0))]
    pub(crate) fn create_delay(
        &self,
        py: Python<'_>,
        max_delay_time: f64,
    ) -> PyResult<Py<DelayNode>> {
        let options = web_audio_api_rs::node::DelayOptions {
            max_delay_time,
            ..Default::default()
        };
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => delay_node_py(py, ctx.as_ref(), options),
            BaseAudioContextInner::Offline(ctx) => delay_node_py(py, ctx.as_ref(), options),
            BaseAudioContextInner::Concrete(ctx) => delay_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createStereoPanner")]
    pub(crate) fn create_stereo_panner(&self, py: Python<'_>) -> PyResult<Py<StereoPannerNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => stereo_panner_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::StereoPannerOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => stereo_panner_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::StereoPannerOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => stereo_panner_node_py(
                py,
                ctx,
                web_audio_api_rs::node::StereoPannerOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createWaveShaper")]
    pub(crate) fn create_wave_shaper(&self, py: Python<'_>) -> PyResult<Py<WaveShaperNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => wave_shaper_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::WaveShaperOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => wave_shaper_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::WaveShaperOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => wave_shaper_node_py(
                py,
                ctx,
                web_audio_api_rs::node::WaveShaperOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createChannelMerger", signature = (number_of_inputs=6))]
    pub(crate) fn create_channel_merger(
        &self,
        py: Python<'_>,
        number_of_inputs: usize,
    ) -> PyResult<Py<ChannelMergerNode>> {
        let options = web_audio_api_rs::node::ChannelMergerOptions {
            number_of_inputs,
            ..Default::default()
        };
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                channel_merger_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                channel_merger_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => channel_merger_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createChannelSplitter", signature = (number_of_outputs=6))]
    pub(crate) fn create_channel_splitter(
        &self,
        py: Python<'_>,
        number_of_outputs: usize,
    ) -> PyResult<Py<ChannelSplitterNode>> {
        let options = web_audio_api_rs::node::ChannelSplitterOptions {
            number_of_outputs,
            ..Default::default()
        };
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                channel_splitter_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                channel_splitter_node_py(py, ctx.as_ref(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => channel_splitter_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createBiquadFilter")]
    pub(crate) fn create_biquad_filter(&self, py: Python<'_>) -> PyResult<Py<BiquadFilterNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => biquad_filter_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::BiquadFilterOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => biquad_filter_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::BiquadFilterOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => biquad_filter_node_py(
                py,
                ctx,
                web_audio_api_rs::node::BiquadFilterOptions::default(),
            ),
        }
    }

    #[pyo3(name = "createAnalyser")]
    pub(crate) fn create_analyser(&self, py: Python<'_>) -> PyResult<Py<AnalyserNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => analyser_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::AnalyserOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => analyser_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::AnalyserOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => {
                analyser_node_py(py, ctx, web_audio_api_rs::node::AnalyserOptions::default())
            }
        }
    }

    #[pyo3(name = "createConvolver")]
    pub(crate) fn create_convolver(&self, py: Python<'_>) -> PyResult<Py<ConvolverNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => convolver_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::ConvolverOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => convolver_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::ConvolverOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => {
                convolver_node_py(py, ctx, web_audio_api_rs::node::ConvolverOptions::default())
            }
        }
    }

    #[pyo3(name = "createDynamicsCompressor")]
    pub(crate) fn create_dynamics_compressor(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<DynamicsCompressorNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => dynamics_compressor_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => dynamics_compressor_node_py(
                py,
                ctx.as_ref(),
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => dynamics_compressor_node_py(
                py,
                ctx,
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
        }
    }

    #[pyo3(
        name = "decodeAudioData",
        signature = (audio_data, success_callback=None, error_callback=None)
    )]
    pub(crate) fn decode_audio_data<'py>(
        &self,
        py: Python<'py>,
        audio_data: &Bound<'_, PyAny>,
        success_callback: Option<Py<PyAny>>,
        error_callback: Option<Py<PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let audio_data = decode_audio_data_input(audio_data)?;
        let future = decode_audio_data_future(&self.inner, audio_data);

        into_py_future(py, async move {
            match future.await {
                Ok(buffer) => {
                    if let Some(callback) = success_callback {
                        Python::attach(|py| {
                            callback
                                .bind(py)
                                .call1((AudioBuffer::owned(buffer.clone()),))?;
                            Ok::<(), PyErr>(())
                        })?;
                    }
                    Ok(AudioBuffer::owned(buffer))
                }
                Err(err) => {
                    if let Some(callback) = error_callback {
                        Python::attach(|py| {
                            callback.bind(py).call1((err.value(py),))?;
                            Ok::<(), PyErr>(())
                        })?;
                    }
                    Err(err)
                }
            }
        })
    }
}

#[pyclass(extends = BaseAudioContext)]
pub(crate) struct AudioContext(
    pub(crate) Arc<web_audio_api_rs::context::AudioContext>,
    pub(crate) Arc<Mutex<EventTargetRegistry>>,
);

#[pymethods]
impl AudioContext {
    #[getter(renderCapacity)]
    pub(crate) fn render_capacity(&self, py: Python<'_>) -> PyResult<Py<AudioRenderCapacity>> {
        Py::new(
            py,
            PyClassInitializer::from(EventTarget::from_registry(Arc::clone(&self.1))).add_subclass(
                AudioRenderCapacity {
                    inner: Arc::new(self.0.render_capacity()),
                },
            ),
        )
    }

    #[getter]
    pub(crate) fn onsinkchange(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().as_super().event_handler(py, "sinkchange")
    }

    #[getter(baseLatency)]
    pub(crate) fn base_latency(&self) -> f64 {
        self.0.base_latency()
    }

    #[getter(outputLatency)]
    pub(crate) fn output_latency(&self) -> f64 {
        self.0.output_latency()
    }

    #[getter(sinkId)]
    pub(crate) fn sink_id(&self) -> String {
        self.0.sink_id()
    }

    #[setter]
    pub(crate) fn set_onsinkchange(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super()
            .as_super()
            .set_event_handler("sinkchange", value);
        slf.clear_onsinkchange();
        slf.set_onsinkchange_registry(registry);
    }

    pub(crate) fn resume<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            ctx.resume().await;
            Ok(())
        })
    }

    pub(crate) fn suspend<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            ctx.suspend().await;
            Ok(())
        })
    }

    pub(crate) fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            ctx.close().await;
            Ok(())
        })
    }

    #[pyo3(name = "createMediaElementSource")]
    pub(crate) fn create_media_element_source(
        &self,
        py: Python<'_>,
        media_element: PyRef<'_, MediaElement>,
    ) -> PyResult<Py<MediaElementAudioSourceNode>> {
        media_element_audio_source_node_py(py, self.0.as_ref(), &media_element)
    }

    #[pyo3(name = "createMediaStreamSource")]
    pub(crate) fn create_media_stream_source(
        &self,
        py: Python<'_>,
        media_stream: PyRef<'_, MediaStream>,
    ) -> PyResult<Py<MediaStreamAudioSourceNode>> {
        media_stream_audio_source_node_py(py, self.0.as_ref(), &media_stream.0)
    }

    #[pyo3(name = "createMediaStreamTrackSource")]
    pub(crate) fn create_media_stream_track_source(
        &self,
        py: Python<'_>,
        media_stream_track: PyRef<'_, MediaStreamTrack>,
    ) -> PyResult<Py<MediaStreamTrackAudioSourceNode>> {
        media_stream_track_audio_source_node_py(py, self.0.as_ref(), &media_stream_track.0)
    }

    #[pyo3(name = "createMediaStreamDestination")]
    pub(crate) fn create_media_stream_destination(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<MediaStreamAudioDestinationNode>> {
        media_stream_audio_destination_node_py(py, self.0.as_ref())
    }

    #[new]
    #[pyo3(signature = (options=None))]
    pub(crate) fn new(options: Option<&Bound<'_, PyAny>>) -> PyResult<PyClassInitializer<Self>> {
        let options = audio_context_options(options)?;
        let ctx = catch_web_audio_panic_result(|| Arc::new(new_realtime_context(options)))?;
        let render_capacity_registry = Arc::new(Mutex::new(EventTargetRegistry::default()));
        Ok(PyClassInitializer::from(EventTarget::new())
            .add_subclass(BaseAudioContext::new(BaseAudioContextInner::Realtime(
                Arc::clone(&ctx),
            )))
            .add_subclass(Self(ctx, render_capacity_registry)))
    }
}

#[pyclass(extends = BaseAudioContext)]
pub(crate) struct OfflineAudioContext(
    pub(crate) Arc<web_audio_api_rs::context::OfflineAudioContext>,
);

#[pymethods]
impl AudioRenderCapacity {
    #[getter]
    pub(crate) fn onupdate(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().event_handler(py, "update")
    }

    #[setter]
    pub(crate) fn set_onupdate(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().set_owner(owner);
        let registry = slf.as_super().registry();
        slf.as_super().set_event_handler("update", value);
        slf.clear_onupdate();
        slf.set_onupdate_registry(registry);
    }

    #[pyo3(signature = (options=None))]
    pub(crate) fn start(&self, options: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let options = audio_render_capacity_options(options)?;
        self.inner.start(options);
        Ok(())
    }

    pub(crate) fn stop(&self) {
        self.inner.stop();
    }
}

#[pymethods]
impl OfflineAudioContext {
    #[getter]
    pub(crate) fn oncomplete(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().as_super().event_handler(py, "complete")
    }

    #[setter]
    pub(crate) fn set_oncomplete(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super()
            .as_super()
            .set_event_handler("complete", value);
        slf.0.clear_oncomplete();
        slf.0.set_oncomplete(move |event| {
            Python::attach(|py| {
                match offline_audio_completion_event_py(py, &registry, event.rendered_buffer) {
                    Ok(event) => {
                        if let Err(err) =
                            EventTarget::dispatch_event_object(py, &registry, "complete", event)
                        {
                            err.print(py);
                        }
                    }
                    Err(err) => err.print(py),
                }
            });
        });
    }

    #[new]
    #[pyo3(signature = (number_of_channels_or_options, length=None, sample_rate=None))]
    pub(crate) fn new(
        number_of_channels_or_options: &Bound<'_, PyAny>,
        length: Option<usize>,
        sample_rate: Option<f32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let (number_of_channels, length, sample_rate) = if let Ok(options) =
            number_of_channels_or_options.cast::<PyDict>()
        {
            offline_audio_context_options_from_dict(options)?
        } else {
            (
                number_of_channels_or_options.extract::<usize>()?,
                length.ok_or_else(|| {
                    pyo3::exceptions::PyTypeError::new_err("OfflineAudioContext.length is required")
                })?,
                sample_rate.ok_or_else(|| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "OfflineAudioContext.sampleRate is required",
                    )
                })?,
            )
        };

        let ctx = Arc::new(web_audio_api_rs::context::OfflineAudioContext::new(
            number_of_channels,
            length,
            sample_rate,
        ));
        Ok(PyClassInitializer::from(EventTarget::new())
            .add_subclass(BaseAudioContext::new(BaseAudioContextInner::Offline(
                Arc::clone(&ctx),
            )))
            .add_subclass(Self(ctx)))
    }

    #[getter]
    pub(crate) fn length(&self) -> usize {
        self.0.length()
    }

    #[pyo3(name = "startRendering")]
    pub(crate) fn start_rendering<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            Ok(AudioBuffer::owned(ctx.start_rendering().await))
        })
    }

    pub(crate) fn resume<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            ctx.resume().await;
            Ok(())
        })
    }

    pub(crate) fn suspend<'py>(
        &self,
        py: Python<'py>,
        suspend_time: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Arc::clone(&self.0);
        into_py_future(py, async move {
            ctx.suspend(suspend_time).await;
            Ok(())
        })
    }
}
