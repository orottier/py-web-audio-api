use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::PyClass;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use web_audio_api_rs::context::{
    BaseAudioContext as RsBaseAudioContext, ConcreteBaseAudioContext as RsConcreteBaseAudioContext,
};
use web_audio_api_rs::node::{
    AudioNode as RsAudioNode, AudioScheduledSourceNode as _, ChannelCountMode,
    ChannelInterpretation,
};
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

    #[pyo3(name = "copyFromChannel", signature = (destination, channel_number, buffer_offset=0))]
    fn copy_from_channel(
        &self,
        mut destination: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<Vec<f32>> {
        catch_web_audio_panic(|| {
            self.0
                .copy_from_channel_with_offset(&mut destination, channel_number, buffer_offset);
        })?;
        Ok(destination)
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

enum BaseAudioContextInner {
    Realtime(Arc<Mutex<web_audio_api_rs::context::AudioContext>>),
    Offline(Arc<Mutex<web_audio_api_rs::context::OfflineAudioContext>>),
    Concrete(RsConcreteBaseAudioContext),
}

#[pyclass]
struct AudioListener(web_audio_api_rs::AudioListener);

#[pyclass(subclass)]
struct BaseAudioContext {
    inner: BaseAudioContextInner,
}

impl BaseAudioContext {
    fn new(inner: BaseAudioContextInner) -> Self {
        Self { inner }
    }

    #[cfg(test)]
    fn destination_inner(&self) -> AudioDestinationNode {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(&*ctx.lock().unwrap()).0,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(&*ctx.lock().unwrap()).0,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).0,
        }
    }

    #[cfg(test)]
    fn destination_audio_node(&self) -> AudioNode {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_parts(&*ctx.lock().unwrap()).1,
            BaseAudioContextInner::Offline(ctx) => destination_node_parts(&*ctx.lock().unwrap()).1,
            BaseAudioContextInner::Concrete(ctx) => destination_node_parts(ctx).1,
        }
    }

    fn listener_inner(&self) -> AudioListener {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => AudioListener(ctx.lock().unwrap().listener()),
            BaseAudioContextInner::Offline(ctx) => AudioListener(ctx.lock().unwrap().listener()),
            BaseAudioContextInner::Concrete(ctx) => AudioListener(ctx.listener()),
        }
    }
}

fn new_realtime_context() -> web_audio_api_rs::context::AudioContext {
    web_audio_api_rs::context::AudioContext::new(web_audio_api_rs::context::AudioContextOptions {
        sink_id: "none".into(),
        ..Default::default()
    })
}

#[pymethods]
impl BaseAudioContext {
    #[getter]
    fn destination(&self, py: Python<'_>) -> PyResult<Py<AudioDestinationNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => destination_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Offline(ctx) => destination_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Concrete(ctx) => destination_node_py(py, ctx),
        }
    }

    #[getter]
    fn listener(&self) -> AudioListener {
        self.listener_inner()
    }

    #[getter(sampleRate)]
    fn sample_rate(&self) -> f32 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.lock().unwrap().sample_rate(),
            BaseAudioContextInner::Offline(ctx) => ctx.lock().unwrap().sample_rate(),
            BaseAudioContextInner::Concrete(ctx) => ctx.sample_rate(),
        }
    }

    #[getter(currentTime)]
    fn current_time(&self) -> f64 {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => ctx.lock().unwrap().current_time(),
            BaseAudioContextInner::Offline(ctx) => ctx.lock().unwrap().current_time(),
            BaseAudioContextInner::Concrete(ctx) => ctx.current_time(),
        }
    }

    #[pyo3(name = "createBuffer")]
    fn create_buffer(
        &self,
        number_of_channels: usize,
        length: usize,
        sample_rate: f32,
    ) -> AudioBuffer {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => AudioBuffer(ctx.lock().unwrap().create_buffer(
                number_of_channels,
                length,
                sample_rate,
            )),
            BaseAudioContextInner::Offline(ctx) => AudioBuffer(ctx.lock().unwrap().create_buffer(
                number_of_channels,
                length,
                sample_rate,
            )),
            BaseAudioContextInner::Concrete(ctx) => {
                AudioBuffer(ctx.create_buffer(number_of_channels, length, sample_rate))
            }
        }
    }

    #[pyo3(name = "createOscillator")]
    fn create_oscillator(&self, py: Python<'_>) -> PyResult<Py<OscillatorNode>> {
        match &self.inner {
            BaseAudioContextInner::Realtime(ctx) => oscillator_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Offline(ctx) => oscillator_node_py(py, &*ctx.lock().unwrap()),
            BaseAudioContextInner::Concrete(ctx) => oscillator_node_py(py, ctx),
        }
    }

    #[pyo3(name = "createConstantSource")]
    fn create_constant_source(&self, py: Python<'_>) -> PyResult<Py<ConstantSourceNode>> {
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
    fn create_buffer_source(&self, py: Python<'_>) -> PyResult<Py<AudioBufferSourceNode>> {
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
    fn create_gain(&self, py: Python<'_>) -> PyResult<Py<GainNode>> {
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

    #[pyo3(name = "createDelay", signature = (max_delay_time=1.0))]
    fn create_delay(&self, py: Python<'_>, max_delay_time: f64) -> PyResult<Py<DelayNode>> {
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
    fn create_stereo_panner(&self, py: Python<'_>) -> PyResult<Py<StereoPannerNode>> {
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

    #[pyo3(name = "createChannelMerger", signature = (number_of_inputs=6))]
    fn create_channel_merger(
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
    fn create_channel_splitter(
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
    fn create_biquad_filter(&self, py: Python<'_>) -> PyResult<Py<BiquadFilterNode>> {
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
    fn create_analyser(&self, py: Python<'_>) -> PyResult<Py<AnalyserNode>> {
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
    fn create_convolver(&self, py: Python<'_>) -> PyResult<Py<ConvolverNode>> {
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
    fn create_dynamics_compressor(&self, py: Python<'_>) -> PyResult<Py<DynamicsCompressorNode>> {
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
struct AudioContext(Arc<Mutex<web_audio_api_rs::context::AudioContext>>);

#[pymethods]
impl AudioContext {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        let ctx = Arc::new(Mutex::new(new_realtime_context()));
        PyClassInitializer::from(BaseAudioContext::new(BaseAudioContextInner::Realtime(
            Arc::clone(&ctx),
        )))
        .add_subclass(Self(ctx))
    }
}

#[pyclass(extends = BaseAudioContext)]
struct OfflineAudioContext(Arc<Mutex<web_audio_api_rs::context::OfflineAudioContext>>);

#[pymethods]
impl OfflineAudioContext {
    #[new]
    fn new(number_of_channels: usize, length: usize, sample_rate: f32) -> PyClassInitializer<Self> {
        let ctx = Arc::new(Mutex::new(
            web_audio_api_rs::context::OfflineAudioContext::new(
                number_of_channels,
                length,
                sample_rate,
            ),
        ));
        PyClassInitializer::from(BaseAudioContext::new(BaseAudioContextInner::Offline(
            Arc::clone(&ctx),
        )))
        .add_subclass(Self(ctx))
    }

    #[pyo3(name = "startRendering")]
    fn start_rendering(&self) -> PyResult<AudioBuffer> {
        catch_web_audio_panic_result(|| AudioBuffer(self.0.lock().unwrap().start_rendering_sync()))
    }
}

#[pyclass(subclass)]
struct AudioNode(Arc<Mutex<dyn RsAudioNode + Send + 'static>>);

#[pyclass(extends = AudioNode)]
struct AudioDestinationNode(Arc<Mutex<web_audio_api_rs::node::AudioDestinationNode>>);

#[pymethods]
impl AudioDestinationNode {
    #[getter(maxChannelCount)]
    fn max_channel_count(&self) -> usize {
        self.0.lock().unwrap().max_channel_count()
    }
}

#[pymethods]
impl AudioListener {
    #[getter(positionX)]
    fn position_x(&self) -> AudioParam {
        AudioParam(self.0.position_x().clone())
    }

    #[getter(positionY)]
    fn position_y(&self) -> AudioParam {
        AudioParam(self.0.position_y().clone())
    }

    #[getter(positionZ)]
    fn position_z(&self) -> AudioParam {
        AudioParam(self.0.position_z().clone())
    }

    #[getter(forwardX)]
    fn forward_x(&self) -> AudioParam {
        AudioParam(self.0.forward_x().clone())
    }

    #[getter(forwardY)]
    fn forward_y(&self) -> AudioParam {
        AudioParam(self.0.forward_y().clone())
    }

    #[getter(forwardZ)]
    fn forward_z(&self) -> AudioParam {
        AudioParam(self.0.forward_z().clone())
    }

    #[getter(upX)]
    fn up_x(&self) -> AudioParam {
        AudioParam(self.0.up_x().clone())
    }

    #[getter(upY)]
    fn up_y(&self) -> AudioParam {
        AudioParam(self.0.up_y().clone())
    }

    #[getter(upZ)]
    fn up_z(&self) -> AudioParam {
        AudioParam(self.0.up_z().clone())
    }

    #[pyo3(name = "setPosition")]
    fn set_position(&self, x: f32, y: f32, z: f32) {
        self.0.position_x().set_value(x);
        self.0.position_y().set_value(y);
        self.0.position_z().set_value(z);
    }

    #[pyo3(name = "setOrientation")]
    fn set_orientation(&self, x: f32, y: f32, z: f32, x_up: f32, y_up: f32, z_up: f32) {
        self.0.forward_x().set_value(x);
        self.0.forward_y().set_value(y);
        self.0.forward_z().set_value(z);
        self.0.up_x().set_value(x_up);
        self.0.up_y().set_value(y_up);
        self.0.up_z().set_value(z_up);
    }
}

impl AudioNode {
    fn connect_node(&self, other: &Self, output: usize, input: usize) -> PyResult<()> {
        if Arc::ptr_eq(&self.0, &other.0) {
            let node = lock_audio_node(&self.0)?;
            return catch_web_audio_panic(|| {
                node.connect_from_output_to_input(&*node, output, input);
            });
        }

        let (source, destination) = lock_pair(&self.0, &other.0)?;
        catch_web_audio_panic(|| {
            source.connect_from_output_to_input(&*destination, output, input);
        })
    }

    fn disconnect_node(
        &self,
        other: &Self,
        output: Option<usize>,
        input: Option<usize>,
    ) -> PyResult<()> {
        if Arc::ptr_eq(&self.0, &other.0) {
            let node = lock_audio_node(&self.0)?;
            return match (output, input) {
                (None, None) => catch_web_audio_panic(|| node.disconnect_dest(&*node)),
                (Some(output), None) => {
                    catch_web_audio_panic(|| node.disconnect_dest_from_output(&*node, output))
                }
                (Some(output), Some(input)) => catch_web_audio_panic(|| {
                    node.disconnect_dest_from_output_to_input(&*node, output, input)
                }),
                (None, Some(_)) => Err(pyo3::exceptions::PyTypeError::new_err(
                    "disconnect(destinationNode, input) is not a valid overload",
                )),
            };
        }

        let (source, destination) = lock_pair(&self.0, &other.0)?;
        match (output, input) {
            (None, None) => catch_web_audio_panic(|| source.disconnect_dest(&*destination)),
            (Some(output), None) => {
                catch_web_audio_panic(|| source.disconnect_dest_from_output(&*destination, output))
            }
            (Some(output), Some(input)) => catch_web_audio_panic(|| {
                source.disconnect_dest_from_output_to_input(&*destination, output, input)
            }),
            (None, Some(_)) => Err(pyo3::exceptions::PyTypeError::new_err(
                "disconnect(destinationNode, input) is not a valid overload",
            )),
        }
    }
}

#[pymethods]
impl AudioNode {
    #[pyo3(signature = (destination, output=0, input=0))]
    #[pyo3(name = "connect")]
    fn py_connect(
        &self,
        py: Python<'_>,
        destination: &Bound<'_, PyAny>,
        output: usize,
        input: usize,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(other) = destination.extract::<PyRef<'_, AudioNode>>() {
            self.connect_node(&other, output, input)?;
            return Ok(destination.clone().unbind());
        }

        if let Ok(param) = destination.extract::<PyRef<'_, AudioParam>>() {
            let source = lock_audio_node(&self.0)?;
            catch_web_audio_panic(|| {
                source.connect_from_output_to_input(&param.0, output, 0);
            })?;
            return Ok(py.None());
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "destination must be an AudioNode or AudioParam",
        ))
    }

    #[getter]
    fn context(&self, py: Python<'_>) -> PyResult<Py<BaseAudioContext>> {
        let node = lock_audio_node(&self.0)?;
        Py::new(
            py,
            BaseAudioContext::new(BaseAudioContextInner::Concrete(node.context().clone())),
        )
    }

    #[getter(numberOfInputs)]
    fn number_of_inputs(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.number_of_inputs())
    }

    #[getter(numberOfOutputs)]
    fn number_of_outputs(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.number_of_outputs())
    }

    #[getter(channelCount)]
    fn channel_count(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.channel_count())
    }

    #[setter(channelCount)]
    fn set_channel_count(&self, value: usize) -> PyResult<()> {
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_count(value))
    }

    #[getter(channelCountMode)]
    fn channel_count_mode(&self) -> PyResult<&'static str> {
        let node = lock_audio_node(&self.0)?;
        Ok(channel_count_mode_to_str(node.channel_count_mode()))
    }

    #[setter(channelCountMode)]
    fn set_channel_count_mode(&self, value: &str) -> PyResult<()> {
        let value = channel_count_mode_from_str(value)?;
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_count_mode(value))
    }

    #[getter(channelInterpretation)]
    fn channel_interpretation(&self) -> PyResult<&'static str> {
        let node = lock_audio_node(&self.0)?;
        Ok(channel_interpretation_to_str(node.channel_interpretation()))
    }

    #[setter(channelInterpretation)]
    fn set_channel_interpretation(&self, value: &str) -> PyResult<()> {
        let value = channel_interpretation_from_str(value)?;
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_interpretation(value))
    }

    #[pyo3(signature = (destination_or_output=None, output=None, input=None))]
    #[pyo3(name = "disconnect")]
    fn py_disconnect(
        &self,
        destination_or_output: Option<&Bound<'_, PyAny>>,
        output: Option<usize>,
        input: Option<usize>,
    ) -> PyResult<()> {
        let Some(destination_or_output) = destination_or_output else {
            let node = lock_audio_node(&self.0)?;
            return catch_web_audio_panic(|| node.disconnect());
        };

        if let Ok(output_only) = destination_or_output.extract::<usize>() {
            if output.is_some() || input.is_some() {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "disconnect(output) does not accept destination output/input arguments",
                ));
            }

            let node = lock_audio_node(&self.0)?;
            return catch_web_audio_panic(|| node.disconnect_output(output_only));
        }

        if let Ok(other) = destination_or_output.extract::<PyRef<'_, AudioNode>>() {
            return self.disconnect_node(&other, output, input);
        }

        if let Ok(param) = destination_or_output.extract::<PyRef<'_, AudioParam>>() {
            if input.is_some() {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "disconnect(destinationParam, output, input) is not a valid overload",
                ));
            }

            let source = lock_audio_node(&self.0)?;
            return match output {
                None => catch_web_audio_panic(|| source.disconnect_dest(&param.0)),
                Some(output) => {
                    catch_web_audio_panic(|| source.disconnect_dest_from_output(&param.0, output))
                }
            };
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "disconnect expects no arguments, an output index, an AudioNode, or an AudioParam",
        ))
    }
}

fn channel_count_mode_to_str(value: ChannelCountMode) -> &'static str {
    match value {
        ChannelCountMode::Max => "max",
        ChannelCountMode::ClampedMax => "clamped-max",
        ChannelCountMode::Explicit => "explicit",
    }
}

fn channel_count_mode_from_str(value: &str) -> PyResult<ChannelCountMode> {
    match value {
        "max" => Ok(ChannelCountMode::Max),
        "clamped-max" => Ok(ChannelCountMode::ClampedMax),
        "explicit" => Ok(ChannelCountMode::Explicit),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'max', 'clamped-max', or 'explicit'",
        )),
    }
}

fn channel_interpretation_to_str(value: ChannelInterpretation) -> &'static str {
    match value {
        ChannelInterpretation::Speakers => "speakers",
        ChannelInterpretation::Discrete => "discrete",
    }
}

fn channel_interpretation_from_str(value: &str) -> PyResult<ChannelInterpretation> {
    match value {
        "speakers" => Ok(ChannelInterpretation::Speakers),
        "discrete" => Ok(ChannelInterpretation::Discrete),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'speakers' or 'discrete'",
        )),
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

fn destination_node_parts(ctx: &impl RsBaseAudioContext) -> (AudioDestinationNode, AudioNode) {
    let dest = Arc::new(Mutex::new(ctx.destination()));
    let node = Arc::clone(&dest) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (AudioDestinationNode(dest), AudioNode(node))
}

fn destination_node(ctx: &impl RsBaseAudioContext) -> PyClassInitializer<AudioDestinationNode> {
    let (dest, node) = destination_node_parts(ctx);
    PyClassInitializer::from(node).add_subclass(dest)
}

fn destination_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
) -> PyResult<Py<AudioDestinationNode>> {
    Py::new(py, destination_node(ctx))
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

fn wrap_audio_node<T, P>(node: T, wrap: impl FnOnce(Arc<Mutex<T>>) -> P) -> (P, AudioNode)
where
    T: RsAudioNode + Send + 'static,
{
    let node = Arc::new(Mutex::new(node));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (wrap(node), AudioNode(audio_node))
}

fn init_audio_node<T, P>(node: T, wrap: impl FnOnce(Arc<Mutex<T>>) -> P) -> PyClassInitializer<P>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioNode>,
{
    let (node, base) = wrap_audio_node(node, wrap);
    PyClassInitializer::from(base).add_subclass(node)
}

fn new_audio_node_py<T, P>(
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

fn wrap_scheduled_source_node<T, P>(
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

fn init_scheduled_source_node<T, P>(
    node: T,
    scheduled: impl FnOnce(Arc<Mutex<T>>) -> ScheduledSourceInner,
    wrap: impl FnOnce(Arc<Mutex<T>>) -> P,
) -> PyClassInitializer<P>
where
    T: RsAudioNode + Send + 'static,
    P: PyClass<BaseType = AudioScheduledSourceNode>,
{
    let (node, scheduled, base) = wrap_scheduled_source_node(node, scheduled, wrap);
    PyClassInitializer::from(base)
        .add_subclass(scheduled)
        .add_subclass(node)
}

fn new_scheduled_source_node_py<T, P>(
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
fn audio_buffer_source_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> (AudioBufferSourceNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options),
        ScheduledSourceInner::AudioBufferSource,
        AudioBufferSourceNode,
    )
}

fn audio_buffer_source_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AudioBufferSourceOptions,
) -> PyClassInitializer<AudioBufferSourceNode> {
    init_scheduled_source_node(
        web_audio_api_rs::node::AudioBufferSourceNode::new(ctx, options),
        ScheduledSourceInner::AudioBufferSource,
        AudioBufferSourceNode,
    )
}

fn audio_buffer_source_node_py(
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
fn analyser_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AnalyserOptions,
) -> (AnalyserNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::AnalyserNode::new(ctx, options),
        AnalyserNode,
    )
}

fn analyser_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::AnalyserOptions,
) -> PyClassInitializer<AnalyserNode> {
    init_audio_node(
        web_audio_api_rs::node::AnalyserNode::new(ctx, options),
        AnalyserNode,
    )
}

fn analyser_node_py(
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
fn convolver_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConvolverOptions,
) -> (ConvolverNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ConvolverNode::new(ctx, options),
        ConvolverNode,
    )
}

fn convolver_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConvolverOptions,
) -> PyClassInitializer<ConvolverNode> {
    init_audio_node(
        web_audio_api_rs::node::ConvolverNode::new(ctx, options),
        ConvolverNode,
    )
}

fn convolver_node_py(
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
fn dynamics_compressor_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DynamicsCompressorOptions,
) -> (DynamicsCompressorNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::DynamicsCompressorNode::new(ctx, options),
        DynamicsCompressorNode,
    )
}

fn dynamics_compressor_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DynamicsCompressorOptions,
) -> PyClassInitializer<DynamicsCompressorNode> {
    init_audio_node(
        web_audio_api_rs::node::DynamicsCompressorNode::new(ctx, options),
        DynamicsCompressorNode,
    )
}

fn dynamics_compressor_node_py(
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
fn gain_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> (GainNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::GainNode::new(ctx, options),
        GainNode,
    )
}

fn gain_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::GainOptions,
) -> PyClassInitializer<GainNode> {
    init_audio_node(
        web_audio_api_rs::node::GainNode::new(ctx, options),
        GainNode,
    )
}

fn gain_node_py(
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
fn delay_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> (DelayNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::DelayNode::new(ctx, options),
        DelayNode,
    )
}

fn delay_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::DelayOptions,
) -> PyClassInitializer<DelayNode> {
    init_audio_node(
        web_audio_api_rs::node::DelayNode::new(ctx, options),
        DelayNode,
    )
}

fn delay_node_py(
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
fn stereo_panner_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> (StereoPannerNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::StereoPannerNode::new(ctx, options),
        StereoPannerNode,
    )
}

fn stereo_panner_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::StereoPannerOptions,
) -> PyClassInitializer<StereoPannerNode> {
    init_audio_node(
        web_audio_api_rs::node::StereoPannerNode::new(ctx, options),
        StereoPannerNode,
    )
}

fn stereo_panner_node_py(
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
fn channel_merger_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> (ChannelMergerNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ChannelMergerNode::new(ctx, options),
        ChannelMergerNode,
    )
}

fn channel_merger_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelMergerOptions,
) -> PyClassInitializer<ChannelMergerNode> {
    init_audio_node(
        web_audio_api_rs::node::ChannelMergerNode::new(ctx, options),
        ChannelMergerNode,
    )
}

fn channel_merger_node_py(
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
fn channel_splitter_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> (ChannelSplitterNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options),
        ChannelSplitterNode,
    )
}

fn channel_splitter_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ChannelSplitterOptions,
) -> PyClassInitializer<ChannelSplitterNode> {
    init_audio_node(
        web_audio_api_rs::node::ChannelSplitterNode::new(ctx, options),
        ChannelSplitterNode,
    )
}

fn channel_splitter_node_py(
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
fn biquad_filter_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> (BiquadFilterNode, AudioNode) {
    wrap_audio_node(
        web_audio_api_rs::node::BiquadFilterNode::new(ctx, options),
        BiquadFilterNode,
    )
}

fn biquad_filter_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::BiquadFilterOptions,
) -> PyClassInitializer<BiquadFilterNode> {
    init_audio_node(
        web_audio_api_rs::node::BiquadFilterNode::new(ctx, options),
        BiquadFilterNode,
    )
}

fn biquad_filter_node_py(
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
fn oscillator_node_parts(
    ctx: &impl RsBaseAudioContext,
) -> (OscillatorNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        ctx.create_oscillator(),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

fn oscillator_node(ctx: &impl RsBaseAudioContext) -> PyClassInitializer<OscillatorNode> {
    init_scheduled_source_node(
        ctx.create_oscillator(),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

fn oscillator_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
) -> PyResult<Py<OscillatorNode>> {
    new_scheduled_source_node_py(
        py,
        ctx.create_oscillator(),
        ScheduledSourceInner::Oscillator,
        OscillatorNode,
    )
}

#[cfg(test)]
fn constant_source_node_parts(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> (ConstantSourceNode, AudioScheduledSourceNode, AudioNode) {
    wrap_scheduled_source_node(
        web_audio_api_rs::node::ConstantSourceNode::new(ctx, options),
        ScheduledSourceInner::ConstantSource,
        ConstantSourceNode,
    )
}

fn constant_source_node(
    ctx: &impl RsBaseAudioContext,
    options: web_audio_api_rs::node::ConstantSourceOptions,
) -> PyClassInitializer<ConstantSourceNode> {
    init_scheduled_source_node(
        web_audio_api_rs::node::ConstantSourceNode::new(ctx, options),
        ScheduledSourceInner::ConstantSource,
        ConstantSourceNode,
    )
}

fn constant_source_node_py(
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

fn analyser_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::AnalyserOptions> {
    let mut parsed = web_audio_api_rs::node::AnalyserOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("AnalyserOptions must be a dict"))?;

    if let Some(fft_size) = options.get_item("fftSize")? {
        parsed.fft_size = fft_size.extract()?;
    }
    if let Some(max_decibels) = options.get_item("maxDecibels")? {
        parsed.max_decibels = max_decibels.extract()?;
    }
    if let Some(min_decibels) = options.get_item("minDecibels")? {
        parsed.min_decibels = min_decibels.extract()?;
    }
    if let Some(smoothing_time_constant) = options.get_item("smoothingTimeConstant")? {
        parsed.smoothing_time_constant = smoothing_time_constant.extract()?;
    }

    Ok(parsed)
}

fn convolver_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::ConvolverOptions> {
    let mut parsed = web_audio_api_rs::node::ConvolverOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("ConvolverOptions must be a dict"))?;

    if let Some(buffer) = options.get_item("buffer")? {
        if !buffer.is_none() {
            parsed.buffer = Some(buffer.extract::<PyRef<'_, AudioBuffer>>()?.0.clone());
        }
    }
    if let Some(normalize) = options.get_item("normalize")? {
        let normalize = normalize.extract::<bool>()?;
        parsed.disable_normalization = !normalize;
    }
    if let Some(disable_normalization) = options.get_item("disableNormalization")? {
        parsed.disable_normalization = disable_normalization.extract()?;
    }

    Ok(parsed)
}

fn dynamics_compressor_options(
    options: Option<&Bound<'_, PyAny>>,
) -> PyResult<web_audio_api_rs::node::DynamicsCompressorOptions> {
    let mut parsed = web_audio_api_rs::node::DynamicsCompressorOptions::default();
    let Some(options) = options else {
        return Ok(parsed);
    };

    let options = options.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("DynamicsCompressorOptions must be a dict")
    })?;

    if let Some(attack) = options.get_item("attack")? {
        parsed.attack = attack.extract()?;
    }
    if let Some(knee) = options.get_item("knee")? {
        parsed.knee = knee.extract()?;
    }
    if let Some(ratio) = options.get_item("ratio")? {
        parsed.ratio = ratio.extract()?;
    }
    if let Some(release) = options.get_item("release")? {
        parsed.release = release.extract()?;
    }
    if let Some(threshold) = options.get_item("threshold")? {
        parsed.threshold = threshold.extract()?;
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
            return Ok(audio_buffer_source_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(audio_buffer_source_node(&*ctx.0.lock().unwrap(), options));
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
struct AnalyserNode(Arc<Mutex<web_audio_api_rs::node::AnalyserNode>>);

#[pymethods]
impl AnalyserNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = analyser_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(analyser_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(analyser_node(&*ctx.0.lock().unwrap(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter(fftSize)]
    fn fft_size(&self) -> usize {
        self.0.lock().unwrap().fft_size()
    }

    #[setter(fftSize)]
    fn set_fft_size(&mut self, value: usize) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_fft_size(value))
    }

    #[getter(frequencyBinCount)]
    fn frequency_bin_count(&self) -> usize {
        self.0.lock().unwrap().frequency_bin_count()
    }

    #[getter(minDecibels)]
    fn min_decibels(&self) -> f64 {
        self.0.lock().unwrap().min_decibels()
    }

    #[setter(minDecibels)]
    fn set_min_decibels(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_min_decibels(value))
    }

    #[getter(maxDecibels)]
    fn max_decibels(&self) -> f64 {
        self.0.lock().unwrap().max_decibels()
    }

    #[setter(maxDecibels)]
    fn set_max_decibels(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_max_decibels(value))
    }

    #[getter(smoothingTimeConstant)]
    fn smoothing_time_constant(&self) -> f64 {
        self.0.lock().unwrap().smoothing_time_constant()
    }

    #[setter(smoothingTimeConstant)]
    fn set_smoothing_time_constant(&mut self, value: f64) -> PyResult<()> {
        catch_web_audio_panic(|| self.0.lock().unwrap().set_smoothing_time_constant(value))
    }

    #[pyo3(name = "getFloatFrequencyData")]
    fn get_float_frequency_data(&mut self, mut array: Vec<f32>) -> Vec<f32> {
        self.0.lock().unwrap().get_float_frequency_data(&mut array);
        array
    }

    #[pyo3(name = "getByteFrequencyData")]
    fn get_byte_frequency_data(&mut self, mut array: Vec<u8>) -> Vec<u8> {
        self.0.lock().unwrap().get_byte_frequency_data(&mut array);
        array
    }

    #[pyo3(name = "getFloatTimeDomainData")]
    fn get_float_time_domain_data(&mut self, mut array: Vec<f32>) -> Vec<f32> {
        self.0
            .lock()
            .unwrap()
            .get_float_time_domain_data(&mut array);
        array
    }

    #[pyo3(name = "getByteTimeDomainData")]
    fn get_byte_time_domain_data(&mut self, mut array: Vec<u8>) -> Vec<u8> {
        self.0.lock().unwrap().get_byte_time_domain_data(&mut array);
        array
    }
}

#[pyclass(extends = AudioNode)]
struct ConvolverNode(Arc<Mutex<web_audio_api_rs::node::ConvolverNode>>);

#[pymethods]
impl ConvolverNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = convolver_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(convolver_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(convolver_node(&*ctx.0.lock().unwrap(), options));
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
            catch_web_audio_panic(|| self.0.lock().unwrap().set_buffer(buffer.0.clone()))?;
        }
        Ok(())
    }

    #[getter]
    fn normalize(&self) -> bool {
        self.0.lock().unwrap().normalize()
    }

    #[setter]
    fn set_normalize(&mut self, value: bool) {
        self.0.lock().unwrap().set_normalize(value);
    }
}

#[pyclass(extends = AudioNode)]
struct DynamicsCompressorNode(Arc<Mutex<web_audio_api_rs::node::DynamicsCompressorNode>>);

#[pymethods]
impl DynamicsCompressorNode {
    #[new]
    #[pyo3(signature = (ctx, options=None))]
    fn new(
        ctx: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = dynamics_compressor_options(options)?;

        if let Ok(ctx) = ctx.extract::<PyRef<'_, AudioContext>>() {
            return Ok(dynamics_compressor_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(dynamics_compressor_node(&*ctx.0.lock().unwrap(), options));
        }

        Err(pyo3::exceptions::PyTypeError::new_err(
            "expected AudioContext or OfflineAudioContext",
        ))
    }

    #[getter]
    fn threshold(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().threshold().clone())
    }

    #[getter]
    fn knee(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().knee().clone())
    }

    #[getter]
    fn ratio(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().ratio().clone())
    }

    #[getter]
    fn reduction(&self) -> f32 {
        self.0.lock().unwrap().reduction()
    }

    #[getter]
    fn attack(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().attack().clone())
    }

    #[getter]
    fn release(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().release().clone())
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
            return Ok(gain_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(gain_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(delay_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(delay_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(stereo_panner_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(stereo_panner_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(channel_merger_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_merger_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(channel_splitter_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(channel_splitter_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(biquad_filter_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(biquad_filter_node(&*ctx.0.lock().unwrap(), options));
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
            return Ok(oscillator_node(&*ctx.0.lock().unwrap()));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(oscillator_node(&*ctx.0.lock().unwrap()));
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
            return Ok(constant_source_node(&*ctx.0.lock().unwrap(), options));
        }

        if let Ok(ctx) = ctx.extract::<PyRef<'_, OfflineAudioContext>>() {
            return Ok(constant_source_node(&*ctx.0.lock().unwrap(), options));
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
    m.add_class::<BaseAudioContext>()?;
    m.add_class::<AudioContext>()?;
    m.add_class::<OfflineAudioContext>()?;
    m.add_class::<AudioBuffer>()?;
    m.add_class::<AudioListener>()?;
    m.add_class::<AudioNode>()?;
    m.add_class::<AudioDestinationNode>()?;
    m.add_class::<AudioScheduledSourceNode>()?;
    m.add_class::<AnalyserNode>()?;
    m.add_class::<ConvolverNode>()?;
    m.add_class::<DynamicsCompressorNode>()?;
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

    fn audio_context_parts() -> (AudioContext, BaseAudioContext) {
        let ctx = Arc::new(Mutex::new(new_realtime_context()));
        (
            AudioContext(Arc::clone(&ctx)),
            BaseAudioContext::new(BaseAudioContextInner::Realtime(ctx)),
        )
    }

    fn offline_context_parts(
        number_of_channels: usize,
        length: usize,
        sample_rate: f32,
    ) -> (OfflineAudioContext, BaseAudioContext) {
        let ctx = Arc::new(Mutex::new(
            web_audio_api_rs::context::OfflineAudioContext::new(
                number_of_channels,
                length,
                sample_rate,
            ),
        ));
        (
            OfflineAudioContext(Arc::clone(&ctx)),
            BaseAudioContext::new(BaseAudioContextInner::Offline(ctx)),
        )
    }

    #[test]
    fn base_audio_context_shared_surface_works() {
        let (_, realtime) = audio_context_parts();
        let (_, offline) = offline_context_parts(1, 128, 44_100.);

        assert!(realtime.sample_rate() > 0.);
        assert_eq!(offline.sample_rate(), 44_100.);
        assert!(realtime.current_time() >= 0.0);
        assert_eq!(offline.current_time(), 0.0);
        assert_eq!(realtime.create_buffer(1, 16, 8_000.).length(), 16);
        assert_eq!(offline.create_buffer(1, 16, 8_000.).length(), 16);

        let _ = realtime.destination_inner();
        let _ = offline.destination_inner();
    }

    #[test]
    fn audio_node_shared_surface_works() {
        let (ctx, _) = offline_context_parts(1, 128, 44_100.);
        let (_, gain_node) = gain_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::GainOptions::default(),
        );
        assert_eq!(gain_node.number_of_inputs().unwrap(), 1);
        assert_eq!(gain_node.number_of_outputs().unwrap(), 1);
        assert_eq!(gain_node.channel_count().unwrap(), 2);
        assert_eq!(gain_node.channel_count_mode().unwrap(), "max");
        assert_eq!(gain_node.channel_interpretation().unwrap(), "speakers");

        gain_node.set_channel_count(1).unwrap();
        gain_node.set_channel_count_mode("explicit").unwrap();
        gain_node.set_channel_interpretation("discrete").unwrap();

        assert_eq!(gain_node.channel_count().unwrap(), 1);
        assert_eq!(gain_node.channel_count_mode().unwrap(), "explicit");
        assert_eq!(gain_node.channel_interpretation().unwrap(), "discrete");
    }

    #[test]
    fn analyser_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (mut analyser, analyser_node) = analyser_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::AnalyserOptions {
                fft_size: 64,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        analyser_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(analyser.fft_size(), 64);
        assert_eq!(analyser.frequency_bin_count(), 32);
        analyser.set_smoothing_time_constant(0.5).unwrap();
        assert_eq!(analyser.smoothing_time_constant(), 0.5);
    }

    #[test]
    fn convolver_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let buffer = web_audio_api_rs::AudioBuffer::new(web_audio_api_rs::AudioBufferOptions {
            number_of_channels: 1,
            length: 8,
            sample_rate: 44_100.,
        });
        let (mut convolver, convolver_node) = convolver_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::ConvolverOptions {
                buffer: Some(buffer),
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        convolver_node.connect_node(&destination, 0, 0).unwrap();
        assert!(convolver.buffer().is_some());
        assert!(convolver.normalize());
        convolver.set_normalize(false);
        assert!(!convolver.normalize());
    }

    #[test]
    fn dynamics_compressor_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (compressor, compressor_node) = dynamics_compressor_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::DynamicsCompressorOptions {
                threshold: -18.,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        compressor_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(compressor.threshold().value().unwrap(), -18.);
        assert_eq!(compressor.knee().value().unwrap(), 30.);
        assert_eq!(compressor.ratio().value().unwrap(), 12.);
    }

    #[test]
    fn oscillator_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (osc, scheduled, osc_node) = oscillator_node_parts(&*ctx.0.lock().unwrap());
        let destination = base.destination_audio_node();

        osc_node.connect_node(&destination, 0, 0).unwrap();
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
            let (ctx, _) = offline_context_parts(1, 128, 44_100.);
            let (_, _, node) = oscillator_node_parts(&*ctx.0.lock().unwrap());
            let result = node
                .connect_node(&node, 0, 0)
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
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (src, scheduled, src_node) = constant_source_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::ConstantSourceOptions { offset: 2. },
        );
        let destination = base.destination_audio_node();

        src_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(src.offset().value().unwrap(), 2.);

        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }

    #[test]
    fn audio_buffer_source_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let buffer = web_audio_api_rs::AudioBuffer::new(web_audio_api_rs::AudioBufferOptions {
            number_of_channels: 1,
            length: 128,
            sample_rate: 44_100.,
        });
        let (src, scheduled, src_node) = audio_buffer_source_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::AudioBufferSourceOptions {
                buffer: Some(buffer),
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        src_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(src.playback_rate().value().unwrap(), 1.);
        assert_eq!(src.detune().value().unwrap(), 0.);

        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }

    #[test]
    fn gain_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (gain, gain_node) = gain_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::GainOptions {
                gain: 0.5,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        gain_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(gain.gain().value().unwrap(), 0.5);
    }

    #[test]
    fn delay_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (delay, delay_node) = delay_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::DelayOptions {
                delay_time: 0.25,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        delay_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(delay.delay_time().value().unwrap(), 0.25);
    }

    #[test]
    fn stereo_panner_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(2, 128, 44_100.);
        let (panner, panner_node) = stereo_panner_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::StereoPannerOptions {
                pan: -0.5,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        panner_node.connect_node(&destination, 0, 0).unwrap();
        assert_eq!(panner.pan().value().unwrap(), -0.5);
    }

    #[test]
    fn channel_merger_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(2, 128, 44_100.);
        let (_, merger_node) = channel_merger_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::ChannelMergerOptions {
                number_of_inputs: 2,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        merger_node.connect_node(&destination, 0, 0).unwrap();
    }

    #[test]
    fn channel_splitter_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(2, 128, 44_100.);
        let (_, splitter_node) = channel_splitter_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::ChannelSplitterOptions {
                number_of_outputs: 2,
                ..Default::default()
            },
        );
        let destination = base.destination_audio_node();

        splitter_node.connect_node(&destination, 0, 0).unwrap();
    }

    #[test]
    fn biquad_filter_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(2, 128, 44_100.);
        let (mut filter, filter_node) = biquad_filter_node_parts(
            &*ctx.0.lock().unwrap(),
            web_audio_api_rs::node::BiquadFilterOptions::default(),
        );
        let destination = base.destination_audio_node();

        filter_node.connect_node(&destination, 0, 0).unwrap();
        filter.set_type("highpass").unwrap();
        assert_eq!(filter.r#type(), "highpass");
        assert_eq!(filter.frequency().value().unwrap(), 350.);
    }
}
