use super::*;

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PeriodicWave(pub(crate) web_audio_api_rs::PeriodicWave);

#[pymethods]
impl PeriodicWave {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let options = periodic_wave_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(Self(web_audio_api_rs::PeriodicWave::new(
                ctx.0.as_ref(),
                options,
            )));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(Self(web_audio_api_rs::PeriodicWave::new(
                ctx.0.as_ref(),
                options,
            )));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }
}

pub(crate) fn options_dict<'py>(
    options: Option<&'py Bound<'py, PyAny>>,
    type_name: &str,
) -> PyResult<Option<&'py Bound<'py, PyDict>>> {
    options
        .map(|options| {
            options.cast::<PyDict>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(format!("{type_name} must be a dict"))
            })
        })
        .transpose()
}

pub(crate) fn update_audio_node_options(
    options: &Bound<'_, PyDict>,
    parsed: &mut web_audio_api_rs::node::AudioNodeOptions,
) -> PyResult<()> {
    if let Some(channel_count) = options.get_item("channelCount")? {
        parsed.channel_count = channel_count.extract()?;
    }
    if let Some(channel_count_mode) = options.get_item("channelCountMode")? {
        parsed.channel_count_mode =
            channel_count_mode_from_str(channel_count_mode.extract::<&str>()?)?;
    }
    if let Some(channel_interpretation) = options.get_item("channelInterpretation")? {
        parsed.channel_interpretation =
            channel_interpretation_from_str(channel_interpretation.extract::<&str>()?)?;
    }

    Ok(())
}

pub(crate) fn with_option_item<'py>(
    options: &Bound<'py, PyDict>,
    key: &str,
    apply: impl FnOnce(Bound<'py, PyAny>) -> PyResult<()>,
) -> PyResult<()> {
    if let Some(value) = options.get_item(key)? {
        apply(value)?;
    }
    Ok(())
}

pub(crate) fn update_option_field<'py, T>(
    options: &Bound<'py, PyDict>,
    key: &str,
    target: &mut T,
) -> PyResult<()>
where
    T: for<'a> pyo3::FromPyObject<'a, 'py>,
    for<'a> <T as pyo3::FromPyObject<'a, 'py>>::Error: Into<pyo3::PyErr>,
{
    if let Some(value) = options.get_item(key)? {
        *target = value.extract().map_err(Into::<pyo3::PyErr>::into)?;
    }
    Ok(())
}

pub(crate) fn audio_worklet_node_options(
    name: &str,
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<(
    Vec<web_audio_api_rs::AudioParamDescriptor>,
    web_audio_api_rs::worklet::AudioWorkletNodeOptions<PythonWorkletBridgeOptions>,
)> {
    let descriptors = registered_worklet_descriptors(name)?;
    let mut parsed =
        web_audio_api_rs::worklet::AudioWorkletNodeOptions::<PythonWorkletBridgeOptions>::default();
    parsed.processor_options = PythonWorkletBridgeOptions {
        bridge_id: next_worklet_bridge_id(),
        registration_name: name.to_owned(),
        processor_options: BasicMessageValue::None,
        node_port: new_worklet_node_port_shared(),
    };

    if let Some(options) = options_dict(options, "AudioWorkletNodeOptions")? {
        update_audio_node_options(options, &mut parsed.audio_node_options)?;
        if let Some(number_of_inputs) = options.get_item("numberOfInputs")? {
            parsed.number_of_inputs = number_of_inputs.extract()?;
        }
        if let Some(number_of_outputs) = options.get_item("numberOfOutputs")? {
            parsed.number_of_outputs = number_of_outputs.extract()?;
        }
        if let Some(output_channel_count) = options.get_item("outputChannelCount")? {
            parsed.output_channel_count = output_channel_count.extract()?;
        }
        if let Some(parameter_data) = options.get_item("parameterData")? {
            let parameter_data = parameter_data.cast::<PyDict>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(
                    "AudioWorkletNodeOptions.parameterData must be a dict",
                )
            })?;
            for (key, value) in parameter_data.iter() {
                parsed
                    .parameter_data
                    .insert(key.extract::<String>()?, value.extract::<f64>()?);
            }
        }
        if let Some(processor_options) = options.get_item("processorOptions")? {
            parsed.processor_options.processor_options =
                py_to_basic_message_value(&processor_options)?;
        }
    }

    Ok((descriptors, parsed))
}

pub(crate) enum ScheduledSourceInner {
    AudioBufferSource(Arc<Mutex<web_audio_api_rs::node::AudioBufferSourceNode>>),
    Oscillator(Arc<Mutex<web_audio_api_rs::node::OscillatorNode>>),
    ConstantSource(Arc<Mutex<web_audio_api_rs::node::ConstantSourceNode>>),
}

impl ScheduledSourceInner {
    pub(crate) fn start_at(&self, when: f64) -> PyResult<()> {
        match self {
            Self::AudioBufferSource(node) => {
                catch_web_audio_panic(|| node.lock().unwrap().start_at(when))
            }
            Self::Oscillator(node) => catch_web_audio_panic(|| node.lock().unwrap().start_at(when)),
            Self::ConstantSource(node) => {
                catch_web_audio_panic(|| node.lock().unwrap().start_at(when))
            }
        }
    }

    pub(crate) fn stop_at(&self, when: f64) -> PyResult<()> {
        match self {
            Self::AudioBufferSource(node) => {
                catch_web_audio_panic(|| node.lock().unwrap().stop_at(when))
            }
            Self::Oscillator(node) => catch_web_audio_panic(|| node.lock().unwrap().stop_at(when)),
            Self::ConstantSource(node) => {
                catch_web_audio_panic(|| node.lock().unwrap().stop_at(when))
            }
        }
    }

    pub(crate) fn clear_onended(&self) {
        match self {
            Self::AudioBufferSource(node) => node.lock().unwrap().clear_onended(),
            Self::Oscillator(node) => node.lock().unwrap().clear_onended(),
            Self::ConstantSource(node) => node.lock().unwrap().clear_onended(),
        }
    }

    pub(crate) fn set_onended_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        match self {
            Self::AudioBufferSource(node) => node.lock().unwrap().set_onended(move |_| {
                Python::attach(|py| {
                    if let Err(err) =
                        EventTarget::dispatch_from_registry(py, &registry, "ended", None, None)
                    {
                        err.print(py);
                    }
                });
            }),
            Self::Oscillator(node) => node.lock().unwrap().set_onended(move |_| {
                Python::attach(|py| {
                    if let Err(err) =
                        EventTarget::dispatch_from_registry(py, &registry, "ended", None, None)
                    {
                        err.print(py);
                    }
                });
            }),
            Self::ConstantSource(node) => node.lock().unwrap().set_onended(move |_| {
                Python::attach(|py| {
                    if let Err(err) =
                        EventTarget::dispatch_from_registry(py, &registry, "ended", None, None)
                    {
                        err.print(py);
                    }
                });
            }),
        }
    }
}

pub(crate) fn wrap_audio_node<T, P>(
    node: T,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> (P, AudioNode)
where
    T: RsAudioNode + Send + 'static,
{
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (wrap(node), AudioNode(audio_node))
}

pub(crate) fn init_audio_node<T, P>(
    node: T,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> PyClassInitializer<P>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioNode>,
{
    let (node, base) = wrap_audio_node(node, wrap);
    PyClassInitializer::from(EventTarget::new())
        .add_subclass(base)
        .add_subclass(node)
}

pub(crate) fn new_audio_node_py<T, P>(
    py: Python<'_>,
    node: T,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> PyResult<Py<P>>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioNode>,
{
    Py::new(py, init_audio_node(node, wrap))
}

pub(crate) fn wrap_scheduled_source_node<T, P>(
    node: T,
    scheduled: impl FnOnce(Arc<Mutex<T>>) -> ScheduledSourceInner,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> (P, AudioScheduledSourceNode, AudioNode)
where
    T: RsAudioNode + Send + 'static,
{
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (
        wrap(Arc::clone(&node)),
        AudioScheduledSourceNode::new(scheduled(node)),
        AudioNode(audio_node),
    )
}

pub(crate) fn init_scheduled_source_node<T, P>(
    node: T,
    scheduled: impl FnOnce(Arc<Mutex<T>>) -> ScheduledSourceInner,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> PyClassInitializer<P>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioScheduledSourceNode>,
{
    let (node, scheduled, base) = wrap_scheduled_source_node(node, scheduled, wrap);
    PyClassInitializer::from(EventTarget::new())
        .add_subclass(base)
        .add_subclass(scheduled)
        .add_subclass(node)
}

pub(crate) fn new_scheduled_source_node_py<T, P>(
    py: Python<'_>,
    node: T,
    scheduled: impl FnOnce(Arc<Mutex<T>>) -> ScheduledSourceInner,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> PyResult<Py<P>>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioScheduledSourceNode>,
{
    Py::new(py, init_scheduled_source_node(node, scheduled, wrap))
}

#[cfg(test)]
pub(crate) fn media_element_audio_source_node_parts(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_element: &MediaElement,
) -> PyResult<(MediaElementAudioSourceNode, AudioNode)> {
    let node = catch_web_audio_panic_result(|| {
        ctx.create_media_element_source(&mut media_element.0.lock().unwrap())
    })?;
    Ok(wrap_audio_node(node, |_| MediaElementAudioSourceNode {
        media_element: media_element.clone(),
    }))
}

pub(crate) fn media_element_audio_source_node(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_element: &MediaElement,
) -> PyResult<PyClassInitializer<MediaElementAudioSourceNode>> {
    let node = catch_web_audio_panic_result(|| {
        ctx.create_media_element_source(&mut media_element.0.lock().unwrap())
    })?;
    Ok(init_audio_node(node, |_| MediaElementAudioSourceNode {
        media_element: media_element.clone(),
    }))
}

pub(crate) fn media_element_audio_source_node_py(
    py: Python<'_>,
    ctx: &web_audio_api_rs::context::AudioContext,
    media_element: &MediaElement,
) -> PyResult<Py<MediaElementAudioSourceNode>> {
    let node = catch_web_audio_panic_result(|| {
        ctx.create_media_element_source(&mut media_element.0.lock().unwrap())
    })?;
    new_audio_node_py(py, node, |_| MediaElementAudioSourceNode {
        media_element: media_element.clone(),
    })
}

#[cfg(test)]
pub(crate) fn audio_buffer_source_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> (AudioBufferSourceNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options),
        ScheduledSourceInner::AudioBufferSource,
        AudioBufferSourceNode,
    )
}

pub(crate) fn audio_buffer_source_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> PyClassInitializer<AudioBufferSourceNode> {
    init_scheduled_source_node(
        web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options),
        ScheduledSourceInner::AudioBufferSource,
        AudioBufferSourceNode,
    )
}

pub(crate) fn audio_buffer_source_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> PyResult<Py<AudioBufferSourceNode>> {
    new_scheduled_source_node_py(
        py,
        web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options),
        ScheduledSourceInner::AudioBufferSource,
        AudioBufferSourceNode,
    )
}

#[cfg(test)]
pub(crate) fn media_stream_audio_source_node_parts(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream: &web_audio_api_rs::media_streams::MediaStream,
) -> (MediaStreamAudioSourceNode, AudioNode) {
    let media_stream = MediaStream(media_stream.clone());
    wrap_audio_node(ctx.create_media_stream_source(&media_stream.0), |inner| {
        let _ = inner;
        MediaStreamAudioSourceNode {
            media_stream: media_stream.clone(),
        }
    })
}

pub(crate) fn media_stream_audio_source_node(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream: &web_audio_api_rs::media_streams::MediaStream,
) -> PyClassInitializer<MediaStreamAudioSourceNode> {
    let media_stream = MediaStream(media_stream.clone());
    init_audio_node(ctx.create_media_stream_source(&media_stream.0), |inner| {
        let _ = inner;
        MediaStreamAudioSourceNode {
            media_stream: media_stream.clone(),
        }
    })
}

pub(crate) fn media_stream_audio_source_node_py(
    py: Python<'_>,
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream: &web_audio_api_rs::media_streams::MediaStream,
) -> PyResult<Py<MediaStreamAudioSourceNode>> {
    let media_stream = MediaStream(media_stream.clone());
    new_audio_node_py(py, ctx.create_media_stream_source(&media_stream.0), |_| {
        MediaStreamAudioSourceNode {
            media_stream: media_stream.clone(),
        }
    })
}

#[cfg(test)]
pub(crate) fn media_stream_track_audio_source_node_parts(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream_track: &web_audio_api_rs::media_streams::MediaStreamTrack,
) -> (MediaStreamTrackAudioSourceNode, AudioNode) {
    let media_stream_track = MediaStreamTrack(media_stream_track.clone());
    wrap_audio_node(
        ctx.create_media_stream_track_source(&media_stream_track.0),
        |_| MediaStreamTrackAudioSourceNode {
            media_stream_track: media_stream_track.clone(),
        },
    )
}

pub(crate) fn media_stream_track_audio_source_node(
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream_track: &web_audio_api_rs::media_streams::MediaStreamTrack,
) -> PyClassInitializer<MediaStreamTrackAudioSourceNode> {
    let media_stream_track = MediaStreamTrack(media_stream_track.clone());
    init_audio_node(
        ctx.create_media_stream_track_source(&media_stream_track.0),
        |_| MediaStreamTrackAudioSourceNode {
            media_stream_track: media_stream_track.clone(),
        },
    )
}

pub(crate) fn media_stream_track_audio_source_node_py(
    py: Python<'_>,
    ctx: &web_audio_api_rs::context::AudioContext,
    media_stream_track: &web_audio_api_rs::media_streams::MediaStreamTrack,
) -> PyResult<Py<MediaStreamTrackAudioSourceNode>> {
    let media_stream_track = MediaStreamTrack(media_stream_track.clone());
    new_audio_node_py(
        py,
        ctx.create_media_stream_track_source(&media_stream_track.0),
        |_| MediaStreamTrackAudioSourceNode {
            media_stream_track: media_stream_track.clone(),
        },
    )
}

#[cfg(test)]
pub(crate) fn media_stream_audio_destination_node_parts(
    ctx: &web_audio_api_rs::context::AudioContext,
) -> (MediaStreamAudioDestinationNode, AudioNode) {
    wrap_audio_node(
        ctx.create_media_stream_destination(),
        MediaStreamAudioDestinationNode,
    )
}

pub(crate) fn media_stream_audio_destination_node_py(
    py: Python<'_>,
    ctx: &web_audio_api_rs::context::AudioContext,
) -> PyResult<Py<MediaStreamAudioDestinationNode>> {
    new_audio_node_py(
        py,
        ctx.create_media_stream_destination(),
        MediaStreamAudioDestinationNode,
    )
}

#[cfg(test)]
pub(crate) fn analyser_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AnalyserOptions,
) -> (AnalyserNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::AnalyserNode::new(ctx, options),
        AnalyserNode,
    )
}

pub(crate) fn analyser_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AnalyserOptions,
) -> PyClassInitializer<AnalyserNode> {
    init_audio_node(
        web_audio_api_rs::node::AnalyserNode::new(ctx, options),
        AnalyserNode,
    )
}

pub(crate) fn analyser_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AnalyserOptions,
) -> PyResult<Py<AnalyserNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::AnalyserNode::new(ctx, options),
        AnalyserNode,
    )
}

#[cfg(test)]
pub(crate) fn convolver_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConvolverOptions,
) -> (ConvolverNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ConvolverNode::new(ctx, options),
        ConvolverNode,
    )
}

pub(crate) fn convolver_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConvolverOptions,
) -> PyClassInitializer<ConvolverNode> {
    init_audio_node(
        web_audio_api_rs::node::ConvolverNode::new(ctx, options),
        ConvolverNode,
    )
}

pub(crate) fn convolver_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConvolverOptions,
) -> PyResult<Py<ConvolverNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::ConvolverNode::new(ctx, options),
        ConvolverNode,
    )
}

#[cfg(test)]
pub(crate) fn dynamics_compressor_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DynamicsCompressorOptions,
) -> (DynamicsCompressorNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::DynamicsCompressorNode::new(ctx, options),
        DynamicsCompressorNode,
    )
}

pub(crate) fn dynamics_compressor_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DynamicsCompressorOptions,
) -> PyClassInitializer<DynamicsCompressorNode> {
    init_audio_node(
        web_audio_api_rs::node::DynamicsCompressorNode::new(ctx, options),
        DynamicsCompressorNode,
    )
}

pub(crate) fn dynamics_compressor_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DynamicsCompressorOptions,
) -> PyResult<Py<DynamicsCompressorNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::DynamicsCompressorNode::new(ctx, options),
        DynamicsCompressorNode,
    )
}

#[cfg(test)]
pub(crate) fn gain_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> (GainNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::GainNode::new(ctx, options),
        GainNode,
    )
}

pub(crate) fn gain_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> PyClassInitializer<GainNode> {
    init_audio_node(
        web_audio_api_rs::node::GainNode::new(ctx, options),
        GainNode,
    )
}

pub(crate) fn gain_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> PyResult<Py<GainNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::GainNode::new(ctx, options),
        GainNode,
    )
}

#[cfg(test)]
pub(crate) fn delay_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> (DelayNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::DelayNode::new(ctx, options),
        DelayNode,
    )
}

pub(crate) fn delay_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> PyClassInitializer<DelayNode> {
    init_audio_node(
        web_audio_api_rs::node::DelayNode::new(ctx, options),
        DelayNode,
    )
}

pub(crate) fn delay_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> PyResult<Py<DelayNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::DelayNode::new(ctx, options),
        DelayNode,
    )
}

#[cfg(test)]
pub(crate) fn stereo_panner_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> (StereoPannerNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::StereoPannerNode::new(ctx, options),
        StereoPannerNode,
    )
}

pub(crate) fn stereo_panner_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> PyClassInitializer<StereoPannerNode> {
    init_audio_node(
        web_audio_api_rs::node::StereoPannerNode::new(ctx, options),
        StereoPannerNode,
    )
}

pub(crate) fn stereo_panner_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> PyResult<Py<StereoPannerNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::StereoPannerNode::new(ctx, options),
        StereoPannerNode,
    )
}

#[cfg(test)]
pub(crate) fn channel_merger_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> (ChannelMergerNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ChannelMergerNode::new(ctx, options),
        ChannelMergerNode,
    )
}

pub(crate) fn channel_merger_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> PyClassInitializer<ChannelMergerNode> {
    init_audio_node(
        web_audio_api_rs::node::ChannelMergerNode::new(ctx, options),
        ChannelMergerNode,
    )
}

pub(crate) fn channel_merger_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> PyResult<Py<ChannelMergerNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::ChannelMergerNode::new(ctx, options),
        ChannelMergerNode,
    )
}

#[cfg(test)]
pub(crate) fn channel_splitter_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> (ChannelSplitterNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options),
        ChannelSplitterNode,
    )
}

pub(crate) fn channel_splitter_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> PyClassInitializer<ChannelSplitterNode> {
    init_audio_node(
        web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options),
        ChannelSplitterNode,
    )
}

pub(crate) fn channel_splitter_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> PyResult<Py<ChannelSplitterNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options),
        ChannelSplitterNode,
    )
}

#[cfg(test)]
pub(crate) fn biquad_filter_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> (BiquadFilterNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::BiquadFilterNode::new(ctx, options),
        BiquadFilterNode,
    )
}

pub(crate) fn biquad_filter_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> PyClassInitializer<BiquadFilterNode> {
    init_audio_node(
        web_audio_api_rs::node::BiquadFilterNode::new(ctx, options),
        BiquadFilterNode,
    )
}

pub(crate) fn biquad_filter_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> PyResult<Py<BiquadFilterNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::BiquadFilterNode::new(ctx, options),
        BiquadFilterNode,
    )
}

#[cfg(test)]
pub(crate) fn iir_filter_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::IIRFilterOptions,
) -> (IIRFilterNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::IIRFilterNode::new(ctx, options),
        IIRFilterNode,
    )
}

pub(crate) fn iir_filter_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::IIRFilterOptions,
) -> PyClassInitializer<IIRFilterNode> {
    init_audio_node(
        web_audio_api_rs::node::IIRFilterNode::new(ctx, options),
        IIRFilterNode,
    )
}

pub(crate) fn iir_filter_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::IIRFilterOptions,
) -> PyResult<Py<IIRFilterNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::IIRFilterNode::new(ctx, options),
        IIRFilterNode,
    )
}

#[cfg(test)]
pub(crate) fn wave_shaper_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::WaveShaperOptions,
) -> (WaveShaperNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::WaveShaperNode::new(ctx, options),
        WaveShaperNode,
    )
}

pub(crate) fn wave_shaper_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::WaveShaperOptions,
) -> PyClassInitializer<WaveShaperNode> {
    init_audio_node(
        web_audio_api_rs::node::WaveShaperNode::new(ctx, options),
        WaveShaperNode,
    )
}

pub(crate) fn wave_shaper_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::WaveShaperOptions,
) -> PyResult<Py<WaveShaperNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::WaveShaperNode::new(ctx, options),
        WaveShaperNode,
    )
}

#[cfg(test)]
pub(crate) fn panner_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::PannerOptions,
) -> (PannerNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::PannerNode::new(ctx, options),
        PannerNode,
    )
}

pub(crate) fn panner_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::PannerOptions,
) -> PyClassInitializer<PannerNode> {
    init_audio_node(
        web_audio_api_rs::node::PannerNode::new(ctx, options),
        PannerNode,
    )
}

pub(crate) fn panner_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::PannerOptions,
) -> PyResult<Py<PannerNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::PannerNode::new(ctx, options),
        PannerNode,
    )
}

pub(crate) fn script_processor_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ScriptProcessorOptions,
) -> PyResult<Py<ScriptProcessorNode>> {
    new_audio_node_py(
        py,
        web_audio_api_rs::node::ScriptProcessorNode::new(ctx, options),
        ScriptProcessorNode,
    )
}

pub(crate) fn audio_worklet_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    descriptors: Vec<web_audio_api_rs::AudioParamDescriptor>,
    options: web_audio_api_rs::worklet::AudioWorkletNodeOptions<PythonWorkletBridgeOptions>,
) -> PyResult<Py<AudioWorkletNode>> {
    let node_port_shared = options.processor_options.node_port.clone();
    let node_port = MessagePort::new_py(py, Arc::clone(&node_port_shared))?;
    let node = catch_web_audio_panic_result(|| {
        with_worklet_descriptors(descriptors, || {
            web_audio_api_rs::worklet::AudioWorkletNode::new::<PythonWorkletBridgeProcessor>(
                ctx, options,
            )
        })
    })?;
    let params = AudioParamMap {
        params: node
            .parameters()
            .iter()
            .map(|(name, param)| (name.clone(), AudioParam(param.clone())))
            .collect(),
    };
    let inner = Arc::new(Mutex::new(node));
    set_worklet_node_port_node(&node_port_shared, Arc::clone(&inner));
    let base = AudioNode(inner.clone());
    Py::new(
        py,
        PyClassInitializer::from(EventTarget::new())
            .add_subclass(base)
            .add_subclass(AudioWorkletNode {
                inner,
                parameters: params,
                port: node_port,
            }),
    )
}

#[cfg(test)]
pub(crate) fn oscillator_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::OscillatorOptions,
) -> (OscillatorNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        web_audio_api_rs::node::OscillatorNode::new(ctx, options),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

pub(crate) fn oscillator_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::OscillatorOptions,
) -> PyClassInitializer<OscillatorNode> {
    init_scheduled_source_node(
        web_audio_api_rs::node::OscillatorNode::new(ctx, options),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

pub(crate) fn oscillator_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::OscillatorOptions,
) -> PyResult<Py<OscillatorNode>> {
    new_scheduled_source_node_py(
        py,
        web_audio_api_rs::node::OscillatorNode::new(ctx, options),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

#[cfg(test)]
pub(crate) fn constant_source_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> (ConstantSourceNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        web_audio_api_rs::node::ConstantSourceNode::new(ctx, options),
        ScheduledSourceInner::ConstantSource,
        ConstantSourceNode,
    )
}

pub(crate) fn constant_source_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> PyClassInitializer<ConstantSourceNode> {
    init_scheduled_source_node(
        web_audio_api_rs::node::ConstantSourceNode::new(ctx, options),
        ScheduledSourceInner::ConstantSource,
        ConstantSourceNode,
    )
}

pub(crate) fn constant_source_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> PyResult<Py<ConstantSourceNode>> {
    new_scheduled_source_node_py(
        py,
        web_audio_api_rs::node::ConstantSourceNode::new(ctx, options),
        ScheduledSourceInner::ConstantSource,
        ConstantSourceNode,
    )
}

pub(crate) fn constant_source_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ConstantSourceOptions> {
    let mut parsed = web_audio_api_rs::node::ConstantSourceOptions::default();
    let Some(options) = options_dict(options, "ConstantSourceOptions")? else {
        return Ok(parsed);
    };
    update_option_field(options, "offset", &mut parsed.offset)?;

    Ok(parsed)
}

pub(crate) fn audio_buffer_source_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::AudioBufferSourceOptions> {
    let mut parsed = web_audio_api_rs::node::AudioBufferSourceOptions::default();
    let Some(options) = options_dict(options, "AudioBufferSourceOptions")? else {
        return Ok(parsed);
    };

    with_option_item(options, "buffer", |buffer| {
        if !buffer.is_none() {
            parsed.buffer = Some(buffer.extract::<PyRef<'_, AudioBuffer>>()?.snapshot()?);
        }
        Ok(())
    })?;
    update_option_field(options, "detune", &mut parsed.detune)?;
    update_option_field(options, "loop", &mut parsed.loop_)?;
    update_option_field(options, "loopEnd", &mut parsed.loop_end)?;
    update_option_field(options, "loopStart", &mut parsed.loop_start)?;
    update_option_field(options, "playbackRate", &mut parsed.playback_rate)?;

    Ok(parsed)
}

pub(crate) fn gain_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::GainOptions> {
    let mut parsed = web_audio_api_rs::node::GainOptions::default();
    let Some(options) = options_dict(options, "GainOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "gain", &mut parsed.gain)?;

    Ok(parsed)
}

pub(crate) fn analyser_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::AnalyserOptions> {
    let mut parsed = web_audio_api_rs::node::AnalyserOptions::default();
    let Some(options) = options_dict(options, "AnalyserOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "fftSize", &mut parsed.fft_size)?;
    update_option_field(options, "maxDecibels", &mut parsed.max_decibels)?;
    update_option_field(options, "minDecibels", &mut parsed.min_decibels)?;
    update_option_field(
        options,
        "smoothingTimeConstant",
        &mut parsed.smoothing_time_constant,
    )?;

    Ok(parsed)
}

pub(crate) fn convolver_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ConvolverOptions> {
    let mut parsed = web_audio_api_rs::node::ConvolverOptions::default();
    let Some(options) = options_dict(options, "ConvolverOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;

    with_option_item(options, "buffer", |buffer| {
        if !buffer.is_none() {
            parsed.buffer = Some(buffer.extract::<PyRef<'_, AudioBuffer>>()?.snapshot()?);
        }
        Ok(())
    })?;
    with_option_item(options, "normalize", |normalize| {
        let normalize = normalize
            .extract::<bool>()
            .map_err(Into::<pyo3::PyErr>::into)?;
        parsed.disable_normalization = !normalize;
        Ok(())
    })?;
    update_option_field(
        options,
        "disableNormalization",
        &mut parsed.disable_normalization,
    )?;

    Ok(parsed)
}

pub(crate) fn dynamics_compressor_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::DynamicsCompressorOptions> {
    let mut parsed = web_audio_api_rs::node::DynamicsCompressorOptions::default();
    let Some(options) = options_dict(options, "DynamicsCompressorOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "attack", &mut parsed.attack)?;
    update_option_field(options, "knee", &mut parsed.knee)?;
    update_option_field(options, "ratio", &mut parsed.ratio)?;
    update_option_field(options, "release", &mut parsed.release)?;
    update_option_field(options, "threshold", &mut parsed.threshold)?;

    Ok(parsed)
}

pub(crate) fn delay_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::DelayOptions> {
    let mut parsed = web_audio_api_rs::node::DelayOptions::default();
    let Some(options) = options_dict(options, "DelayOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "maxDelayTime", &mut parsed.max_delay_time)?;
    update_option_field(options, "delayTime", &mut parsed.delay_time)?;

    Ok(parsed)
}

pub(crate) fn stereo_panner_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::StereoPannerOptions> {
    let mut parsed = web_audio_api_rs::node::StereoPannerOptions::default();
    let Some(options) = options_dict(options, "StereoPannerOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "pan", &mut parsed.pan)?;

    Ok(parsed)
}

pub(crate) fn channel_merger_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ChannelMergerOptions> {
    let mut parsed = web_audio_api_rs::node::ChannelMergerOptions::default();
    let Some(options) = options_dict(options, "ChannelMergerOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "numberOfInputs", &mut parsed.number_of_inputs)?;

    Ok(parsed)
}

pub(crate) fn channel_splitter_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ChannelSplitterOptions> {
    let mut parsed = web_audio_api_rs::node::ChannelSplitterOptions::default();
    let Some(options) = options_dict(options, "ChannelSplitterOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    update_option_field(options, "numberOfOutputs", &mut parsed.number_of_outputs)?;

    Ok(parsed)
}

pub(crate) fn biquad_filter_type_to_str(
    value: web_audio_api_rs::node::BiquadFilterType,
) -> &'static str {
    match value {
        web_audio_api_rs::node::BiquadFilterType::Lowpass => "lowpass",
        web_audio_api_rs::node::BiquadFilterType::Highpass => "highpass",
        web_audio_api_rs::node::BiquadFilterType::Bandpass => "bandpass",
        web_audio_api_rs::node::BiquadFilterType::Lowshelf => "lowshelf",
        web_audio_api_rs::node::BiquadFilterType::Highshelf => "highshelf",
        web_audio_api_rs::node::BiquadFilterType::Peaking => "peaking",
        web_audio_api_rs::node::BiquadFilterType::Notch => "notch",
        web_audio_api_rs::node::BiquadFilterType::Allpass => "allpass",
    }
}

pub(crate) fn biquad_filter_type_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::node::BiquadFilterType> {
    match value {
        "lowpass" => Ok(web_audio_api_rs::node::BiquadFilterType::Lowpass),
        "highpass" => Ok(web_audio_api_rs::node::BiquadFilterType::Highpass),
        "bandpass" => Ok(web_audio_api_rs::node::BiquadFilterType::Bandpass),
        "lowshelf" => Ok(web_audio_api_rs::node::BiquadFilterType::Lowshelf),
        "highshelf" => Ok(web_audio_api_rs::node::BiquadFilterType::Highshelf),
        "peaking" => Ok(web_audio_api_rs::node::BiquadFilterType::Peaking),
        "notch" => Ok(web_audio_api_rs::node::BiquadFilterType::Notch),
        "allpass" => Ok(web_audio_api_rs::node::BiquadFilterType::Allpass),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'lowpass', 'highpass', 'bandpass', 'lowshelf', 'highshelf', 'peaking', 'notch', or 'allpass'",
        )),
    }
}

pub(crate) fn biquad_filter_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::BiquadFilterOptions> {
    let mut parsed = web_audio_api_rs::node::BiquadFilterOptions::default();
    let Some(options) = options_dict(options, "BiquadFilterOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    with_option_item(options, "type", |type_| {
        parsed.type_ = biquad_filter_type_from_str(type_.extract::<&str>()?)?;
        Ok(())
    })?;
    update_option_field(options, "Q", &mut parsed.q)?;
    update_option_field(options, "detune", &mut parsed.detune)?;
    update_option_field(options, "frequency", &mut parsed.frequency)?;
    update_option_field(options, "gain", &mut parsed.gain)?;

    Ok(parsed)
}

pub(crate) fn oscillator_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::OscillatorOptions> {
    let mut parsed = web_audio_api_rs::node::OscillatorOptions::default();
    let Some(options) = options_dict(options, "OscillatorOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;

    with_option_item(options, "type", |type_| {
        parsed.type_ = oscillator_type_from_str(type_.extract::<&str>()?)?;
        Ok(())
    })?;
    update_option_field(options, "frequency", &mut parsed.frequency)?;
    update_option_field(options, "detune", &mut parsed.detune)?;
    with_option_item(options, "periodicWave", |periodic_wave| {
        if !periodic_wave.is_none() {
            parsed.periodic_wave = Some(
                periodic_wave
                    .extract::<PyRef<'_, PeriodicWave>>()?
                    .0
                    .clone(),
            );
        }
        Ok(())
    })?;

    Ok(parsed)
}

pub(crate) fn iir_filter_options(
    options: &Bound<'_, PyAny>,
) -> PyResult<web_audio_api_rs::node::IIRFilterOptions> {
    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("IIRFilterOptions must be a dict"))?;

    let mut parsed = web_audio_api_rs::node::IIRFilterOptions {
        audio_node_options: web_audio_api_rs::node::AudioNodeOptions::default(),
        feedforward: options
            .get_item("feedforward")?
            .ok_or_else(|| {
                pyo3::exceptions::PyTypeError::new_err("IIRFilterOptions.feedforward is required")
            })?
            .extract()?,
        feedback: options
            .get_item("feedback")?
            .ok_or_else(|| {
                pyo3::exceptions::PyTypeError::new_err("IIRFilterOptions.feedback is required")
            })?
            .extract()?,
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;

    Ok(parsed)
}

pub(crate) fn oversample_type_to_str(
    value: web_audio_api_rs::node::OverSampleType,
) -> &'static str {
    match value {
        web_audio_api_rs::node::OverSampleType::None => "none",
        web_audio_api_rs::node::OverSampleType::X2 => "2x",
        web_audio_api_rs::node::OverSampleType::X4 => "4x",
    }
}

pub(crate) fn oversample_type_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::node::OverSampleType> {
    match value {
        "none" => Ok(web_audio_api_rs::node::OverSampleType::None),
        "2x" => Ok(web_audio_api_rs::node::OverSampleType::X2),
        "4x" => Ok(web_audio_api_rs::node::OverSampleType::X4),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'none', '2x', or '4x'",
        )),
    }
}

pub(crate) fn wave_shaper_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::WaveShaperOptions> {
    let mut parsed = web_audio_api_rs::node::WaveShaperOptions::default();
    let Some(options) = options_dict(options, "WaveShaperOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;

    with_option_item(options, "curve", |curve| {
        if !curve.is_none() {
            parsed.curve = Some(curve.extract().map_err(Into::<pyo3::PyErr>::into)?);
        }
        Ok(())
    })?;
    with_option_item(options, "oversample", |oversample| {
        parsed.oversample = oversample_type_from_str(oversample.extract::<&str>()?)?;
        Ok(())
    })?;

    Ok(parsed)
}

pub(crate) fn panning_model_type_to_str(
    value: web_audio_api_rs::node::PanningModelType,
) -> &'static str {
    match value {
        web_audio_api_rs::node::PanningModelType::EqualPower => "equalpower",
        web_audio_api_rs::node::PanningModelType::HRTF => "HRTF",
    }
}

pub(crate) fn panning_model_type_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::node::PanningModelType> {
    match value {
        "equalpower" => Ok(web_audio_api_rs::node::PanningModelType::EqualPower),
        "HRTF" => Ok(web_audio_api_rs::node::PanningModelType::HRTF),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'equalpower' or 'HRTF'",
        )),
    }
}

pub(crate) fn distance_model_type_to_str(
    value: web_audio_api_rs::node::DistanceModelType,
) -> &'static str {
    match value {
        web_audio_api_rs::node::DistanceModelType::Linear => "linear",
        web_audio_api_rs::node::DistanceModelType::Inverse => "inverse",
        web_audio_api_rs::node::DistanceModelType::Exponential => "exponential",
    }
}

pub(crate) fn distance_model_type_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::node::DistanceModelType> {
    match value {
        "linear" => Ok(web_audio_api_rs::node::DistanceModelType::Linear),
        "inverse" => Ok(web_audio_api_rs::node::DistanceModelType::Inverse),
        "exponential" => Ok(web_audio_api_rs::node::DistanceModelType::Exponential),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'linear', 'inverse', or 'exponential'",
        )),
    }
}

pub(crate) fn panner_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::PannerOptions> {
    let mut parsed = web_audio_api_rs::node::PannerOptions::default();
    let Some(options) = options_dict(options, "PannerOptions")? else {
        return Ok(parsed);
    };

    update_audio_node_options(options, &mut parsed.audio_node_options)?;
    with_option_item(options, "panningModel", |panning_model| {
        parsed.panning_model = panning_model_type_from_str(panning_model.extract::<&str>()?)?;
        Ok(())
    })?;
    with_option_item(options, "distanceModel", |distance_model| {
        parsed.distance_model = distance_model_type_from_str(distance_model.extract::<&str>()?)?;
        Ok(())
    })?;
    update_option_field(options, "positionX", &mut parsed.position_x)?;
    update_option_field(options, "positionY", &mut parsed.position_y)?;
    update_option_field(options, "positionZ", &mut parsed.position_z)?;
    update_option_field(options, "orientationX", &mut parsed.orientation_x)?;
    update_option_field(options, "orientationY", &mut parsed.orientation_y)?;
    update_option_field(options, "orientationZ", &mut parsed.orientation_z)?;
    update_option_field(options, "refDistance", &mut parsed.ref_distance)?;
    update_option_field(options, "maxDistance", &mut parsed.max_distance)?;
    update_option_field(options, "rolloffFactor", &mut parsed.rolloff_factor)?;
    update_option_field(options, "coneInnerAngle", &mut parsed.cone_inner_angle)?;
    update_option_field(options, "coneOuterAngle", &mut parsed.cone_outer_angle)?;
    update_option_field(options, "coneOuterGain", &mut parsed.cone_outer_gain)?;

    Ok(parsed)
}

pub(crate) fn periodic_wave_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::PeriodicWaveOptions> {
    let mut parsed = web_audio_api_rs::PeriodicWaveOptions::default();
    let Some(options) = options_dict(options, "PeriodicWaveOptions")? else {
        return Ok(parsed);
    };

    with_option_item(options, "real", |real| {
        parsed.real = Some(real.extract().map_err(Into::<pyo3::PyErr>::into)?);
        Ok(())
    })?;
    with_option_item(options, "imag", |imag| {
        parsed.imag = Some(imag.extract().map_err(Into::<pyo3::PyErr>::into)?);
        Ok(())
    })?;
    update_option_field(
        options,
        "disableNormalization",
        &mut parsed.disable_normalization,
    )?;

    Ok(parsed)
}

pub(crate) fn periodic_wave_constraints(
    constraints: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::PeriodicWaveOptions> {
    let mut parsed = web_audio_api_rs::PeriodicWaveOptions::default();
    let Some(constraints) = options_dict(constraints, "PeriodicWaveConstraints")? else {
        return Ok(parsed);
    };

    update_option_field(
        constraints,
        "disableNormalization",
        &mut parsed.disable_normalization,
    )?;

    Ok(parsed)
}

pub(crate) fn automation_rate_to_str(value: AutomationRate) -> &'static str {
    match value {
        AutomationRate::A => "a-rate",
        AutomationRate::K => "k-rate",
    }
}

pub(crate) fn automation_rate_from_str(value: &str) -> PyResult<AutomationRate> {
    match value {
        "a-rate" => Ok(AutomationRate::A),
        "k-rate" => Ok(AutomationRate::K),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'a-rate' or 'k-rate'",
        )),
    }
}

pub(crate) fn oscillator_type_to_str(
    value: web_audio_api_rs::node::OscillatorType,
) -> &'static str {
    match value {
        web_audio_api_rs::node::OscillatorType::Sine => "sine",
        web_audio_api_rs::node::OscillatorType::Square => "square",
        web_audio_api_rs::node::OscillatorType::Sawtooth => "sawtooth",
        web_audio_api_rs::node::OscillatorType::Triangle => "triangle",
        web_audio_api_rs::node::OscillatorType::Custom => "custom",
    }
}

pub(crate) fn oscillator_type_from_str(
    value: &str,
) -> PyResult<web_audio_api_rs::node::OscillatorType> {
    match value {
        "sine" => Ok(web_audio_api_rs::node::OscillatorType::Sine),
        "square" => Ok(web_audio_api_rs::node::OscillatorType::Square),
        "sawtooth" => Ok(web_audio_api_rs::node::OscillatorType::Sawtooth),
        "triangle" => Ok(web_audio_api_rs::node::OscillatorType::Triangle),
        "custom" => Ok(web_audio_api_rs::node::OscillatorType::Custom),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'sine', 'square', 'sawtooth', 'triangle', or 'custom'",
        )),
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct AudioParam(pub(crate) web_audio_api_rs::AudioParam);

#[pymethods]
impl AudioParam {
    #[getter(automationRate)]
    pub(crate) fn automation_rate(&self) -> String {
        automation_rate_to_str(self.0.automation_rate()).to_owned()
    }

    #[setter(automationRate)]
    pub(crate) fn set_automation_rate(&self, value: &str) -> PyResult<()> {
        let value = automation_rate_from_str(value)?;
        catch_web_audio_panic(|| self.0.set_automation_rate(value))
    }

    #[getter(defaultValue)]
    pub(crate) fn default_value(&self) -> f32 {
        self.0.default_value()
    }

    #[getter(minValue)]
    pub(crate) fn min_value(&self) -> f32 {
        self.0.min_value()
    }

    #[getter(maxValue)]
    pub(crate) fn max_value(&self) -> f32 {
        self.0.max_value()
    }

    #[getter]
    pub(crate) fn value(&self) -> PyResult<f32> {
        Ok(self.0.value())
    }

    #[setter]
    pub(crate) fn set_value(&self, value: f32) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.set_value(value);
        })
    }

    #[pyo3(name = "setValueAtTime")]
    pub(crate) fn set_value_at_time(
        slf: PyRef<'_, Self>,
        value: f32,
        start_time: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.set_value_at_time(value, start_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "linearRampToValueAtTime")]
    pub(crate) fn linear_ramp_to_value_at_time(
        slf: PyRef<'_, Self>,
        value: f32,
        end_time: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.linear_ramp_to_value_at_time(value, end_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "exponentialRampToValueAtTime")]
    pub(crate) fn exponential_ramp_to_value_at_time(
        slf: PyRef<'_, Self>,
        value: f32,
        end_time: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.exponential_ramp_to_value_at_time(value, end_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "setTargetAtTime")]
    pub(crate) fn set_target_at_time(
        slf: PyRef<'_, Self>,
        value: f32,
        start_time: f64,
        time_constant: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.set_target_at_time(value, start_time, time_constant);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "cancelScheduledValues")]
    pub(crate) fn cancel_scheduled_values(
        slf: PyRef<'_, Self>,
        cancel_time: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.cancel_scheduled_values(cancel_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "cancelAndHoldAtTime")]
    pub(crate) fn cancel_and_hold_at_time(
        slf: PyRef<'_, Self>,
        cancel_time: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.cancel_and_hold_at_time(cancel_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "setValueCurveAtTime")]
    pub(crate) fn set_value_curve_at_time(
        slf: PyRef<'_, Self>,
        values: Vec<f32>,
        start_time: f64,
        duration: f64,
    ) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.set_value_curve_at_time(&values, start_time, duration);
        })?;
        Ok(slf.into())
    }
}

#[pyclass(extends = AudioNode, subclass)]
pub(crate) struct AudioScheduledSourceNode {
    inner: ScheduledSourceInner,
}

impl AudioScheduledSourceNode {
    pub(crate) fn new(inner: ScheduledSourceInner) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl AudioScheduledSourceNode {
    #[pyo3(signature = (when=0.0))]
    pub(crate) fn start(&self, when: f64) -> PyResult<()> {
        self.inner.start_at(when)
    }

    #[pyo3(signature = (when=0.0))]
    pub(crate) fn stop(&self, when: f64) -> PyResult<()> {
        self.inner.stop_at(when)
    }

    #[getter]
    pub(crate) fn onended(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().as_super().event_handler(py, "ended")
    }

    #[setter]
    pub(crate) fn set_onended(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super().as_super().set_event_handler("ended", value);
        slf.inner.clear_onended();
        slf.inner.set_onended_registry(registry);
    }
}

#[pyclass(extends = Event)]
pub(crate) struct AudioProcessingEvent {
    event: Arc<Mutex<Option<web_audio_api_rs::AudioProcessingEvent>>>,
}

impl AudioProcessingEvent {
    fn with_event<T>(
        &self,
        f: impl FnOnce(&web_audio_api_rs::AudioProcessingEvent) -> PyResult<T>,
    ) -> PyResult<T> {
        let guard = self.event.lock().unwrap();
        let event = guard.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "AudioProcessingEvent is no longer available after the callback returns",
            )
        })?;
        f(event)
    }
}

#[pymethods]
impl AudioProcessingEvent {
    #[getter(playbackTime)]
    pub(crate) fn playback_time(&self) -> PyResult<f64> {
        self.with_event(|event| Ok(event.playback_time))
    }

    #[getter(inputBuffer)]
    pub(crate) fn input_buffer(&self) -> PyResult<AudioBuffer> {
        self.with_event(|_| {
            Ok(AudioBuffer::audio_processing(
                Arc::clone(&self.event),
                AudioProcessingBufferKind::Input,
            ))
        })
    }

    #[getter(outputBuffer)]
    pub(crate) fn output_buffer(&self) -> PyResult<AudioBuffer> {
        self.with_event(|_| {
            Ok(AudioBuffer::audio_processing(
                Arc::clone(&self.event),
                AudioProcessingBufferKind::Output,
            ))
        })
    }
}

#[pyclass(extends = AudioScheduledSourceNode)]
pub(crate) struct AudioBufferSourceNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::AudioBufferSourceNode>>,
);

#[pyclass(extends = AudioNode)]
pub(crate) struct MediaElementAudioSourceNode {
    media_element: MediaElement,
}

#[pyclass(extends = AudioNode)]
pub(crate) struct MediaStreamAudioSourceNode {
    media_stream: MediaStream,
}

#[pyclass(extends = AudioNode)]
pub(crate) struct MediaStreamTrackAudioSourceNode {
    media_stream_track: MediaStreamTrack,
}

#[pyclass(extends = AudioNode)]
pub(crate) struct MediaStreamAudioDestinationNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::MediaStreamAudioDestinationNode>>,
);

#[pymethods]
impl AudioBufferSourceNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = audio_buffer_source_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(audio_buffer_source_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(audio_buffer_source_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn buffer(&self) -> Option<AudioBuffer> {
        self.0
            .lock()
            .unwrap()
            .buffer()
            .cloned()
            .map(AudioBuffer::owned)
    }

    #[setter]
    pub(crate) fn set_buffer(&mut self, value: Option<PyRef<'_, AudioBuffer>>) -> PyResult<()> {
        if let Some(buffer) = value {
            let buffer = buffer.snapshot()?;
            catch_web_audio_panic(|| {
                self.0.lock().unwrap().set_buffer(buffer);
            })?;
        }
        Ok(())
    }

    #[getter(playbackRate)]
    pub(crate) fn playback_rate(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().playback_rate().clone())
    }

    #[getter]
    pub(crate) fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }

    #[getter(r#loop)]
    pub(crate) fn r#loop(&self) -> bool {
        self.0.lock().unwrap().loop_()
    }

    #[setter(r#loop)]
    pub(crate) fn set_loop(&mut self, value: bool) {
        self.0.lock().unwrap().set_loop(value)
    }

    #[getter(loopStart)]
    pub(crate) fn loop_start(&self) -> f64 {
        self.0.lock().unwrap().loop_start()
    }

    #[setter(loopStart)]
    pub(crate) fn set_loop_start(&mut self, value: f64) {
        self.0.lock().unwrap().set_loop_start(value)
    }

    #[getter(loopEnd)]
    pub(crate) fn loop_end(&self) -> f64 {
        self.0.lock().unwrap().loop_end()
    }

    #[setter(loopEnd)]
    pub(crate) fn set_loop_end(&mut self, value: f64) {
        self.0.lock().unwrap().set_loop_end(value)
    }

    #[pyo3(signature = (when=0.0, offset=None, duration=None))]
    pub(crate) fn start(
        &self,
        when: f64,
        offset: Option<f64>,
        duration: Option<f64>,
    ) -> PyResult<()> {
        let offset = offset.unwrap_or(0.0);
        catch_web_audio_panic(|| {
            let mut node = self.0.lock().unwrap();
            if let Some(duration) = duration {
                node.start_at_with_offset_and_duration(when, offset, duration);
            } else if offset == 0.0 {
                node.start_at(when);
            } else {
                node.start_at_with_offset(when, offset);
            }
        })
    }
}

#[pymethods]
impl MediaElementAudioSourceNode {
    #[new]
    pub(crate) fn new(
        ctx: PyRef<'_, AudioContext>,
        options: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = options.cast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("MediaElementAudioSourceOptions must be a dict")
        })?;
        let media_element = options
            .get_item("mediaElement")?
            .ok_or_else(|| {
                pyo3::exceptions::PyTypeError::new_err(
                    "MediaElementAudioSourceOptions.mediaElement is required",
                )
            })?
            .extract::<PyRef<'_, MediaElement>>()?;

        media_element_audio_source_node(ctx.0.as_ref(), &media_element)
    }

    #[getter(mediaElement)]
    pub(crate) fn media_element(&self) -> MediaElement {
        self.media_element.clone()
    }
}

#[pymethods]
impl MediaStreamAudioSourceNode {
    #[new]
    pub(crate) fn new(
        ctx: PyRef<'_, AudioContext>,
        options: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = options.cast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("MediaStreamAudioSourceOptions must be a dict")
        })?;
        let media_stream = options
            .get_item("mediaStream")?
            .ok_or_else(|| {
                pyo3::exceptions::PyTypeError::new_err(
                    "MediaStreamAudioSourceOptions.mediaStream is required",
                )
            })?
            .extract::<PyRef<'_, MediaStream>>()?;

        Ok(media_stream_audio_source_node(
            ctx.0.as_ref(),
            &media_stream.0,
        ))
    }

    #[getter(mediaStream)]
    pub(crate) fn media_stream(&self) -> MediaStream {
        self.media_stream.clone()
    }
}

#[pymethods]
impl MediaStreamTrackAudioSourceNode {
    #[new]
    pub(crate) fn new(
        ctx: PyRef<'_, AudioContext>,
        options: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = options.cast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "MediaStreamTrackAudioSourceOptions must be a dict",
            )
        })?;
        let media_stream_track = options
            .get_item("mediaStreamTrack")?
            .ok_or_else(|| {
                pyo3::exceptions::PyTypeError::new_err(
                    "MediaStreamTrackAudioSourceOptions.mediaStreamTrack is required",
                )
            })?
            .extract::<PyRef<'_, MediaStreamTrack>>()?;

        Ok(media_stream_track_audio_source_node(
            ctx.0.as_ref(),
            &media_stream_track.0,
        ))
    }

    #[getter(mediaStreamTrack)]
    pub(crate) fn media_stream_track(&self) -> MediaStreamTrack {
        self.media_stream_track.clone()
    }
}

#[pymethods]
impl MediaStreamAudioDestinationNode {
    #[getter]
    pub(crate) fn stream(&self) -> MediaStream {
        MediaStream(self.0.lock().unwrap().stream().clone())
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct AnalyserNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::AnalyserNode>>);

#[pyclass(extends = AudioNode)]
pub(crate) struct ScriptProcessorNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::ScriptProcessorNode>>,
);

#[pyclass(extends = AudioNode)]
pub(crate) struct AudioWorkletNode {
    pub(crate) inner: Arc<Mutex<web_audio_api_rs::worklet::AudioWorkletNode>>,
    parameters: AudioParamMap,
    port: Py<MessagePort>,
}

impl ScriptProcessorNode {
    pub(crate) fn clear_onaudioprocess(&self) {
        self.0.lock().unwrap().clear_onaudioprocess();
    }

    pub(crate) fn set_onaudioprocess_registry(&self, registry: Arc<Mutex<EventTargetRegistry>>) {
        self.0.lock().unwrap().set_onaudioprocess(move |event| {
            Python::attach(|py| {
                let owner = EventTarget::owner_for_registry(py, &registry);
                let event_state = Arc::new(Mutex::new(Some(event)));
                let event_obj = Py::new(
                    py,
                    PyClassInitializer::from(Event::new_dispatched(
                        "audioprocess",
                        owner.as_ref().map(|owner| owner.clone_ref(py)),
                        owner,
                    ))
                    .add_subclass(AudioProcessingEvent {
                        event: Arc::clone(&event_state),
                    }),
                );

                match event_obj {
                    Ok(event_obj) => {
                        if let Err(err) = EventTarget::dispatch_event_object(
                            py,
                            &registry,
                            "audioprocess",
                            event_obj.into_any(),
                        ) {
                            err.print(py);
                        }
                    }
                    Err(err) => err.print(py),
                }

                event_state.lock().unwrap().take();
            });
        });
    }
}

#[pymethods]
impl ScriptProcessorNode {
    #[pyo3(name = "addEventListener")]
    pub(crate) fn add_event_listener(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        type_: &str,
        listener: Py<PyAny>,
    ) -> PyResult<()> {
        if !listener.bind(py).is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "listener must be callable",
            ));
        }

        let owner = EventTarget::owner_from_ptr(py, slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super().as_super().add_listener(type_, listener);

        if type_ == "audioprocess" {
            slf.set_onaudioprocess_registry(registry);
        }

        Ok(())
    }

    #[pyo3(name = "removeEventListener")]
    pub(crate) fn remove_event_listener(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        type_: &str,
        listener: Py<PyAny>,
    ) {
        let owner = EventTarget::owner_from_ptr(py, slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        slf.as_super()
            .as_super()
            .remove_listener(py, type_, &listener);
        let keep_callback = slf.as_super().as_super().has_callbacks(type_);

        if type_ == "audioprocess" && !keep_callback {
            slf.clear_onaudioprocess();
        }
    }

    #[getter]
    pub(crate) fn onaudioprocess(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super().as_super().event_handler(py, "audioprocess")
    }

    #[setter]
    pub(crate) fn set_onaudioprocess(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super()
            .as_super()
            .set_event_handler("audioprocess", value);
        slf.clear_onaudioprocess();
        slf.set_onaudioprocess_registry(registry);
    }

    #[getter(bufferSize)]
    pub(crate) fn buffer_size(&self) -> usize {
        self.0.lock().unwrap().buffer_size()
    }
}

#[pymethods]
impl AudioWorkletNode {
    #[new]
    #[pyo3(signature = (ctx, name, options=None))]
    pub(crate) fn new(
        py: Python<'_>,
        ctx: &Bound<'_, PyAny>,
        name: &str,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<Self>> {
        let (descriptors, options) = audio_worklet_node_options(name, options)?;
        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return audio_worklet_node_py(py, ctx.0.as_ref(), descriptors, options);
        }
        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return audio_worklet_node_py(py, ctx.0.as_ref(), descriptors, options);
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn parameters(&self) -> AudioParamMap {
        self.parameters.clone()
    }

    #[getter]
    pub(crate) fn port(&self, py: Python<'_>) -> Py<MessagePort> {
        self.port.clone_ref(py)
    }

    #[getter]
    pub(crate) fn onprocessorerror(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf.as_super()
            .as_super()
            .event_handler(py, "processorerror")
    }

    #[setter]
    pub(crate) fn set_onprocessorerror(mut slf: PyRefMut<'_, Self>, value: Option<Py<PyAny>>) {
        let owner = EventTarget::owner_from_ptr(slf.py(), slf.as_ptr());
        slf.as_super().as_super().set_owner(owner);
        let registry = slf.as_super().as_super().registry();
        slf.as_super()
            .as_super()
            .set_event_handler("processorerror", value);
        slf.inner.lock().unwrap().clear_onprocessorerror();
        slf.inner
            .lock()
            .unwrap()
            .set_onprocessorerror(Box::new(move |event| {
                Python::attach(|py| match error_event_py(py, &registry, event.message) {
                    Ok(event) => {
                        if let Err(err) = EventTarget::dispatch_event_object(
                            py,
                            &registry,
                            "processorerror",
                            event,
                        ) {
                            err.print(py);
                        }
                    }
                    Err(err) => err.print(py),
                });
            }));
    }
}

#[pymethods]
impl AnalyserNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = analyser_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(analyser_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(analyser_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter(fftSize)]
    pub(crate) fn fft_size(&self) -> usize {
        self.0.lock().unwrap().fft_size()
    }

    #[setter(fftSize)]
    pub(crate) fn set_fft_size(&mut self, value: usize) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_fft_size(value))
    }

    #[getter(frequencyBinCount)]
    pub(crate) fn frequency_bin_count(&self) -> usize {
        self.0.lock().unwrap().frequency_bin_count()
    }

    #[getter(minDecibels)]
    pub(crate) fn min_decibels(&self) -> f64 {
        self.0.lock().unwrap().min_decibels()
    }

    #[setter(minDecibels)]
    pub(crate) fn set_min_decibels(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_min_decibels(value))
    }

    #[getter(maxDecibels)]
    pub(crate) fn max_decibels(&self) -> f64 {
        self.0.lock().unwrap().max_decibels()
    }

    #[setter(maxDecibels)]
    pub(crate) fn set_max_decibels(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_max_decibels(value))
    }

    #[getter(smoothingTimeConstant)]
    pub(crate) fn smoothing_time_constant(&self) -> f64 {
        self.0.lock().unwrap().smoothing_time_constant()
    }

    #[setter(smoothingTimeConstant)]
    pub(crate) fn set_smoothing_time_constant(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_smoothing_time_constant(value))
    }

    #[pyo3(name = "getFloatFrequencyData")]
    pub(crate) fn get_float_frequency_data(&mut self, mut array: Vec<f32>) -> Vec<f32> {
        self.0.lock().unwrap().get_float_frequency_data(&mut array);
        array
    }

    #[pyo3(name = "getByteFrequencyData")]
    pub(crate) fn get_byte_frequency_data(&mut self, mut array: Vec<u8>) -> Vec<u8> {
        self.0.lock().unwrap().get_byte_frequency_data(&mut array);
        array
    }

    #[pyo3(name = "getFloatTimeDomainData")]
    pub(crate) fn get_float_time_domain_data(&mut self, mut array: Vec<f32>) -> Vec<f32> {
        self.0
            .lock()
            .unwrap()
            .get_float_time_domain_data(&mut array);
        array
    }

    #[pyo3(name = "getByteTimeDomainData")]
    pub(crate) fn get_byte_time_domain_data(&mut self, mut array: Vec<u8>) -> Vec<u8> {
        self.0.lock().unwrap().get_byte_time_domain_data(&mut array);
        array
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct ConvolverNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::ConvolverNode>>);

#[pymethods]
impl ConvolverNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = convolver_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(convolver_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(convolver_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn buffer(&self) -> Option<AudioBuffer> {
        self.0
            .lock()
            .unwrap()
            .buffer()
            .cloned()
            .map(AudioBuffer::owned)
    }

    #[setter]
    pub(crate) fn set_buffer(&mut self, value: Option<PyRef<'_, AudioBuffer>>) -> PyResult<()> {
        if let Some(buffer) = value {
            let buffer = buffer.snapshot()?;
            catch_web_audio_panic(|| self.0.lock().unwrap().set_buffer(buffer))?;
        }
        Ok(())
    }

    #[getter]
    pub(crate) fn normalize(&self) -> bool {
        self.0.lock().unwrap().normalize()
    }

    #[setter]
    pub(crate) fn set_normalize(&mut self, value: bool) {
        self.0.lock().unwrap().set_normalize(value);
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct DynamicsCompressorNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::DynamicsCompressorNode>>,
);

#[pymethods]
impl DynamicsCompressorNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = dynamics_compressor_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(dynamics_compressor_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(dynamics_compressor_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn threshold(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().threshold().clone())
    }

    #[getter]
    pub(crate) fn knee(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().knee().clone())
    }

    #[getter]
    pub(crate) fn ratio(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().ratio().clone())
    }

    #[getter]
    pub(crate) fn reduction(&self) -> f32 {
        self.0.lock().unwrap().reduction()
    }

    #[getter]
    pub(crate) fn attack(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().attack().clone())
    }

    #[getter]
    pub(crate) fn release(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().release().clone())
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct GainNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::GainNode>>);

#[pymethods]
impl GainNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = gain_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(gain_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(gain_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn gain(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().gain().clone())
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct DelayNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::DelayNode>>);

#[pymethods]
impl DelayNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = delay_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(delay_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(delay_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter(delayTime)]
    pub(crate) fn delay_time(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().delay_time().clone())
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct StereoPannerNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::StereoPannerNode>>);

#[pymethods]
impl StereoPannerNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = stereo_panner_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(stereo_panner_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(stereo_panner_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn pan(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().pan().clone())
    }
}

#[pyclass(extends = AudioNode)]
#[allow(dead_code)]
pub(crate) struct ChannelMergerNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::ChannelMergerNode>>,
);

#[pymethods]
impl ChannelMergerNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = channel_merger_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(channel_merger_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_merger_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }
}

#[pyclass(extends = AudioNode)]
#[allow(dead_code)]
pub(crate) struct ChannelSplitterNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::ChannelSplitterNode>>,
);

#[pymethods]
impl ChannelSplitterNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = channel_splitter_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(channel_splitter_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_splitter_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct BiquadFilterNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::BiquadFilterNode>>);

#[pymethods]
impl BiquadFilterNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = biquad_filter_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(biquad_filter_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(biquad_filter_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn r#type(&self) -> String {
        biquad_filter_type_to_str(self.0.lock().unwrap().type_()).to_owned()
    }

    #[setter]
    pub(crate) fn set_type(&mut self, value: &str) -> PyResult<()> {
        let value = biquad_filter_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_type(value))
    }

    #[getter]
    pub(crate) fn frequency(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().frequency().clone())
    }

    #[getter]
    pub(crate) fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }

    #[getter(Q)]
    pub(crate) fn q(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().q().clone())
    }

    #[getter]
    pub(crate) fn gain(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().gain().clone())
    }

    #[pyo3(name = "getFrequencyResponse")]
    pub(crate) fn get_frequency_response(
        &self,
        frequency_hz: Vec<f32>,
    ) -> PyResult<(Vec<f32>, Vec<f32>)> {
        let mut mag_response = vec![0.0; frequency_hz.len()];
        let mut phase_response = vec![0.0; frequency_hz.len()];
        catch_web_audio_panic(|| {
            self.0.lock().unwrap().get_frequency_response(
                &frequency_hz,
                &mut mag_response,
                &mut phase_response,
            );
        })?;
        Ok((mag_response, phase_response))
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct IIRFilterNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::IIRFilterNode>>);

#[pymethods]
impl IIRFilterNode {
    #[new]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = iir_filter_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(iir_filter_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(iir_filter_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[pyo3(name = "getFrequencyResponse")]
    pub(crate) fn get_frequency_response(
        &self,
        frequency_hz: Vec<f32>,
    ) -> PyResult<(Vec<f32>, Vec<f32>)> {
        let mut mag_response = vec![0.0; frequency_hz.len()];
        let mut phase_response = vec![0.0; frequency_hz.len()];
        catch_web_audio_panic(|| {
            self.0.lock().unwrap().get_frequency_response(
                &frequency_hz,
                &mut mag_response,
                &mut phase_response,
            );
        })?;
        Ok((mag_response, phase_response))
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct WaveShaperNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::WaveShaperNode>>);

#[pymethods]
impl WaveShaperNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = wave_shaper_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(wave_shaper_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(wave_shaper_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn curve(&self) -> Option<Vec<f32>> {
        self.0.lock().unwrap().curve().map(|curve| curve.to_vec())
    }

    #[setter]
    pub(crate) fn set_curve(&mut self, value: Option<Vec<f32>>) -> PyResult<()> {
        match value {
            Some(curve) => catch_web_audio_panic(|| self.0.lock().unwrap().set_curve(curve)),
            None => Err(pyo3::exceptions::PyNotImplementedError::new_err(
                "clearing WaveShaperNode.curve is not implemented yet",
            )),
        }
    }

    #[getter]
    pub(crate) fn oversample(&self) -> String {
        oversample_type_to_str(self.0.lock().unwrap().oversample()).to_owned()
    }

    #[setter]
    pub(crate) fn set_oversample(&mut self, value: &str) -> PyResult<()> {
        let value = oversample_type_from_str(value)?;
        self.0.lock().unwrap().set_oversample(value);
        Ok(())
    }
}

#[pyclass(extends = AudioNode)]
pub(crate) struct PannerNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::PannerNode>>);

#[pymethods]
impl PannerNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = panner_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(panner_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(panner_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter(panningModel)]
    pub(crate) fn panning_model(&self) -> String {
        panning_model_type_to_str(self.0.lock().unwrap().panning_model()).to_owned()
    }

    #[setter(panningModel)]
    pub(crate) fn set_panning_model(&mut self, value: &str) -> PyResult<()> {
        let value = panning_model_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_panning_model(value))
    }

    #[getter(positionX)]
    pub(crate) fn position_x(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().position_x().clone())
    }

    #[getter(positionY)]
    pub(crate) fn position_y(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().position_y().clone())
    }

    #[getter(positionZ)]
    pub(crate) fn position_z(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().position_z().clone())
    }

    #[getter(orientationX)]
    pub(crate) fn orientation_x(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().orientation_x().clone())
    }

    #[getter(orientationY)]
    pub(crate) fn orientation_y(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().orientation_y().clone())
    }

    #[getter(orientationZ)]
    pub(crate) fn orientation_z(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().orientation_z().clone())
    }

    #[getter(distanceModel)]
    pub(crate) fn distance_model(&self) -> String {
        distance_model_type_to_str(self.0.lock().unwrap().distance_model()).to_owned()
    }

    #[setter(distanceModel)]
    pub(crate) fn set_distance_model(&mut self, value: &str) -> PyResult<()> {
        let value = distance_model_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_distance_model(value))
    }

    #[getter(refDistance)]
    pub(crate) fn ref_distance(&self) -> f64 {
        self.0.lock().unwrap().ref_distance()
    }

    #[setter(refDistance)]
    pub(crate) fn set_ref_distance(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_ref_distance(value))
    }

    #[getter(maxDistance)]
    pub(crate) fn max_distance(&self) -> f64 {
        self.0.lock().unwrap().max_distance()
    }

    #[setter(maxDistance)]
    pub(crate) fn set_max_distance(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_max_distance(value))
    }

    #[getter(rolloffFactor)]
    pub(crate) fn rolloff_factor(&self) -> f64 {
        self.0.lock().unwrap().rolloff_factor()
    }

    #[setter(rolloffFactor)]
    pub(crate) fn set_rolloff_factor(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_rolloff_factor(value))
    }

    #[getter(coneInnerAngle)]
    pub(crate) fn cone_inner_angle(&self) -> f64 {
        self.0.lock().unwrap().cone_inner_angle()
    }

    #[setter(coneInnerAngle)]
    pub(crate) fn set_cone_inner_angle(&mut self, value: f64) {
        self.0.lock().unwrap().set_cone_inner_angle(value);
    }

    #[getter(coneOuterAngle)]
    pub(crate) fn cone_outer_angle(&self) -> f64 {
        self.0.lock().unwrap().cone_outer_angle()
    }

    #[setter(coneOuterAngle)]
    pub(crate) fn set_cone_outer_angle(&mut self, value: f64) {
        self.0.lock().unwrap().set_cone_outer_angle(value);
    }

    #[getter(coneOuterGain)]
    pub(crate) fn cone_outer_gain(&self) -> f64 {
        self.0.lock().unwrap().cone_outer_gain()
    }

    #[setter(coneOuterGain)]
    pub(crate) fn set_cone_outer_gain(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_cone_outer_gain(value))
    }
}

#[pyclass(extends = AudioScheduledSourceNode)]
pub(crate) struct OscillatorNode(pub(crate) Arc<Mutex<web_audio_api_rs::node::OscillatorNode>>);

#[pymethods]
impl OscillatorNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = oscillator_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(oscillator_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(oscillator_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn r#type(&self) -> PyResult<String> {
        Ok(oscillator_type_to_str(self.0.lock().unwrap().type_()).to_owned())
    }

    #[setter]
    pub(crate) fn set_type(&mut self, value: &str) -> PyResult<()> {
        let value = oscillator_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_type(value))
    }

    #[getter]
    pub(crate) fn frequency(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().frequency().clone())
    }

    #[getter]
    pub(crate) fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }

    #[pyo3(name = "setPeriodicWave")]
    pub(crate) fn set_periodic_wave(&mut self, periodic_wave: PyRef<'_, PeriodicWave>) {
        self.0
            .lock()
            .unwrap()
            .set_periodic_wave(periodic_wave.0.clone());
    }
}

#[pyclass(extends = AudioScheduledSourceNode)]
pub(crate) struct ConstantSourceNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::ConstantSourceNode>>,
);

#[pymethods]
impl ConstantSourceNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    pub(crate) fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = constant_source_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(constant_source_node(ctx.0.as_ref(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(constant_source_node(ctx.0.as_ref(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    pub(crate) fn offset(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().offset().clone())
    }
}
