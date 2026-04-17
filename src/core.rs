use super::*;
use std::collections::HashMap;

pub(crate) static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
pub(crate) struct EventTargetRegistry {
    handlers: HashMap<String, Py<PyAny>>,
    listeners: HashMap<String, Vec<Py<PyAny>>>,
}

#[pyclass]
pub(crate) struct Event {
    type_: String,
    target: Option<Py<PyAny>>,
    current_target: Option<Py<PyAny>>,
}

impl Event {
    pub(crate) fn new_dispatched(
        type_: impl Into<String>,
        target: Option<Py<PyAny>>,
        current_target: Option<Py<PyAny>>,
    ) -> Self {
        Self {
            type_: type_.into(),
            target,
            current_target,
        }
    }
}

#[pymethods]
impl Event {
    #[new]
    pub(crate) fn new(type_: &str) -> Self {
        Self::new_dispatched(type_, None, None)
    }

    #[getter]
    pub(crate) fn r#type(&self) -> &str {
        &self.type_
    }

    #[getter]
    pub(crate) fn target(&self, py: Python<'_>) -> Py<PyAny> {
        self.target
            .as_ref()
            .map(|target| target.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    #[getter(currentTarget)]
    pub(crate) fn current_target(&self, py: Python<'_>) -> Py<PyAny> {
        self.current_target
            .as_ref()
            .map(|target| target.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }
}

#[pyclass(subclass)]
pub(crate) struct EventTarget {
    registry: Arc<Mutex<EventTargetRegistry>>,
}

impl EventTarget {
    pub(crate) fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(EventTargetRegistry::default())),
        }
    }

    pub(crate) fn registry(&self) -> Arc<Mutex<EventTargetRegistry>> {
        Arc::clone(&self.registry)
    }

    pub(crate) fn event_handler(&self, py: Python<'_>, type_: &str) -> Py<PyAny> {
        self.registry
            .lock()
            .unwrap()
            .handlers
            .get(type_)
            .map(|handler| handler.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    pub(crate) fn set_event_handler(&self, type_: &str, handler: Option<Py<PyAny>>) {
        let mut registry = self.registry.lock().unwrap();
        if let Some(handler) = handler {
            registry.handlers.insert(type_.to_owned(), handler);
        } else {
            registry.handlers.remove(type_);
        }
    }

    pub(crate) fn dispatch_from_registry(
        py: Python<'_>,
        registry: &Arc<Mutex<EventTargetRegistry>>,
        type_: &str,
        target: Option<Py<PyAny>>,
        current_target: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        let (handler, listeners) = {
            let registry = registry.lock().unwrap();
            let handler = registry.handlers.get(type_).map(|h| h.clone_ref(py));
            let listeners: Vec<Py<PyAny>> = registry
                .listeners
                .get(type_)
                .map(|listeners| listeners.iter().map(|l| l.clone_ref(py)).collect())
                .unwrap_or_default();
            (handler, listeners)
        };

        let target = target.as_ref().map(|target| target.clone_ref(py));
        let current_target = current_target
            .as_ref()
            .map(|current_target| current_target.clone_ref(py));

        let event = Py::new(py, Event::new_dispatched(type_, target, current_target))?;

        if let Some(handler) = handler {
            handler.bind(py).call1((event.clone_ref(py),))?;
        }

        for listener in listeners {
            listener.bind(py).call1((event.clone_ref(py),))?;
        }

        Ok(())
    }
}

#[pymethods]
impl EventTarget {
    #[pyo3(name = "addEventListener")]
    pub(crate) fn add_event_listener(
        &self,
        py: Python<'_>,
        type_: &str,
        listener: Py<PyAny>,
    ) -> PyResult<()> {
        if !listener.bind(py).is_callable() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "listener must be callable",
            ));
        }

        self.registry
            .lock()
            .unwrap()
            .listeners
            .entry(type_.to_owned())
            .or_default()
            .push(listener);
        Ok(())
    }

    #[pyo3(name = "removeEventListener")]
    pub(crate) fn remove_event_listener(&self, py: Python<'_>, type_: &str, listener: Py<PyAny>) {
        if let Some(listeners) = self.registry.lock().unwrap().listeners.get_mut(type_) {
            let listener_ptr = listener.bind(py).as_ptr();
            listeners.retain(|existing| existing.bind(py).as_ptr() != listener_ptr);
        }
    }

    #[pyo3(name = "dispatchEvent")]
    pub(crate) fn dispatch_event(&self, py: Python<'_>, event: PyRef<'_, Event>) -> PyResult<bool> {
        Self::dispatch_from_registry(py, &self.registry, &event.type_, None, None)?;
        Ok(true)
    }
}

#[pyclass(extends = EventTarget, subclass)]
pub(crate) struct AudioNode(pub(crate) Arc<Mutex<dyn RsAudioNode + Send + 'static>>);

#[pyclass(extends = AudioNode)]
pub(crate) struct AudioDestinationNode(
    pub(crate) Arc<Mutex<web_audio_api_rs::node::AudioDestinationNode>>,
);

#[pymethods]
impl AudioDestinationNode {
    #[getter(maxChannelCount)]
    pub(crate) fn max_channel_count(&self) -> usize {
        self.0.lock().unwrap().max_channel_count()
    }
}

#[pymethods]
impl AudioListener {
    #[getter(positionX)]
    pub(crate) fn position_x(&self) -> AudioParam {
        AudioParam(self.0.position_x().clone())
    }

    #[getter(positionY)]
    pub(crate) fn position_y(&self) -> AudioParam {
        AudioParam(self.0.position_y().clone())
    }

    #[getter(positionZ)]
    pub(crate) fn position_z(&self) -> AudioParam {
        AudioParam(self.0.position_z().clone())
    }

    #[getter(forwardX)]
    pub(crate) fn forward_x(&self) -> AudioParam {
        AudioParam(self.0.forward_x().clone())
    }

    #[getter(forwardY)]
    pub(crate) fn forward_y(&self) -> AudioParam {
        AudioParam(self.0.forward_y().clone())
    }

    #[getter(forwardZ)]
    pub(crate) fn forward_z(&self) -> AudioParam {
        AudioParam(self.0.forward_z().clone())
    }

    #[getter(upX)]
    pub(crate) fn up_x(&self) -> AudioParam {
        AudioParam(self.0.up_x().clone())
    }

    #[getter(upY)]
    pub(crate) fn up_y(&self) -> AudioParam {
        AudioParam(self.0.up_y().clone())
    }

    #[getter(upZ)]
    pub(crate) fn up_z(&self) -> AudioParam {
        AudioParam(self.0.up_z().clone())
    }

    #[pyo3(name = "setPosition")]
    pub(crate) fn set_position(&self, x: f32, y: f32, z: f32) {
        self.0.position_x().set_value(x);
        self.0.position_y().set_value(y);
        self.0.position_z().set_value(z);
    }

    #[pyo3(name = "setOrientation")]
    pub(crate) fn set_orientation(&self, x: f32, y: f32, z: f32, x_up: f32, y_up: f32, z_up: f32) {
        self.0.forward_x().set_value(x);
        self.0.forward_y().set_value(y);
        self.0.forward_z().set_value(z);
        self.0.up_x().set_value(x_up);
        self.0.up_y().set_value(y_up);
        self.0.up_z().set_value(z_up);
    }
}

impl AudioNode {
    pub(crate) fn connect_node(&self, other: &Self, output: usize, input: usize) -> PyResult<()> {
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

    pub(crate) fn disconnect_node(
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
    pub(crate) fn py_connect(
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
    pub(crate) fn context(&self, py: Python<'_>) -> PyResult<Py<BaseAudioContext>> {
        let node = lock_audio_node(&self.0)?;
        Py::new(
            py,
            BaseAudioContext::new(BaseAudioContextInner::Concrete(node.context().clone())),
        )
    }

    #[getter(numberOfInputs)]
    pub(crate) fn number_of_inputs(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.number_of_inputs())
    }

    #[getter(numberOfOutputs)]
    pub(crate) fn number_of_outputs(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.number_of_outputs())
    }

    #[getter(channelCount)]
    pub(crate) fn channel_count(&self) -> PyResult<usize> {
        let node = lock_audio_node(&self.0)?;
        Ok(node.channel_count())
    }

    #[setter(channelCount)]
    pub(crate) fn set_channel_count(&self, value: usize) -> PyResult<()> {
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_count(value))
    }

    #[getter(channelCountMode)]
    pub(crate) fn channel_count_mode(&self) -> PyResult<&'static str> {
        let node = lock_audio_node(&self.0)?;
        Ok(channel_count_mode_to_str(node.channel_count_mode()))
    }

    #[setter(channelCountMode)]
    pub(crate) fn set_channel_count_mode(&self, value: &str) -> PyResult<()> {
        let value = channel_count_mode_from_str(value)?;
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_count_mode(value))
    }

    #[getter(channelInterpretation)]
    pub(crate) fn channel_interpretation(&self) -> PyResult<&'static str> {
        let node = lock_audio_node(&self.0)?;
        Ok(channel_interpretation_to_str(node.channel_interpretation()))
    }

    #[setter(channelInterpretation)]
    pub(crate) fn set_channel_interpretation(&self, value: &str) -> PyResult<()> {
        let value = channel_interpretation_from_str(value)?;
        let node = lock_audio_node(&self.0)?;
        catch_web_audio_panic(|| node.set_channel_interpretation(value))
    }

    #[pyo3(signature = (destination_or_output=None, output=None, input=None))]
    #[pyo3(name = "disconnect")]
    pub(crate) fn py_disconnect(
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

pub(crate) fn channel_count_mode_to_str(value: ChannelCountMode) -> &'static str {
    match value {
        ChannelCountMode::Max => "max",
        ChannelCountMode::ClampedMax => "clamped-max",
        ChannelCountMode::Explicit => "explicit",
    }
}

pub(crate) fn channel_count_mode_from_str(value: &str) -> PyResult<ChannelCountMode> {
    match value {
        "max" => Ok(ChannelCountMode::Max),
        "clamped-max" => Ok(ChannelCountMode::ClampedMax),
        "explicit" => Ok(ChannelCountMode::Explicit),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'max', 'clamped-max', or 'explicit'",
        )),
    }
}

pub(crate) fn channel_interpretation_to_str(value: ChannelInterpretation) -> &'static str {
    match value {
        ChannelInterpretation::Speakers => "speakers",
        ChannelInterpretation::Discrete => "discrete",
    }
}

pub(crate) fn channel_interpretation_from_str(value: &str) -> PyResult<ChannelInterpretation> {
    match value {
        "speakers" => Ok(ChannelInterpretation::Speakers),
        "discrete" => Ok(ChannelInterpretation::Discrete),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "expected 'speakers' or 'discrete'",
        )),
    }
}

pub(crate) fn lock_audio_node<'a>(
    node: &'a Arc<Mutex<dyn RsAudioNode + Send + 'static>>,
) -> PyResult<MutexGuard<'a, dyn RsAudioNode + Send + 'static>> {
    node.lock().map_err(|_| {
        pyo3::exceptions::PyRuntimeError::new_err(
            "audio node lock was poisoned by a previous panic",
        )
    })
}

pub(crate) fn lock_pair<'a>(
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

pub(crate) fn catch_web_audio_panic(f: impl FnOnce()) -> PyResult<()> {
    catch_web_audio_panic_result(f)
}

pub(crate) fn catch_web_audio_panic_result<T>(f: impl FnOnce() -> T) -> PyResult<T> {
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

pub(crate) fn destination_node_parts(
    ctx: &impl RsBaseAudioContext,
) -> (AudioDestinationNode, AudioNode, EventTarget) {
    let dest = Arc::new(Mutex::new(ctx.destination()));
    let node = Arc::clone(&dest) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (
        AudioDestinationNode(dest),
        AudioNode(node),
        EventTarget::new(),
    )
}

pub(crate) fn destination_node(
    ctx: &impl RsBaseAudioContext,
) -> PyClassInitializer<AudioDestinationNode> {
    let (dest, node, event_target) = destination_node_parts(ctx);
    PyClassInitializer::from(event_target)
        .add_subclass(node)
        .add_subclass(dest)
}

pub(crate) fn destination_node_py(
    py: Python<'_>,
    ctx: &impl RsBaseAudioContext,
) -> PyResult<Py<AudioDestinationNode>> {
    Py::new(py, destination_node(ctx))
}
