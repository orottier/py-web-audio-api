use super::*;

pub(crate) enum BaseAudioContextInner {
    Realtime(Arc<Mutex<web_audio_api_rs::context::AudioContext>>),
    Offline(Arc<Mutex<web_audio_api_rs::context::OfflineAudioContext>>),
    Concrete(RsConcreteBaseAudioContext),
}

#[pyclass]
pub(crate) struct AudioListener(pub(crate) web_audio_api_rs::AudioListener);

#[pyclass(extends = Event)]
pub(crate) struct OfflineAudioCompletionEvent {
    rendered_buffer: web_audio_api_rs::AudioBuffer,
}

#[pymethods]
impl OfflineAudioCompletionEvent {
    #[getter(renderedBuffer)]
    pub(crate) fn rendered_buffer(&self) -> AudioBuffer {
        AudioBuffer::owned(self.rendered_buffer.clone())
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
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(&*ctx.lock().unwrap()).0,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(&*ctx.lock().unwrap()).0,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).0,
        }
    }

    #[cfg(test)]
    pub(crate) fn destination_audio_node(&self) -> AudioNode {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(&*ctx.lock().unwrap()).1,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(&*ctx.lock().unwrap()).1,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).1,
        }
    }

    pub(crate) fn listener_inner(&self) -> AudioListener {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => AudioListener(ctx.lock().unwrap().listener()),
            BaseAudioContextInner::Offline(ctx) => AudioListener(ctx.lock().unwrap().listener()),
            BaseAudioContextInner::Concrete(ctx) => AudioListener(ctx.listener()),
        }
    }

    pub(crate) fn clear_onstatechange(&self) {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.lock().unwrap().clear_onstatechange(),
            BaseAudioContextInner::Offline(ctx) => ctx.lock().unwrap().clear_onstatechange(),
            BaseAudioContextInner::Concrete(ctx) => ctx.clear_onstatechange(),
        }
    }

    pub(crate) fn set_onstatechange_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => {
                ctx.lock().unwrap().set_onstatechange(move |_| {
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
                })
            }
            BaseAudioContextInner::Offline(ctx) => {
                ctx.lock().unwrap().set_onstatechange(move |_| {
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
                })
            }
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
        parsed.sink_id = sink_id.extract()?;
    }
    if let Some(render_size_hint) = options.get_item("renderSizeHint")? {
        parsed.render_size_hint =
            audio_context_render_size_category_from_str(render_size_hint.extract::<&str>()?)?;
    }

    Ok(parsed)
}

impl AudioContext {
    pub(crate) fn clear_onsinkchange(&self) {
        self.0.lock().unwrap().clear_onsinkchange();
    }

    pub(crate) fn set_onsinkchange_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        self.0.lock().unwrap().set_onsinkchange(move |_| {
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
            BaseAudioContextInner::Realtime(ctx) => destination_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Offline(ctx) => destination_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Concrete(ctx) => destination_node_py(py, ctx),
        }
    }

    #[getter]
    pub(crate) fn listener(&self) -> AudioListener {
        self.listener_inner()
    }

    #[getter(sampleRate)]
    pub(crate) fn sample_rate(&self) -> f32 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.lock().unwrap().sample_rate(),
            BaseAudioContextInner::Offline(ctx) => ctx.lock().unwrap().sample_rate(),
            BaseAudioContextInner::Concrete(ctx) => ctx.sample_rate(),
        }
    }

    #[getter(currentTime)]
    pub(crate) fn current_time(&self) -> f64 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.lock().unwrap().current_time(),
            BaseAudioContextInner::Offline(ctx) => ctx.lock().unwrap().current_time(),
            BaseAudioContextInner::Concrete(ctx) => ctx.current_time(),
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
            BaseAudioContextInner::Realtime(ctx) => AudioBuffer::owned(
                ctx.lock()
                    .unwrap()
                    .create_buffer(number_of_channels, length, sample_rate),
            ),
            BaseAudioContextInner::Offline(ctx) => AudioBuffer::owned(
                ctx.lock()
                    .unwrap()
                    .create_buffer(number_of_channels, length, sample_rate),
            ),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::OscillatorOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => oscillator_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::ConstantSourceOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => constant_source_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::AudioBufferSourceOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => audio_buffer_source_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::GainOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => gain_node_py(
                py,
                &*ctx.lock().unwrap(),
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
            BaseAudioContextInner::Realtime(ctx) => {
                iir_filter_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                iir_filter_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => iir_filter_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createPanner")]
    pub(crate) fn create_panner(&self, py: Python<'_>) -> PyResult<Py<PannerNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => panner_node_py(
                py,
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::PannerOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => panner_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                web_audio_api_rs::PeriodicWave::new(&*ctx.lock().unwrap(), options),
            )),
            BaseAudioContextInner::Offline(ctx) => Ok(PeriodicWave(
                web_audio_api_rs::PeriodicWave::new(&*ctx.lock().unwrap(), options),
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
                script_processor_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                script_processor_node_py(py, &*ctx.lock().unwrap(), options)
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
            BaseAudioContextInner::Realtime(ctx) => {
                delay_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                delay_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => delay_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createStereoPanner")]
    pub(crate) fn create_stereo_panner(&self, py: Python<'_>) -> PyResult<Py<StereoPannerNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => stereo_panner_node_py(
                py,
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::StereoPannerOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => stereo_panner_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::WaveShaperOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => wave_shaper_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                channel_merger_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                channel_merger_node_py(py, &*ctx.lock().unwrap(), options)
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
                channel_splitter_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Offline(ctx) => {
                channel_splitter_node_py(py, &*ctx.lock().unwrap(), options)
            }
            BaseAudioContextInner::Concrete(ctx) => channel_splitter_node_py(py, ctx, options),
        }
    }

    #[pyo3(name = "createBiquadFilter")]
    pub(crate) fn create_biquad_filter(&self, py: Python<'_>) -> PyResult<Py<BiquadFilterNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => biquad_filter_node_py(
                py,
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::BiquadFilterOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => biquad_filter_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::AnalyserOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => analyser_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::ConvolverOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => convolver_node_py(
                py,
                &*ctx.lock().unwrap(),
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
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
            BaseAudioContextInner::Offline(ctx) => dynamics_compressor_node_py(
                py,
                &*ctx.lock().unwrap(),
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
            BaseAudioContextInner::Concrete(ctx) => dynamics_compressor_node_py(
                py,
                ctx,
                web_audio_api_rs::node::DynamicsCompressorOptions::default(),
            ),
        }
    }
}

#[pyclass(extends = BaseAudioContext)]
pub(crate) struct AudioContext(pub(crate) Arc<Mutex<web_audio_api_rs::context::AudioContext>>);

#[pymethods]
impl AudioContext {
    #[getter]
    pub(crate) fn onsinkchange(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().as_super().event_handler(py, "sinkchange")
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

    #[new]
    #[pyo3(signature = (options=None))]
    pub(crate) fn new(options: Option<&Bound<'_, PyAny>>) -> PyResult<PyClassInitializer<Self>> {
        let options = audio_context_options(options)?;
        let ctx =
            catch_web_audio_panic_result(|| Arc::new(Mutex::new(new_realtime_context(options))))?;
        Ok(PyClassInitializer::from(EventTarget::new())
            .add_subclass(BaseAudioContext::new(BaseAudioContextInner::Realtime(
                Arc::clone(&ctx),
            )))
            .add_subclass(Self(ctx)))
    }
}

#[pyclass(extends = BaseAudioContext)]
pub(crate) struct OfflineAudioContext(
    pub(crate) Arc<Mutex<web_audio_api_rs::context::OfflineAudioContext>>,
);

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
        slf.0.lock().unwrap().clear_oncomplete();
        slf.0.lock().unwrap().set_oncomplete(move |event| {
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
    pub(crate) fn new(
        number_of_channels: usize,
        length: usize,
        sample_rate: f32,
    ) -> PyClassInitializer<Self> {
        let ctx = Arc::new(Mutex::new(
            web_audio_api_rs::context::OfflineAudioContext::new(
                number_of_channels,
                length,
                sample_rate,
            ),
        ));
        PyClassInitializer::from(EventTarget::new())
            .add_subclass(BaseAudioContext::new(BaseAudioContextInner::Offline(
                Arc::clone(&ctx),
            )))
            .add_subclass(Self(ctx))
    }

    #[pyo3(name = "startRendering")]
    pub(crate) fn start_rendering(&self) -> PyResult<AudioBuffer> {
        catch_web_audio_panic_result(|| {
            AudioBuffer::owned(self.0.lock().unwrap().start_rendering_sync())
        })
    }
}
