use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use web_audio_api_rs::context::BaseAudioContext;
use web_audio_api_rs::node::{AudioNode as RsAudioNode, AudioScheduledSourceNode as _};
use web_audio_api_rs::AutomationRate;

static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

#[pyclass]
struct AudioBuffer(web_audio_api_rs::AudioBuffer);

#[pymethods]
impl AudioBuffer {
    #[new]
    fn new(options: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(web_audio_api_rs::AudioBuffer::new(
            audio_buffer_options(options)?,
        )))
    }

    #[getter(numberOfChannels)]
    fn number_of_channels(&self) -> usize {
        self.0.number_of_channels()
    }

    #[getter]
    fn length(&self) -> usize {
        self.0.length()
    }

    #[getter(sampleRate)]
    fn sample_rate(&self) -> f32 {
        self.0.sample_rate()
    }

    #[getter]
    fn duration(&self) -> f64 {
        self.0.duration()
    }

    #[pyo3(name = "getChannelData")]
    fn get_channel_data(&self, channel_number: usize) -> PyResult<Vec<f32>> {
        catch_web_audio_panic_result(|| self.0.get_channel_data(channel_number).to_vec())
    }

    #[pyo3(name = "copyToChannel", signature = (source, channel_number, buffer_offset=0))]
    fn copy_to_channel(
        &mut self,
        source: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0
                .copy_to_channel_with_offset(&source, channel_number, buffer_offset);
        })
    }
}

#[pyclass]
struct AudioContext(web_audio_api_rs::context::AudioContext);

#[pymethods]
impl AudioContext {
    #[new]
    fn new() -> Self {
        Self(Default::default())
    }

    #[getter]
    fn destination(&self) -> AudioNode {
        destination_node(&self.0)
    }

    #[pyo3(name = "createOscillator")]
    fn create_oscillator(&self, py: Python<'_>) -> PyResult<Py<OscillatorNode>> {
        oscillator_node_py(py, &self.0)
    }

    #[pyo3(name = "createConstantSource")]
    fn create_constant_source(&self, py: Python<'_>) -> PyResult<Py<ConstantSourceNode>> {
        constant_source_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ConstantSourceOptions::default(),
        )
    }

    #[pyo3(name = "createBufferSource")]
    fn create_buffer_source(&self, py: Python<'_>) -> PyResult<Py<AudioBufferSourceNode>> {
        audio_buffer_source_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::AudioBufferSourceOptions::default(),
        )
    }

    #[pyo3(name = "createGain")]
    fn create_gain(&self, py: Python<'_>) -> PyResult<Py<GainNode>> {
        gain_node_py(py, &self.0, web_audio_api_rs::node::GainOptions::default())
    }

    #[pyo3(name = "createDelay", signature = (max_delay_time=1.0))]
    fn create_delay(&self, py: Python<'_>, max_delay_time: f64) -> PyResult<Py<DelayNode>> {
        delay_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::DelayOptions {
                max_delay_time,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createStereoPanner")]
    fn create_stereo_panner(&self, py: Python<'_>) -> PyResult<Py<StereoPannerNode>> {
        stereo_panner_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::StereoPannerOptions::default(),
        )
    }

    #[pyo3(name = "createChannelMerger", signature = (number_of_inputs=6))]
    fn create_channel_merger(
        &self,
        py: Python<'_>,
        number_of_inputs: usize,
    ) -> PyResult<Py<ChannelMergerNode>> {
        channel_merger_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ChannelMergerOptions {
                number_of_inputs,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createChannelSplitter", signature = (number_of_outputs=6))]
    fn create_channel_splitter(
        &self,
        py: Python<'_>,
        number_of_outputs: usize,
    ) -> PyResult<Py<ChannelSplitterNode>> {
        channel_splitter_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ChannelSplitterOptions {
                number_of_outputs,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createBiquadFilter")]
    fn create_biquad_filter(&self, py: Python<'_>) -> PyResult<Py<BiquadFilterNode>> {
        biquad_filter_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::BiquadFilterOptions::default(),
        )
    }
}

#[pyclass]
struct OfflineAudioContext(web_audio_api_rs::context::OfflineAudioContext);

#[pymethods]
impl OfflineAudioContext {
    #[new]
    fn new(number_of_channels: usize, length: usize, sample_rate: f32) -> Self {
        Self(web_audio_api_rs::context::OfflineAudioContext::new(
            number_of_channels,
            length,
            sample_rate,
        ))
    }

    #[getter]
    fn destination(&self) -> AudioNode {
        destination_node(&self.0)
    }

    #[pyo3(name = "createOscillator")]
    fn create_oscillator(&self, py: Python<'_>) -> PyResult<Py<OscillatorNode>> {
        oscillator_node_py(py, &self.0)
    }

    #[pyo3(name = "createConstantSource")]
    fn create_constant_source(&self, py: Python<'_>) -> PyResult<Py<ConstantSourceNode>> {
        constant_source_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ConstantSourceOptions::default(),
        )
    }

    #[pyo3(name = "createBufferSource")]
    fn create_buffer_source(&self, py: Python<'_>) -> PyResult<Py<AudioBufferSourceNode>> {
        audio_buffer_source_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::AudioBufferSourceOptions::default(),
        )
    }

    #[pyo3(name = "createGain")]
    fn create_gain(&self, py: Python<'_>) -> PyResult<Py<GainNode>> {
        gain_node_py(py, &self.0, web_audio_api_rs::node::GainOptions::default())
    }

    #[pyo3(name = "createDelay", signature = (max_delay_time=1.0))]
    fn create_delay(&self, py: Python<'_>, max_delay_time: f64) -> PyResult<Py<DelayNode>> {
        delay_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::DelayOptions {
                max_delay_time,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createStereoPanner")]
    fn create_stereo_panner(&self, py: Python<'_>) -> PyResult<Py<StereoPannerNode>> {
        stereo_panner_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::StereoPannerOptions::default(),
        )
    }

    #[pyo3(name = "createChannelMerger", signature = (number_of_inputs=6))]
    fn create_channel_merger(
        &self,
        py: Python<'_>,
        number_of_inputs: usize,
    ) -> PyResult<Py<ChannelMergerNode>> {
        channel_merger_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ChannelMergerOptions {
                number_of_inputs,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createChannelSplitter", signature = (number_of_outputs=6))]
    fn create_channel_splitter(
        &self,
        py: Python<'_>,
        number_of_outputs: usize,
    ) -> PyResult<Py<ChannelSplitterNode>> {
        channel_splitter_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::ChannelSplitterOptions {
                number_of_outputs,
                ..Default::default()
            },
        )
    }

    #[pyo3(name = "createBiquadFilter")]
    fn create_biquad_filter(&self, py: Python<'_>) -> PyResult<Py<BiquadFilterNode>> {
        biquad_filter_node_py(
            py,
            &self.0,
            web_audio_api_rs::node::BiquadFilterOptions::default(),
        )
    }

    #[pyo3(name = "startRendering")]
    fn start_rendering(&mut self) -> PyResult<AudioBuffer> {
        catch_web_audio_panic_result(|| AudioBuffer(self.0.start_rendering_sync()))
    }
}

#[pyclass(subclass)]
struct AudioNode(Arc<Mutex<dyn RsAudioNode + Send + 'static>>);

#[pymethods]
impl AudioNode {
    fn connect(&self, other: &Self) -> PyResult<()> {
        if Arc::ptr_eq(&self.0, &other.0) {
            let node = lock_audio_node(&self.0)?;
            return catch_web_audio_panic(|| {
                node.connect(&*node);
            });
        }

        let (source, destination) = lock_pair(&self.0, &other.0)?;
        catch_web_audio_panic(|| {
            source.connect(&*destination);
        })
    }

    fn disconnect(&self, other: &Self) -> PyResult<()> {
        if Arc::ptr_eq(&self.0, &other.0) {
            let node = lock_audio_node(&self.0)?;
            return catch_web_audio_panic(|| node.disconnect_dest(&*node));
        }

        let (source, destination) = lock_pair(&self.0, &other.0)?;
        catch_web_audio_panic(|| source.disconnect_dest(&*destination))
    }
}

fn lock_audio_node<'a>(
    node: &'a Arc<Mutex<dyn RsAudioNode + Send + 'static>>,
) -> PyResult<MutexGuard<'a, dyn RsAudioNode + Send + 'static>> {
    node.lock().map_err(|_| {
        pyo3::exceptions::PyRuntimeError::new_err(
            "audio node lock was poisoned by a previous panic",
        )
    })
}

fn lock_pair<'a>(
    source: &'a Arc<Mutex<dyn RsAudioNode + Send + 'static>>,
    destination: &'a Arc<Mutex<dyn RsAudioNode + Send + 'static>>,
) -> PyResult<(
    MutexGuard<'a, dyn RsAudioNode + Send + 'static>,
    MutexGuard<'a, dyn RsAudioNode + Send + 'static>,
)> {
    let source_addr = Arc::as_ptr(source) as *const () as usize;
    let destination_addr = Arc::as_ptr(destination) as *const () as usize;

    // Always lock node pairs in the same order to avoid ABBA deadlocks.
    if source_addr < destination_addr {
        let source = lock_audio_node(source)?;
        let destination = lock_audio_node(destination)?;
        Ok((source, destination))
    } else {
        let destination = lock_audio_node(destination)?;
        let source = lock_audio_node(source)?;
        Ok((source, destination))
    }
}

fn catch_web_audio_panic(f: impl FnOnce()) -> PyResult<()> {
    catch_web_audio_panic_result(f)
}

fn catch_web_audio_panic_result<T>(f: impl FnOnce() -> T) -> PyResult<T> {
    let _guard = PANIC_HOOK_LOCK.lock().map_err(|_| {
        pyo3::exceptions::PyRuntimeError::new_err(
            "panic hook lock was poisoned by a previous panic",
        )
    })?;
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    panic::set_hook(hook);

    result.map_err(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("web-audio-api-rs panicked");

        pyo3::exceptions::PyRuntimeError::new_err(message.to_owned())
    })
}

fn destination_node(ctx: &impl BaseAudioContext) -> AudioNode {
    let dest = ctx.destination();
    let node = Arc::new(Mutex::new(dest)) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    AudioNode(node)
}

fn audio_buffer_options(
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

enum ScheduledSourceInner {
    AudioBufferSource(Arc<Mutex<web_audio_api_rs::node::AudioBufferSourceNode>>),
    Oscillator(Arc<Mutex<web_audio_api_rs::node::OscillatorNode>>),
    ConstantSource(Arc<Mutex<web_audio_api_rs::node::ConstantSourceNode>>),
}

impl ScheduledSourceInner {
    fn start_at(&self, when: f64) -> PyResult<()> {
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

    fn stop_at(&self, when: f64) -> PyResult<()> {
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
}

fn audio_buffer_source_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> (AudioBufferSourceNode, AudioScheduledSourceNode, AudioNode) {
    let node = web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (
        AudioBufferSourceNode(Arc::clone(&node)),
        AudioScheduledSourceNode::new(ScheduledSourceInner::AudioBufferSource(node)),
        AudioNode(audio_node),
    )
}

fn audio_buffer_source_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> PyClassInitializer<AudioBufferSourceNode> {
    let (node, scheduled, base) = audio_buffer_source_node_parts(ctx, options);
    PyClassInitializer::from(base)
        .add_subclass(scheduled)
        .add_subclass(node)
}

fn audio_buffer_source_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> PyResult<Py<AudioBufferSourceNode>> {
    Py::new(py, audio_buffer_source_node(ctx, options))
}

fn gain_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> (GainNode, AudioNode) {
    let node = web_audio_api_rs::node::GainNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (GainNode(node), AudioNode(audio_node))
}

fn gain_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> PyClassInitializer<GainNode> {
    let (node, base) = gain_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn gain_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> PyResult<Py<GainNode>> {
    Py::new(py, gain_node(ctx, options))
}

fn delay_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> (DelayNode, AudioNode) {
    let node = web_audio_api_rs::node::DelayNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (DelayNode(node), AudioNode(audio_node))
}

fn delay_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> PyClassInitializer<DelayNode> {
    let (node, base) = delay_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn delay_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> PyResult<Py<DelayNode>> {
    Py::new(py, delay_node(ctx, options))
}

fn stereo_panner_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> (StereoPannerNode, AudioNode) {
    let node = web_audio_api_rs::node::StereoPannerNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (StereoPannerNode(node), AudioNode(audio_node))
}

fn stereo_panner_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> PyClassInitializer<StereoPannerNode> {
    let (node, base) = stereo_panner_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn stereo_panner_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> PyResult<Py<StereoPannerNode>> {
    Py::new(py, stereo_panner_node(ctx, options))
}

fn channel_merger_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> (ChannelMergerNode, AudioNode) {
    let node = web_audio_api_rs::node::ChannelMergerNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (ChannelMergerNode(node), AudioNode(audio_node))
}

fn channel_merger_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> PyClassInitializer<ChannelMergerNode> {
    let (node, base) = channel_merger_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn channel_merger_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> PyResult<Py<ChannelMergerNode>> {
    Py::new(py, channel_merger_node(ctx, options))
}

fn channel_splitter_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> (ChannelSplitterNode, AudioNode) {
    let node = web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (ChannelSplitterNode(node), AudioNode(audio_node))
}

fn channel_splitter_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> PyClassInitializer<ChannelSplitterNode> {
    let (node, base) = channel_splitter_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn channel_splitter_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> PyResult<Py<ChannelSplitterNode>> {
    Py::new(py, channel_splitter_node(ctx, options))
}

fn biquad_filter_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> (BiquadFilterNode, AudioNode) {
    let node = web_audio_api_rs::node::BiquadFilterNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (BiquadFilterNode(node), AudioNode(audio_node))
}

fn biquad_filter_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> PyClassInitializer<BiquadFilterNode> {
    let (node, base) = biquad_filter_node_parts(ctx, options);
    PyClassInitializer::from(base).add_subclass(node)
}

fn biquad_filter_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> PyResult<Py<BiquadFilterNode>> {
    Py::new(py, biquad_filter_node(ctx, options))
}

fn oscillator_node_parts(
    ctx: &impl BaseAudioContext,
) -> (OscillatorNode, AudioScheduledSourceNode, AudioNode) {
    let osc = ctx.create_oscillator();
    let node = Arc::new(Mutex::new(osc));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (
        OscillatorNode(Arc::clone(&node)),
        AudioScheduledSourceNode::new(ScheduledSourceInner::Oscillator(node)),
        AudioNode(audio_node),
    )
}

fn oscillator_node(ctx: &impl BaseAudioContext) -> PyClassInitializer<OscillatorNode> {
    let (osc, scheduled, base) = oscillator_node_parts(ctx);
    PyClassInitializer::from(base)
        .add_subclass(scheduled)
        .add_subclass(osc)
}

fn oscillator_node_py(py: Python<'_>, ctx: &impl BaseAudioContext) -> PyResult<Py<OscillatorNode>> {
    Py::new(py, oscillator_node(ctx))
}

fn constant_source_node_parts(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> (ConstantSourceNode, AudioScheduledSourceNode, AudioNode) {
    let node = web_audio_api_rs::node::ConstantSourceNode::new(ctx, options);
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (
        ConstantSourceNode(Arc::clone(&node)),
        AudioScheduledSourceNode::new(ScheduledSourceInner::ConstantSource(node)),
        AudioNode(audio_node),
    )
}

fn constant_source_node(
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> PyClassInitializer<ConstantSourceNode> {
    let (node, scheduled, base) = constant_source_node_parts(ctx, options);
    PyClassInitializer::from(base)
        .add_subclass(scheduled)
        .add_subclass(node)
}

fn constant_source_node_py(
    py: Python<'_>,
    ctx: &impl BaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> PyResult<Py<ConstantSourceNode>> {
    Py::new(py, constant_source_node(ctx, options))
}

fn constant_source_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ConstantSourceOptions> {
    let mut parsed = web_audio_api_rs::node::ConstantSourceOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("ConstantSourceOptions must be a dict")
    })?;

    if let Some(offset) = options.get_item("offset")? {
        parsed.offset = offset.extract()?;
    }

    Ok(parsed)
}

fn audio_buffer_source_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::AudioBufferSourceOptions> {
    let mut parsed = web_audio_api_rs::node::AudioBufferSourceOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("AudioBufferSourceOptions must be a dict")
    })?;

    if let Some(buffer) = options.get_item("buffer")? {
        if !buffer.is_none() {
            parsed.buffer = Some(buffer.extract::<PyRef<'_, AudioBuffer>>()?.0.clone());
        }
    }
    if let Some(detune) = options.get_item("detune")? {
        parsed.detune = detune.extract()?;
    }
    if let Some(loop_) = options.get_item("loop")? {
        parsed.loop_ = loop_.extract()?;
    }
    if let Some(loop_end) = options.get_item("loopEnd")? {
        parsed.loop_end = loop_end.extract()?;
    }
    if let Some(loop_start) = options.get_item("loopStart")? {
        parsed.loop_start = loop_start.extract()?;
    }
    if let Some(playback_rate) = options.get_item("playbackRate")? {
        parsed.playback_rate = playback_rate.extract()?;
    }

    Ok(parsed)
}

fn gain_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::GainOptions> {
    let mut parsed = web_audio_api_rs::node::GainOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("GainOptions must be a dict"))?;

    if let Some(gain) = options.get_item("gain")? {
        parsed.gain = gain.extract()?;
    }

    Ok(parsed)
}

fn delay_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::DelayOptions> {
    let mut parsed = web_audio_api_rs::node::DelayOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("DelayOptions must be a dict"))?;

    if let Some(max_delay_time) = options.get_item("maxDelayTime")? {
        parsed.max_delay_time = max_delay_time.extract()?;
    }
    if let Some(delay_time) = options.get_item("delayTime")? {
        parsed.delay_time = delay_time.extract()?;
    }

    Ok(parsed)
}

fn stereo_panner_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::StereoPannerOptions> {
    let mut parsed = web_audio_api_rs::node::StereoPannerOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("StereoPannerOptions must be a dict")
    })?;

    if let Some(pan) = options.get_item("pan")? {
        parsed.pan = pan.extract()?;
    }

    Ok(parsed)
}

fn channel_merger_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ChannelMergerOptions> {
    let mut parsed = web_audio_api_rs::node::ChannelMergerOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("ChannelMergerOptions must be a dict")
    })?;

    if let Some(number_of_inputs) = options.get_item("numberOfInputs")? {
        parsed.number_of_inputs = number_of_inputs.extract()?;
    }

    Ok(parsed)
}

fn channel_splitter_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ChannelSplitterOptions> {
    let mut parsed = web_audio_api_rs::node::ChannelSplitterOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("ChannelSplitterOptions must be a dict")
    })?;

    if let Some(number_of_outputs) = options.get_item("numberOfOutputs")? {
        parsed.number_of_outputs = number_of_outputs.extract()?;
    }

    Ok(parsed)
}

fn biquad_filter_type_to_str(value: web_audio_api_rs::node::BiquadFilterType) -> &'static str {
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

fn biquad_filter_type_from_str(value: &str) -> PyResult<web_audio_api_rs::node::BiquadFilterType> {
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

fn biquad_filter_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::BiquadFilterOptions> {
    let mut parsed = web_audio_api_rs::node::BiquadFilterOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("BiquadFilterOptions must be a dict")
    })?;

    if let Some(type_) = options.get_item("type")? {
        parsed.type_ = biquad_filter_type_from_str(type_.extract::<&str>()?)?;
    }
    if let Some(q) = options.get_item("Q")? {
        parsed.q = q.extract()?;
    }
    if let Some(detune) = options.get_item("detune")? {
        parsed.detune = detune.extract()?;
    }
    if let Some(frequency) = options.get_item("frequency")? {
        parsed.frequency = frequency.extract()?;
    }
    if let Some(gain) = options.get_item("gain")? {
        parsed.gain = gain.extract()?;
    }

    Ok(parsed)
}

fn automation_rate_to_str(value: AutomationRate) -> &'static str {
    match value {
        AutomationRate::A => "a-rate",
        AutomationRate::K => "k-rate",
    }
}

fn automation_rate_from_str(value: &str) -> PyResult<AutomationRate> {
    match value {
        "a-rate" => Ok(AutomationRate::A),
        "k-rate" => Ok(AutomationRate::K),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'a-rate' or 'k-rate'",
        )),
    }
}

fn oscillator_type_to_str(value: web_audio_api_rs::node::OscillatorType) -> &'static str {
    match value {
        web_audio_api_rs::node::OscillatorType::Sine => "sine",
        web_audio_api_rs::node::OscillatorType::Square => "square",
        web_audio_api_rs::node::OscillatorType::Sawtooth => "sawtooth",
        web_audio_api_rs::node::OscillatorType::Triangle => "triangle",
        web_audio_api_rs::node::OscillatorType::Custom => "custom",
    }
}

fn oscillator_type_from_str(value: &str) -> PyResult<web_audio_api_rs::node::OscillatorType> {
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

#[pyclass]
struct AudioParam(web_audio_api_rs::AudioParam);

#[pymethods]
impl AudioParam {
    #[getter(automationRate)]
    fn automation_rate(&self) -> String {
        automation_rate_to_str(self.0.automation_rate()).to_owned()
    }

    #[setter(automationRate)]
    fn set_automation_rate(&self, value: &str) -> PyResult<()> {
        let value = automation_rate_from_str(value)?;
        catch_web_audio_panic(|| self.0.set_automation_rate(value))
    }

    #[getter(defaultValue)]
    fn default_value(&self) -> f32 {
        self.0.default_value()
    }

    #[getter(minValue)]
    fn min_value(&self) -> f32 {
        self.0.min_value()
    }

    #[getter(maxValue)]
    fn max_value(&self) -> f32 {
        self.0.max_value()
    }

    #[getter]
    fn value(&self) -> PyResult<f32> {
        Ok(self.0.value())
    }

    #[setter]
    fn set_value(&self, value: f32) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.set_value(value);
        })
    }

    #[pyo3(name = "setValueAtTime")]
    fn set_value_at_time(slf: PyRef<'_, Self>, value: f32, start_time: f64) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.set_value_at_time(value, start_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "linearRampToValueAtTime")]
    fn linear_ramp_to_value_at_time(
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
    fn exponential_ramp_to_value_at_time(
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
    fn set_target_at_time(
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
    fn cancel_scheduled_values(slf: PyRef<'_, Self>, cancel_time: f64) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.cancel_scheduled_values(cancel_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "cancelAndHoldAtTime")]
    fn cancel_and_hold_at_time(slf: PyRef<'_, Self>, cancel_time: f64) -> PyResult<Py<Self>> {
        catch_web_audio_panic(|| {
            slf.0.cancel_and_hold_at_time(cancel_time);
        })?;
        Ok(slf.into())
    }

    #[pyo3(name = "setValueCurveAtTime")]
    fn set_value_curve_at_time(
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
struct AudioScheduledSourceNode {
    inner: ScheduledSourceInner,
    onended: Option<Py<PyAny>>,
}

impl AudioScheduledSourceNode {
    fn new(inner: ScheduledSourceInner) -> Self {
        Self {
            inner,
            onended: None,
        }
    }
}

#[pymethods]
impl AudioScheduledSourceNode {
    #[pyo3(signature = (when=0.0))]
    fn start(&self, when: f64) -> PyResult<()> {
        self.inner.start_at(when)
    }

    #[pyo3(signature = (when=0.0))]
    fn stop(&self, when: f64) -> PyResult<()> {
        self.inner.stop_at(when)
    }

    #[getter]
    fn onended(&self, py: Python<'_>) -> Py<PyAny> {
        self.onended
            .as_ref()
            .map(|onended| onended.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    #[setter]
    fn set_onended(&mut self, value: Option<Py<PyAny>>) {
        self.onended = value;
    }
}

#[pyclass(extends = AudioScheduledSourceNode)]
struct AudioBufferSourceNode(Arc<Mutex<web_audio_api_rs::node::AudioBufferSourceNode>>);

#[pymethods]
impl AudioBufferSourceNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = audio_buffer_source_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(audio_buffer_source_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(audio_buffer_source_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn buffer(&self) -> Option<AudioBuffer> {
        self.0.lock().unwrap().buffer().cloned().map(AudioBuffer)
    }

    #[setter]
    fn set_buffer(&mut self, value: Option<PyRef<'_, AudioBuffer>>) -> PyResult<()> {
        if let Some(buffer) = value {
            catch_web_audio_panic(|| {
                self.0.lock().unwrap().set_buffer(buffer.0.clone());
            })?;
        }
        Ok(())
    }

    #[getter(playbackRate)]
    fn playback_rate(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().playback_rate().clone())
    }

    #[getter]
    fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }

    #[getter(r#loop)]
    fn r#loop(&self) -> bool {
        self.0.lock().unwrap().loop_()
    }

    #[setter(r#loop)]
    fn set_loop(&mut self, value: bool) {
        self.0.lock().unwrap().set_loop(value)
    }

    #[getter(loopStart)]
    fn loop_start(&self) -> f64 {
        self.0.lock().unwrap().loop_start()
    }

    #[setter(loopStart)]
    fn set_loop_start(&mut self, value: f64) {
        self.0.lock().unwrap().set_loop_start(value)
    }

    #[getter(loopEnd)]
    fn loop_end(&self) -> f64 {
        self.0.lock().unwrap().loop_end()
    }

    #[setter(loopEnd)]
    fn set_loop_end(&mut self, value: f64) {
        self.0.lock().unwrap().set_loop_end(value)
    }

    #[pyo3(signature = (when=0.0, offset=None, duration=None))]
    fn start(&self, when: f64, offset: Option<f64>, duration: Option<f64>) -> PyResult<()> {
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

#[pyclass(extends = AudioNode)]
struct GainNode(Arc<Mutex<web_audio_api_rs::node::GainNode>>);

#[pymethods]
impl GainNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = gain_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(gain_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(gain_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn gain(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().gain().clone())
    }
}

#[pyclass(extends = AudioNode)]
struct DelayNode(Arc<Mutex<web_audio_api_rs::node::DelayNode>>);

#[pymethods]
impl DelayNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = delay_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(delay_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(delay_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter(delayTime)]
    fn delay_time(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().delay_time().clone())
    }
}

#[pyclass(extends = AudioNode)]
struct StereoPannerNode(Arc<Mutex<web_audio_api_rs::node::StereoPannerNode>>);

#[pymethods]
impl StereoPannerNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = stereo_panner_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(stereo_panner_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(stereo_panner_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn pan(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().pan().clone())
    }
}

#[pyclass(extends = AudioNode)]
#[allow(dead_code)]
struct ChannelMergerNode(Arc<Mutex<web_audio_api_rs::node::ChannelMergerNode>>);

#[pymethods]
impl ChannelMergerNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = channel_merger_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(channel_merger_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_merger_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }
}

#[pyclass(extends = AudioNode)]
#[allow(dead_code)]
struct ChannelSplitterNode(Arc<Mutex<web_audio_api_rs::node::ChannelSplitterNode>>);

#[pymethods]
impl ChannelSplitterNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = channel_splitter_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(channel_splitter_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_splitter_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }
}

#[pyclass(extends = AudioNode)]
struct BiquadFilterNode(Arc<Mutex<web_audio_api_rs::node::BiquadFilterNode>>);

#[pymethods]
impl BiquadFilterNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = biquad_filter_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(biquad_filter_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(biquad_filter_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn r#type(&self) -> String {
        biquad_filter_type_to_str(self.0.lock().unwrap().type_()).to_owned()
    }

    #[setter]
    fn set_type(&mut self, value: &str) -> PyResult<()> {
        let value = biquad_filter_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_type(value))
    }

    #[getter]
    fn frequency(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().frequency().clone())
    }

    #[getter]
    fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }

    #[getter(Q)]
    fn q(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().q().clone())
    }

    #[getter]
    fn gain(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().gain().clone())
    }

    #[pyo3(name = "getFrequencyResponse")]
    fn get_frequency_response(&self, frequency_hz: Vec<f32>) -> PyResult<(Vec<f32>, Vec<f32>)> {
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

#[pyclass(extends = AudioScheduledSourceNode)]
struct OscillatorNode(Arc<Mutex<web_audio_api_rs::node::OscillatorNode>>);

#[pymethods]
impl OscillatorNode {
    #[new]
    fn new(ctx: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(oscillator_node(&ctx.0));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(oscillator_node(&ctx.0));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn r#type(&self) -> PyResult<String> {
        Ok(oscillator_type_to_str(self.0.lock().unwrap().type_()).to_owned())
    }

    #[setter]
    fn set_type(&mut self, value: &str) -> PyResult<()> {
        let value = oscillator_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_type(value))
    }

    #[getter]
    fn frequency(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().frequency().clone())
    }

    #[getter]
    fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }
}

#[pyclass(extends = AudioScheduledSourceNode)]
struct ConstantSourceNode(Arc<Mutex<web_audio_api_rs::node::ConstantSourceNode>>);

#[pymethods]
impl ConstantSourceNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = constant_source_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(constant_source_node(&ctx.0, options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(constant_source_node(&ctx.0, options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn offset(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().offset().clone())
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn web_audio_api(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AudioContext>()?;
    m.add_class::<OfflineAudioContext>()?;
    m.add_class::<AudioBuffer>()?;
    m.add_class::<AudioNode>()?;
    m.add_class::<AudioScheduledSourceNode>()?;
    m.add_class::<AudioBufferSourceNode>()?;
    m.add_class::<GainNode>()?;
    m.add_class::<DelayNode>()?;
    m.add_class::<StereoPannerNode>()?;
    m.add_class::<ChannelMergerNode>()?;
    m.add_class::<ChannelSplitterNode>()?;
    m.add_class::<BiquadFilterNode>()?;
    m.add_class::<OscillatorNode>()?;
    m.add_class::<ConstantSourceNode>()?;
    m.add_class::<AudioParam>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn oscillator_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(1, 128, 44_100.);
        let (osc, scheduled, osc_node) = oscillator_node_parts(&ctx.0);
        let destination = ctx.destination();

        osc_node.connect(&destination).unwrap();
        osc.frequency().set_value(300.0).unwrap();
        assert_eq!(osc.frequency().value().unwrap(), 300.0);

        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }

    #[test]
    fn self_connect_does_not_deadlock() {
        Python::initialize();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let ctx = OfflineAudioContext::new(1, 128, 44_100.);
            let (_, _, node) = oscillator_node_parts(&ctx.0);
            let result = node
                .connect(&node)
                .is_err_and(|err| err.to_string().contains("input port 0 is out of bounds"));
            let _ = tx.send(result);
        });

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)),
            Ok(true),
            "self connect did not complete before the timeout"
        );
    }

    #[test]
    fn constant_source_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(1, 128, 44_100.);
        let (src, scheduled, src_node) = constant_source_node_parts(
            &ctx.0,
            web_audio_api_rs::node::ConstantSourceOptions { offset: 2. },
        );
        let destination = ctx.destination();

        src_node.connect(&destination).unwrap();
        assert_eq!(src.offset().value().unwrap(), 2.);

        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }

    #[test]
    fn audio_buffer_source_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(1, 128, 44_100.);
        let buffer = web_audio_api_rs::AudioBuffer::new(web_audio_api_rs::AudioBufferOptions {
            number_of_channels: 1,
            length: 128,
            sample_rate: 44_100.,
        });
        let (src, scheduled, src_node) = audio_buffer_source_node_parts(
            &ctx.0,
            web_audio_api_rs::node::AudioBufferSourceOptions {
                buffer: Some(buffer),
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        src_node.connect(&destination).unwrap();
        assert_eq!(src.playback_rate().value().unwrap(), 1.);
        assert_eq!(src.detune().value().unwrap(), 0.);

        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }

    #[test]
    fn gain_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(1, 128, 44_100.);
        let (gain, gain_node) = gain_node_parts(
            &ctx.0,
            web_audio_api_rs::node::GainOptions {
                gain: 0.5,
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        gain_node.connect(&destination).unwrap();
        assert_eq!(gain.gain().value().unwrap(), 0.5);
    }

    #[test]
    fn delay_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(1, 128, 44_100.);
        let (delay, delay_node) = delay_node_parts(
            &ctx.0,
            web_audio_api_rs::node::DelayOptions {
                delay_time: 0.25,
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        delay_node.connect(&destination).unwrap();
        assert_eq!(delay.delay_time().value().unwrap(), 0.25);
    }

    #[test]
    fn stereo_panner_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(2, 128, 44_100.);
        let (panner, panner_node) = stereo_panner_node_parts(
            &ctx.0,
            web_audio_api_rs::node::StereoPannerOptions {
                pan: -0.5,
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        panner_node.connect(&destination).unwrap();
        assert_eq!(panner.pan().value().unwrap(), -0.5);
    }

    #[test]
    fn channel_merger_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(2, 128, 44_100.);
        let (_, merger_node) = channel_merger_node_parts(
            &ctx.0,
            web_audio_api_rs::node::ChannelMergerOptions {
                number_of_inputs: 2,
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        merger_node.connect(&destination).unwrap();
    }

    #[test]
    fn channel_splitter_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(2, 128, 44_100.);
        let (_, splitter_node) = channel_splitter_node_parts(
            &ctx.0,
            web_audio_api_rs::node::ChannelSplitterOptions {
                number_of_outputs: 2,
                ..Default::default()
            },
        );
        let destination = ctx.destination();

        splitter_node.connect(&destination).unwrap();
    }

    #[test]
    fn biquad_filter_graph_smoke_test() {
        let ctx = OfflineAudioContext::new(2, 128, 44_100.);
        let (mut filter, filter_node) = biquad_filter_node_parts(
            &ctx.0,
            web_audio_api_rs::node::BiquadFilterOptions::default(),
        );
        let destination = ctx.destination();

        filter_node.connect(&destination).unwrap();
        filter.set_type("highpass").unwrap();
        assert_eq!(filter.r#type(), "highpass");
        assert_eq!(filter.frequency().value().unwrap(), 350.);
    }
}
