use super::*;
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyStopIteration, PyTypeError, PyValueError};
use pyo3::types::{PyDict, PyIterator, PyList, PyModule, PyTuple, PyType};
use pyo3::IntoPyObjectExt;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::OnceLock;
use std::thread;

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct MediaElement(pub(crate) Arc<Mutex<web_audio_api_rs::MediaElement>>);

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct MediaStream(pub(crate) web_audio_api_rs::media_streams::MediaStream);

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct MediaStreamTrack(pub(crate) web_audio_api_rs::media_streams::MediaStreamTrack);

#[pyclass]
pub(crate) struct MediaStreamTrackBufferIterator {
    iter: Mutex<
        Box<
            dyn Iterator<
                    Item = Result<
                        web_audio_api_rs::AudioBuffer,
                        Box<dyn std::error::Error + Send + Sync>,
                    >,
                > + Send
                + Sync
                + 'static,
        >,
    >,
}

struct PythonMediaStreamTrackProvider {
    iterator: Py<PyAny>,
    sample_rate: Option<f32>,
    number_of_channels: Option<usize>,
}

impl PythonMediaStreamTrackProvider {
    fn buffer_from_value(
        &self,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<web_audio_api_rs::AudioBuffer> {
        if let Ok(buffer) = value.extract::<PyRef<'_, AudioBuffer>>() {
            let snapshot = buffer.snapshot()?;
            if let Some(number_of_channels) = self.number_of_channels {
                if snapshot.number_of_channels() != number_of_channels {
                    return Err(PyValueError::new_err(format!(
                        "expected {number_of_channels} channels but got {}",
                        snapshot.number_of_channels()
                    )));
                }
            }
            return Ok(snapshot);
        }

        let sample_rate = self.sample_rate.ok_or_else(|| {
            PyTypeError::new_err(
                "sampleRate is required when yielding raw sample lists from MediaStreamTrack.fromBufferIterator",
            )
        })?;

        let channels = py_to_channel_samples(value)?;
        if let Some(number_of_channels) = self.number_of_channels {
            if channels.len() != number_of_channels {
                return Err(PyValueError::new_err(format!(
                    "expected {number_of_channels} channels but got {}",
                    channels.len()
                )));
            }
        }
        Ok(web_audio_api_rs::AudioBuffer::from(channels, sample_rate))
    }
}

impl PythonMediaStreamTrackProvider {
    fn next_item(
        &self,
    ) -> Option<Result<web_audio_api_rs::AudioBuffer, Box<dyn std::error::Error + Send + Sync>>>
    {
        Python::attach(|py| match self.iterator.bind(py).call_method0("__next__") {
            Ok(value) => Some(self.buffer_from_value(&value).map_err(
                |err| -> Box<dyn std::error::Error + Send + Sync> { err.to_string().into() },
            )),
            Err(err) if err.is_instance_of::<PyStopIteration>(py) => None,
            Err(err) => Some(Err(err.to_string().into())),
        })
    }
}

struct QueuedMediaStreamTrackProvider {
    receiver: Mutex<
        std::sync::mpsc::Receiver<
            Result<web_audio_api_rs::AudioBuffer, Box<dyn std::error::Error + Send + Sync>>,
        >,
    >,
}

impl Iterator for QueuedMediaStreamTrackProvider {
    type Item = Result<web_audio_api_rs::AudioBuffer, Box<dyn std::error::Error + Send + Sync>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.lock().unwrap().recv().ok()
    }
}

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

fn media_element_path(path: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    if let Ok(path) = path.extract::<PathBuf>() {
        return Ok(path);
    }

    let os = PyModule::import(path.py(), "os")?;
    let fspath = os.getattr("fspath")?;
    fspath.call1((path,))?.extract::<PathBuf>().map_err(|_| {
        PyTypeError::new_err(
            "MediaElement path must be a string, pathlib.Path, or path-like object",
        )
    })
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

pub(crate) fn error_event_py(
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

#[derive(Clone, Debug)]
pub(crate) enum BasicMessageValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<BasicMessageValue>),
    Tuple(Vec<BasicMessageValue>),
    Dict(Vec<(String, BasicMessageValue)>),
}

pub(crate) fn py_to_basic_message_value(value: &Bound<'_, PyAny>) -> PyResult<BasicMessageValue> {
    if value.is_none() {
        return Ok(BasicMessageValue::None);
    }
    if let Ok(value) = value.extract::<bool>() {
        return Ok(BasicMessageValue::Bool(value));
    }
    if let Ok(value) = value.extract::<i64>() {
        return Ok(BasicMessageValue::Int(value));
    }
    if let Ok(value) = value.extract::<f64>() {
        return Ok(BasicMessageValue::Float(value));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(BasicMessageValue::String(value));
    }
    if let Ok(value) = value.extract::<Vec<u8>>() {
        return Ok(BasicMessageValue::Bytes(value));
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return Ok(BasicMessageValue::Tuple(
            tuple
                .iter()
                .map(|item| py_to_basic_message_value(&item))
                .collect::<PyResult<Vec<_>>>()?,
        ));
    }
    if let Ok(list) = value.cast::<PyList>() {
        return Ok(BasicMessageValue::List(
            list.iter()
                .map(|item| py_to_basic_message_value(&item))
                .collect::<PyResult<Vec<_>>>()?,
        ));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut items = Vec::with_capacity(dict.len());
        for (key, value) in dict.iter() {
            items.push((key.extract::<String>()?, py_to_basic_message_value(&value)?));
        }
        return Ok(BasicMessageValue::Dict(items));
    }

    Err(PyTypeError::new_err(
        "worklet messages and processorOptions must be composed of None, bool, int, float, str, bytes, lists, tuples, and dicts with string keys",
    ))
}

fn basic_message_value_to_py(py: Python<'_>, value: &BasicMessageValue) -> PyResult<Py<PyAny>> {
    Ok(match value {
        BasicMessageValue::None => py.None(),
        BasicMessageValue::Bool(value) => value.into_py_any(py)?,
        BasicMessageValue::Int(value) => value.into_py_any(py)?,
        BasicMessageValue::Float(value) => value.into_py_any(py)?,
        BasicMessageValue::String(value) => value.clone().into_py_any(py)?,
        BasicMessageValue::Bytes(value) => value.clone().into_py_any(py)?,
        BasicMessageValue::List(values) => {
            let items = values
                .iter()
                .map(|value| basic_message_value_to_py(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, items)?.unbind().into_any()
        }
        BasicMessageValue::Tuple(values) => {
            let items = values
                .iter()
                .map(|value| basic_message_value_to_py(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            PyTuple::new(py, items)?.unbind().into_any()
        }
        BasicMessageValue::Dict(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, basic_message_value_to_py(py, value)?)?;
            }
            dict.unbind().into_any()
        }
    })
}

#[pyclass(extends = Event)]
pub(crate) struct MessageEvent {
    data: BasicMessageValue,
}

#[pymethods]
impl MessageEvent {
    #[getter]
    pub(crate) fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        basic_message_value_to_py(py, &self.data)
    }
}

#[derive(Clone)]
enum MessagePortEndpoint {
    AudioWorkletGlobal,
    Node {
        node: Arc<Mutex<Option<Arc<Mutex<web_audio_api_rs::worklet::AudioWorkletNode>>>>>,
    },
    Processor {
        bridge_id: u64,
    },
}

pub(crate) struct MessagePortShared {
    registry: Arc<Mutex<EventTargetRegistry>>,
    endpoint: MessagePortEndpoint,
}

impl MessagePortShared {
    fn new(endpoint: MessagePortEndpoint) -> Arc<Self> {
        Arc::new(Self {
            registry: Arc::new(Mutex::new(EventTargetRegistry::default())),
            endpoint,
        })
    }
}

pub(crate) fn new_worklet_node_port_shared() -> Arc<MessagePortShared> {
    MessagePortShared::new(MessagePortEndpoint::Node {
        node: Arc::new(Mutex::new(None)),
    })
}

pub(crate) fn set_worklet_node_port_node(
    shared: &Arc<MessagePortShared>,
    node: Arc<Mutex<web_audio_api_rs::worklet::AudioWorkletNode>>,
) {
    if let MessagePortEndpoint::Node { node: slot } = &shared.endpoint {
        *slot.lock().unwrap() = Some(node);
    }
}

#[pyclass]
pub(crate) struct MessagePort {
    shared: Arc<MessagePortShared>,
}

impl MessagePort {
    pub(crate) fn new_py(py: Python<'_>, shared: Arc<MessagePortShared>) -> PyResult<Py<Self>> {
        let port = Py::new(
            py,
            Self {
                shared: Arc::clone(&shared),
            },
        )?;
        shared.registry.lock().unwrap().owner = Some(port.clone_ref(py).into_any());
        Ok(port)
    }
}

fn message_event_py(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    data: BasicMessageValue,
) -> PyResult<Py<PyAny>> {
    let owner = EventTarget::owner_for_registry(py, registry);
    let event = Py::new(
        py,
        PyClassInitializer::from(Event::new_dispatched(
            "message",
            owner.as_ref().map(|owner| owner.clone_ref(py)),
            owner.as_ref().map(|owner| owner.clone_ref(py)),
        ))
        .add_subclass(MessageEvent { data }),
    )?;
    Ok(event.into_any())
}

fn set_shared_event_handler(
    registry: &Arc<Mutex<EventTargetRegistry>>,
    type_: &str,
    handler: Option<Py<PyAny>>,
) {
    let mut registry = registry.lock().unwrap();
    if let Some(handler) = handler {
        registry.handlers.insert(type_.to_owned(), handler);
    } else {
        registry.handlers.remove(type_);
    }
}

fn add_shared_listener(
    registry: &Arc<Mutex<EventTargetRegistry>>,
    type_: &str,
    listener: Py<PyAny>,
) {
    registry
        .lock()
        .unwrap()
        .listeners
        .entry(type_.to_owned())
        .or_default()
        .push(listener);
}

fn remove_shared_listener(
    py: Python<'_>,
    registry: &Arc<Mutex<EventTargetRegistry>>,
    type_: &str,
    listener: &Py<PyAny>,
) {
    if let Some(listeners) = registry.lock().unwrap().listeners.get_mut(type_) {
        let listener_ptr = listener.bind(py).as_ptr();
        listeners.retain(|existing: &Py<PyAny>| existing.bind(py).as_ptr() != listener_ptr);
    }
}

enum WorkletCommand {
    CreateProcessor {
        bridge_id: u64,
        registration_name: String,
        options: BasicMessageValue,
        processor_port: Arc<MessagePortShared>,
        reply: SyncSender<Result<(), String>>,
    },
    ProcessQuantum {
        bridge_id: u64,
        inputs: Vec<Vec<Vec<f32>>>,
        outputs: Vec<Vec<Vec<f32>>>,
        parameters: Vec<(String, Vec<f32>)>,
        scope: PythonWorkletScopeValues,
        reply: SyncSender<Result<(Vec<Vec<Vec<f32>>>, bool), String>>,
    },
    DeliverToProcessor {
        bridge_id: u64,
        value: BasicMessageValue,
    },
    DeliverToGlobal {
        value: BasicMessageValue,
    },
    DropProcessor {
        bridge_id: u64,
    },
}

#[derive(Clone, Copy)]
struct PythonWorkletScopeValues {
    sample_rate: f32,
    current_time: f64,
    current_frame: u64,
}

struct RegisteredPythonWorklet {
    class: Py<PyAny>,
    descriptors: Vec<web_audio_api_rs::AudioParamDescriptor>,
}

struct WorkletHost {
    registrations: Mutex<HashMap<String, RegisteredPythonWorklet>>,
    sender: mpsc::Sender<WorkletCommand>,
    next_bridge_id: std::sync::atomic::AtomicU64,
    global_port: Arc<MessagePortShared>,
}

static WORKLET_HOST: OnceLock<WorkletHost> = OnceLock::new();

fn worklet_host() -> &'static WorkletHost {
    WORKLET_HOST.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        let global_port = MessagePortShared::new(MessagePortEndpoint::AudioWorkletGlobal);
        let runtime_global_port = Arc::clone(&global_port);
        thread::spawn(move || worklet_runtime_loop(receiver, runtime_global_port));
        WorkletHost {
            registrations: Mutex::new(HashMap::new()),
            sender,
            next_bridge_id: std::sync::atomic::AtomicU64::new(1),
            global_port,
        }
    })
}

fn worklet_global_port_shared() -> Arc<MessagePortShared> {
    Arc::clone(&worklet_host().global_port)
}

pub(crate) fn next_worklet_bridge_id() -> u64 {
    worklet_host()
        .next_bridge_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn register_worklet_processor(
    py: Python<'_>,
    class: &Bound<'_, PyAny>,
) -> PyResult<(String, Vec<web_audio_api_rs::AudioParamDescriptor>)> {
    let name = class
        .getattr("name")
        .map_err(|_| {
            PyTypeError::new_err(
                "AudioWorkletProcessor subclass must define class attribute 'name'",
            )
        })?
        .extract::<String>()?;
    if name.is_empty() {
        return Err(PyValueError::new_err(
            "AudioWorkletProcessor.name must be a non-empty string",
        ));
    }

    if !class.hasattr("process")? {
        return Err(PyTypeError::new_err(
            "AudioWorkletProcessor subclass must define process(self, inputs, outputs, parameters)",
        ));
    }

    let descriptors = if class.hasattr("parameterDescriptors")? {
        let raw = class.call_method0("parameterDescriptors")?;
        parse_audio_param_descriptors(&raw)?
    } else {
        Vec::new()
    };

    let host = worklet_host();
    let mut registrations = host.registrations.lock().unwrap();
    if registrations.contains_key(&name) {
        return Err(PyValueError::new_err(format!(
            "AudioWorklet processor '{name}' is already registered"
        )));
    }
    registrations.insert(
        name.clone(),
        RegisteredPythonWorklet {
            class: class.clone().unbind(),
            descriptors: descriptors.clone(),
        },
    );
    let _ = py;
    Ok((name, descriptors))
}

pub(crate) fn parse_audio_param_descriptors(
    raw: &Bound<'_, PyAny>,
) -> PyResult<Vec<web_audio_api_rs::AudioParamDescriptor>> {
    let list = raw.cast::<PyList>().map_err(|_| {
        PyTypeError::new_err("parameterDescriptors() must return a list of descriptor dicts")
    })?;
    let mut descriptors = Vec::with_capacity(list.len());
    let mut seen = std::collections::HashSet::new();
    for item in list.iter() {
        let dict = item
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("AudioParamDescriptor entries must be dicts"))?;
        let name = dict
            .get_item("name")?
            .ok_or_else(|| PyTypeError::new_err("AudioParamDescriptor.name is required"))?
            .extract::<String>()?;
        if !seen.insert(name.clone()) {
            return Err(PyValueError::new_err(format!(
                "duplicate AudioParamDescriptor name '{name}'"
            )));
        }
        let default_value = dict
            .get_item("defaultValue")?
            .map(|value| value.extract::<f32>())
            .transpose()?
            .unwrap_or(0.0);
        let min_value = dict
            .get_item("minValue")?
            .map(|value| value.extract::<f32>())
            .transpose()?
            .unwrap_or(-3.4028235e38_f32);
        let max_value = dict
            .get_item("maxValue")?
            .map(|value| value.extract::<f32>())
            .transpose()?
            .unwrap_or(3.4028235e38_f32);
        let automation_rate = dict
            .get_item("automationRate")?
            .map(|value| automation_rate_from_str(value.extract::<&str>()?))
            .transpose()?
            .unwrap_or(AutomationRate::A);
        descriptors.push(web_audio_api_rs::AudioParamDescriptor {
            name,
            default_value,
            min_value,
            max_value,
            automation_rate,
        });
    }
    Ok(descriptors)
}

fn find_processor_class_in_module<'py>(
    module: &'py Bound<'py, PyModule>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut matches = Vec::new();
    for (_, value) in module.dict().iter() {
        if let Ok(class) = value.cast::<PyType>() {
            if class.is_subclass_of::<AudioWorkletProcessor>()? {
                matches.push(value);
            }
        } else if value.is_instance_of::<AudioWorkletProcessor>() {
            matches.push(value);
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(PyTypeError::new_err(
            "module must contain exactly one AudioWorkletProcessor subclass",
        )),
        _ => Err(PyTypeError::new_err(
            "module contains more than one AudioWorkletProcessor subclass",
        )),
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct AudioParamMap {
    pub(crate) params: Vec<(String, AudioParam)>,
}

#[pymethods]
impl AudioParamMap {
    pub(crate) fn get(&self, name: &str) -> Option<AudioParam> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn keys(&self) -> Vec<String> {
        self.params.iter().map(|(key, _)| key.clone()).collect()
    }

    pub(crate) fn items(&self) -> Vec<(String, AudioParam)> {
        self.params.clone()
    }

    pub(crate) fn __len__(&self) -> usize {
        self.params.len()
    }

    pub(crate) fn __getitem__(&self, name: &str) -> PyResult<AudioParam> {
        self.get(name)
            .ok_or_else(|| PyKeyError::new_err(name.to_owned()))
    }
}

#[pyclass]
pub(crate) struct AudioWorklet {
    port: Py<MessagePort>,
}

#[pymethods]
impl AudioWorklet {
    #[pyo3(name = "addModule")]
    pub(crate) fn add_module(&self, module_or_processor: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(module) = module_or_processor.cast::<PyModule>() {
            let class = find_processor_class_in_module(module)?;
            register_worklet_processor(module.py(), &class)?;
            return Ok(());
        }
        register_worklet_processor(module_or_processor.py(), module_or_processor)?;
        Ok(())
    }

    #[getter]
    pub(crate) fn port(&self, py: Python<'_>) -> Py<MessagePort> {
        self.port.clone_ref(py)
    }
}

impl AudioWorklet {
    pub(crate) fn new_py(py: Python<'_>) -> PyResult<Py<Self>> {
        let port = MessagePort::new_py(py, worklet_global_port_shared())?;
        Py::new(py, Self { port })
    }
}

#[pyclass(subclass)]
pub(crate) struct AudioWorkletProcessor {
    port: Option<Py<MessagePort>>,
}

#[pymethods]
impl AudioWorkletProcessor {
    #[new]
    #[pyo3(signature = (_options=None))]
    pub(crate) fn new(_options: Option<&Bound<'_, PyAny>>) -> Self {
        Self { port: None }
    }

    #[getter]
    pub(crate) fn port(&self, py: Python<'_>) -> Py<PyAny> {
        self.port
            .as_ref()
            .map(|port| port.clone_ref(py).into_any())
            .unwrap_or_else(|| py.None())
    }
}

#[pymethods]
impl MessagePort {
    #[getter]
    pub(crate) fn onmessage(&self, py: Python<'_>) -> Py<PyAny> {
        self.shared
            .registry
            .lock()
            .unwrap()
            .handlers
            .get("message")
            .map(|handler| handler.clone_ref(py))
            .unwrap_or_else(|| py.None())
    }

    #[setter]
    pub(crate) fn set_onmessage(&self, value: Option<Py<PyAny>>) {
        set_shared_event_handler(&self.shared.registry, "message", value);
    }

    #[pyo3(name = "addEventListener")]
    pub(crate) fn add_event_listener(
        &self,
        py: Python<'_>,
        type_: &str,
        listener: Py<PyAny>,
    ) -> PyResult<()> {
        if !listener.bind(py).is_callable() {
            return Err(PyTypeError::new_err("listener must be callable"));
        }
        add_shared_listener(&self.shared.registry, type_, listener);
        Ok(())
    }

    #[pyo3(name = "removeEventListener")]
    pub(crate) fn remove_event_listener(&self, py: Python<'_>, type_: &str, listener: Py<PyAny>) {
        remove_shared_listener(py, &self.shared.registry, type_, &listener);
    }

    #[pyo3(name = "postMessage")]
    pub(crate) fn post_message(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = py_to_basic_message_value(value)?;
        match &self.shared.endpoint {
            MessagePortEndpoint::AudioWorkletGlobal => worklet_host()
                .sender
                .send(WorkletCommand::DeliverToGlobal { value })
                .map_err(|err| PyRuntimeError::new_err(err.to_string())),
            MessagePortEndpoint::Node { node } => catch_web_audio_panic(|| {
                let node = node
                    .lock()
                    .unwrap()
                    .as_ref()
                    .expect("worklet node port is not attached yet")
                    .clone();
                node.lock().unwrap().port().post_message(value);
            }),
            MessagePortEndpoint::Processor { bridge_id } => {
                dispatch_message_to_node(*bridge_id, value)
            }
        }
    }
}

fn dispatch_message_to_node(bridge_id: u64, value: BasicMessageValue) -> PyResult<()> {
    if let Some(shared) = bridge_node_port_shared(bridge_id) {
        Python::attach(|py| {
            let event = message_event_py(py, &shared.registry, value)?;
            EventTarget::dispatch_event_object(py, &shared.registry, "message", event)
        })?;
    }
    Ok(())
}

fn dispatch_message_to_port_registry(
    py: Python<'_>,
    shared: &Arc<MessagePortShared>,
    value: BasicMessageValue,
) -> PyResult<()> {
    let event = message_event_py(py, &shared.registry, value)?;
    EventTarget::dispatch_event_object(py, &shared.registry, "message", event)
}

struct PythonWorkletInstance {
    processor: Py<PyAny>,
    processor_globals: Py<PyDict>,
    processor_port: Arc<MessagePortShared>,
}

fn processor_globals_for_instance<'py>(
    py: Python<'py>,
    instance: &Bound<'py, PyAny>,
) -> PyResult<Py<PyDict>> {
    let globals = instance
        .getattr("process")?
        .getattr("__func__")?
        .getattr("__globals__")?
        .cast_into::<PyDict>()?;
    let _ = py;
    Ok(globals.unbind())
}

fn with_patched_processor_globals<T>(
    py: Python<'_>,
    globals: &Bound<'_, PyDict>,
    scope: PythonWorkletScopeValues,
    f: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let names = [
        (
            "sampleRate",
            scope.sample_rate.into_pyobject(py)?.unbind().into_any(),
        ),
        (
            "currentTime",
            scope.current_time.into_pyobject(py)?.unbind().into_any(),
        ),
        (
            "currentFrame",
            scope.current_frame.into_pyobject(py)?.unbind().into_any(),
        ),
    ];

    let mut previous = Vec::with_capacity(names.len());
    for (name, value) in &names {
        let old_value = if let Some(existing) = globals.get_item(name)? {
            Some(existing.unbind())
        } else {
            None
        };
        globals.set_item(name, value.bind(py))?;
        previous.push((*name, old_value));
    }

    let result = f();

    for (name, old_value) in previous.into_iter().rev() {
        match old_value {
            Some(value) => globals.set_item(name, value.bind(py))?,
            None => {
                globals.del_item(name)?;
            }
        }
    }

    result
}

fn worklet_runtime_loop(receiver: Receiver<WorkletCommand>, global_port: Arc<MessagePortShared>) {
    let mut processors: HashMap<u64, PythonWorkletInstance> = HashMap::new();
    while let Ok(command) = receiver.recv() {
        match command {
            WorkletCommand::CreateProcessor {
                bridge_id,
                registration_name,
                options,
                processor_port,
                reply,
            } => {
                let result = Python::attach(|py| -> PyResult<()> {
                    let host = worklet_host();
                    let registrations = host.registrations.lock().unwrap();
                    let registration = registrations.get(&registration_name).ok_or_else(|| {
                        PyRuntimeError::new_err(format!(
                            "AudioWorklet processor '{registration_name}' is not registered"
                        ))
                    })?;
                    let options_py = basic_message_value_to_py(py, &options)?;
                    let instance = registration.class.bind(py).call1((options_py,))?;
                    let processor_globals = processor_globals_for_instance(py, &instance)?;
                    let port = MessagePort::new_py(py, Arc::clone(&processor_port))?;
                    let mut base = instance.extract::<PyRefMut<'_, AudioWorkletProcessor>>()?;
                    base.port = Some(port);
                    drop(base);
                    processors.insert(
                        bridge_id,
                        PythonWorkletInstance {
                            processor: instance.unbind(),
                            processor_globals,
                            processor_port,
                        },
                    );
                    Ok(())
                })
                .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            WorkletCommand::ProcessQuantum {
                bridge_id,
                inputs,
                outputs,
                parameters,
                scope,
                reply,
            } => {
                let result = Python::attach(|py| -> PyResult<(Vec<Vec<Vec<f32>>>, bool)> {
                    let instance = processors.get(&bridge_id).ok_or_else(|| {
                        PyRuntimeError::new_err("worklet processor instance is missing")
                    })?;

                    let inputs_py = PyList::new(
                        py,
                        inputs
                            .iter()
                            .map(|input| {
                                PyList::new(py, input.iter().map(|channel| channel.clone()))
                            })
                            .collect::<PyResult<Vec<_>>>()?,
                    )?;

                    let output_lists = outputs
                        .iter()
                        .map(|output| PyList::new(py, output.iter().map(|channel| channel.clone())))
                        .collect::<PyResult<Vec<_>>>()?;
                    let outputs_py = PyList::new(py, output_lists)?;

                    let params_py = PyDict::new(py);
                    for (name, values) in parameters {
                        params_py.set_item(name, values)?;
                    }

                    let keepalive = with_patched_processor_globals(
                        py,
                        &instance.processor_globals.bind(py),
                        scope,
                        || {
                            instance
                                .processor
                                .bind(py)
                                .call_method1(
                                    "process",
                                    (inputs_py, outputs_py.clone(), params_py),
                                )?
                                .extract::<bool>()
                        },
                    )?;

                    let mut copied_outputs = Vec::new();
                    for output in outputs_py.iter() {
                        let output = output.cast::<PyList>()?;
                        let mut output_channels = Vec::new();
                        for channel in output.iter() {
                            output_channels.push(channel.extract::<Vec<f32>>()?);
                        }
                        copied_outputs.push(output_channels);
                    }

                    Ok((copied_outputs, keepalive))
                })
                .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            WorkletCommand::DeliverToProcessor { bridge_id, value } => {
                let _ = Python::attach(|py| -> PyResult<()> {
                    let Some(instance) = processors.get(&bridge_id) else {
                        return Ok(());
                    };
                    let value_py = basic_message_value_to_py(py, &value)?;
                    if instance.processor.bind(py).hasattr("onmessage")? {
                        instance
                            .processor
                            .bind(py)
                            .call_method1("onmessage", (value_py.clone_ref(py),))?;
                    }
                    dispatch_message_to_port_registry(py, &instance.processor_port, value)?;
                    Ok(())
                });
            }
            WorkletCommand::DeliverToGlobal { value } => {
                let _ =
                    Python::attach(|py| dispatch_message_to_port_registry(py, &global_port, value));
            }
            WorkletCommand::DropProcessor { bridge_id } => {
                processors.remove(&bridge_id);
            }
        }
    }
}

fn bridge_construct_processor(
    bridge_id: u64,
    registration_name: String,
    options: BasicMessageValue,
    processor_port: Arc<MessagePortShared>,
) -> Result<(), String> {
    let (send, recv) = mpsc::sync_channel(1);
    worklet_host()
        .sender
        .send(WorkletCommand::CreateProcessor {
            bridge_id,
            registration_name,
            options,
            processor_port,
            reply: send,
        })
        .map_err(|err| err.to_string())?;
    recv.recv().map_err(|err| err.to_string())?
}

fn bridge_process_quantum(
    bridge_id: u64,
    inputs: Vec<Vec<Vec<f32>>>,
    outputs: Vec<Vec<Vec<f32>>>,
    parameters: Vec<(String, Vec<f32>)>,
    scope: PythonWorkletScopeValues,
) -> Result<(Vec<Vec<Vec<f32>>>, bool), String> {
    let (send, recv) = mpsc::sync_channel(1);
    worklet_host()
        .sender
        .send(WorkletCommand::ProcessQuantum {
            bridge_id,
            inputs,
            outputs,
            parameters,
            scope,
            reply: send,
        })
        .map_err(|err| err.to_string())?;
    recv.recv().map_err(|err| err.to_string())?
}

fn bridge_deliver_to_processor(bridge_id: u64, value: BasicMessageValue) {
    let _ = worklet_host()
        .sender
        .send(WorkletCommand::DeliverToProcessor { bridge_id, value });
}

fn bridge_drop_processor(bridge_id: u64) {
    let _ = worklet_host()
        .sender
        .send(WorkletCommand::DropProcessor { bridge_id });
}

struct BridgeDescriptorContext {
    descriptors: Vec<web_audio_api_rs::AudioParamDescriptor>,
}

thread_local! {
    static BRIDGE_DESCRIPTOR_CONTEXT: RefCell<Option<BridgeDescriptorContext>> = const { RefCell::new(None) };
}

pub(crate) fn with_worklet_descriptors<T>(
    descriptors: Vec<web_audio_api_rs::AudioParamDescriptor>,
    f: impl FnOnce() -> T,
) -> T {
    BRIDGE_DESCRIPTOR_CONTEXT.with(|context| {
        *context.borrow_mut() = Some(BridgeDescriptorContext { descriptors });
        let result = f();
        *context.borrow_mut() = None;
        result
    })
}

fn current_worklet_descriptors() -> Vec<web_audio_api_rs::AudioParamDescriptor> {
    BRIDGE_DESCRIPTOR_CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .map(|context| context.descriptors.clone())
            .unwrap_or_default()
    })
}

#[derive(Clone)]
pub(crate) struct PythonWorkletBridgeOptions {
    pub(crate) bridge_id: u64,
    pub(crate) registration_name: String,
    pub(crate) processor_options: BasicMessageValue,
    pub(crate) node_port: Arc<MessagePortShared>,
}

impl Default for PythonWorkletBridgeOptions {
    fn default() -> Self {
        Self {
            bridge_id: 0,
            registration_name: String::new(),
            processor_options: BasicMessageValue::None,
            node_port: new_worklet_node_port_shared(),
        }
    }
}

pub(crate) struct PythonWorkletBridgeProcessor {
    bridge_id: u64,
    dead: bool,
}

impl Drop for PythonWorkletBridgeProcessor {
    fn drop(&mut self) {
        bridge_drop_processor(self.bridge_id);
    }
}

impl web_audio_api_rs::worklet::AudioWorkletProcessor for PythonWorkletBridgeProcessor {
    type ProcessorOptions = PythonWorkletBridgeOptions;

    fn constructor(opts: Self::ProcessorOptions) -> Self {
        let processor_port = MessagePortShared::new(MessagePortEndpoint::Processor {
            bridge_id: opts.bridge_id,
        });
        bridge_construct_processor(
            opts.bridge_id,
            opts.registration_name.clone(),
            opts.processor_options,
            Arc::clone(&processor_port),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        register_bridge_node_port(opts.bridge_id, opts.node_port);
        Self {
            bridge_id: opts.bridge_id,
            dead: false,
        }
    }

    fn parameter_descriptors() -> Vec<web_audio_api_rs::AudioParamDescriptor> {
        current_worklet_descriptors()
    }

    fn process<'a, 'b>(
        &mut self,
        inputs: &'b [&'a [&'a [f32]]],
        outputs: &'b mut [&'a mut [&'a mut [f32]]],
        params: web_audio_api_rs::worklet::AudioParamValues<'b>,
        scope: &'b web_audio_api_rs::worklet::AudioWorkletGlobalScope,
    ) -> bool {
        if self.dead {
            for output in outputs.iter_mut() {
                for channel in output.iter_mut() {
                    channel.fill(0.0);
                }
            }
            return false;
        }

        let inputs_vec = inputs
            .iter()
            .map(|input| {
                input
                    .iter()
                    .map(|channel| channel.to_vec())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let outputs_template = outputs
            .iter()
            .map(|output| {
                output
                    .iter()
                    .map(|channel| vec![0.0; channel.len()])
                    .collect()
            })
            .collect::<Vec<Vec<Vec<f32>>>>();
        let parameters = params
            .keys()
            .map(|key| (key.to_owned(), params.get(key).to_vec()))
            .collect::<Vec<_>>();
        let scope_values = PythonWorkletScopeValues {
            sample_rate: scope.sample_rate,
            current_time: scope.current_time,
            current_frame: scope.current_frame,
        };

        let (rendered_outputs, keepalive) = bridge_process_quantum(
            self.bridge_id,
            inputs_vec,
            outputs_template,
            parameters,
            scope_values,
        )
        .unwrap_or_else(|err| panic!("{err}"));

        for (output, rendered) in outputs.iter_mut().zip(rendered_outputs.iter()) {
            for (channel, rendered_channel) in output.iter_mut().zip(rendered.iter()) {
                channel.copy_from_slice(rendered_channel);
            }
        }
        keepalive
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        if let Some(value) = msg.downcast_mut::<BasicMessageValue>() {
            bridge_deliver_to_processor(self.bridge_id, value.clone());
        }
    }
}

static WORKLET_NODE_PORTS: OnceLock<Mutex<HashMap<u64, Arc<MessagePortShared>>>> = OnceLock::new();

fn worklet_node_ports() -> &'static Mutex<HashMap<u64, Arc<MessagePortShared>>> {
    WORKLET_NODE_PORTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_bridge_node_port(bridge_id: u64, shared: Arc<MessagePortShared>) {
    worklet_node_ports()
        .lock()
        .unwrap()
        .insert(bridge_id, shared);
}

fn bridge_node_port_shared(bridge_id: u64) -> Option<Arc<MessagePortShared>> {
    worklet_node_ports()
        .lock()
        .unwrap()
        .get(&bridge_id)
        .cloned()
}

pub(crate) fn registered_worklet_descriptors(
    name: &str,
) -> PyResult<Vec<web_audio_api_rs::AudioParamDescriptor>> {
    worklet_host()
        .registrations
        .lock()
        .unwrap()
        .get(name)
        .map(|registration| registration.descriptors.clone())
        .ok_or_else(|| {
            PyRuntimeError::new_err(format!("AudioWorklet processor '{name}' is not registered"))
        })
}

#[pymethods]
impl MediaElement {
    #[new]
    pub(crate) fn new(path: &Bound<'_, PyAny>) -> PyResult<Self> {
        let path = media_element_path(path)?;
        let inner = catch_web_audio_panic_result(|| web_audio_api_rs::MediaElement::new(path))?
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        Ok(Self(Arc::new(Mutex::new(inner))))
    }

    #[getter(currentTime)]
    pub(crate) fn current_time(&self) -> f64 {
        self.0.lock().unwrap().current_time()
    }

    #[setter(currentTime)]
    pub(crate) fn set_current_time(&self, value: f64) {
        self.0.lock().unwrap().set_current_time(value);
    }

    #[getter]
    pub(crate) fn r#loop(&self) -> bool {
        self.0.lock().unwrap().loop_()
    }

    #[setter]
    pub(crate) fn set_loop(&self, value: bool) {
        self.0.lock().unwrap().set_loop(value);
    }

    #[getter]
    pub(crate) fn paused(&self) -> bool {
        self.0.lock().unwrap().paused()
    }

    #[getter(playbackRate)]
    pub(crate) fn playback_rate(&self) -> f64 {
        self.0.lock().unwrap().playback_rate()
    }

    #[setter(playbackRate)]
    pub(crate) fn set_playback_rate(&self, value: f64) {
        self.0.lock().unwrap().set_playback_rate(value);
    }

    pub(crate) fn play(&self) {
        self.0.lock().unwrap().play();
    }

    pub(crate) fn pause(&self) {
        self.0.lock().unwrap().pause();
    }
}

#[pymethods]
impl MediaStream {
    #[staticmethod]
    #[pyo3(name = "fromTracks")]
    pub(crate) fn from_tracks(tracks: Vec<PyRef<'_, MediaStreamTrack>>) -> Self {
        Self(web_audio_api_rs::media_streams::MediaStream::from_tracks(
            tracks.into_iter().map(|track| track.0.clone()).collect(),
        ))
    }

    #[staticmethod]
    #[pyo3(name = "fromBufferIterator", signature = (iterable, sampleRate=None, numberOfChannels=None))]
    #[allow(non_snake_case)]
    pub(crate) fn from_buffer_iterator(
        py: Python<'_>,
        iterable: &Bound<'_, PyAny>,
        sampleRate: Option<f32>,
        numberOfChannels: Option<usize>,
    ) -> PyResult<Self> {
        Ok(Self(
            web_audio_api_rs::media_streams::MediaStream::from_tracks(vec![
                MediaStreamTrack::from_buffer_iterator(py, iterable, sampleRate, numberOfChannels)?
                    .0,
            ]),
        ))
    }

    #[pyo3(name = "iterBuffers")]
    pub(crate) fn iter_buffers(&self) -> PyResult<MediaStreamTrackBufferIterator> {
        let track = self
            .0
            .get_tracks()
            .first()
            .cloned()
            .ok_or_else(|| PyRuntimeError::new_err("MediaStream has no tracks"))?;
        Ok(MediaStreamTrackBufferIterator {
            iter: Mutex::new(Box::new(track.iter())),
        })
    }

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
    #[staticmethod]
    #[pyo3(name = "fromBufferIterator", signature = (iterable, sampleRate=None, numberOfChannels=None))]
    #[allow(non_snake_case)]
    pub(crate) fn from_buffer_iterator(
        _py: Python<'_>,
        iterable: &Bound<'_, PyAny>,
        sampleRate: Option<f32>,
        numberOfChannels: Option<usize>,
    ) -> PyResult<Self> {
        let iterator = PyIterator::from_object(iterable)
            .map_err(|_| {
                PyTypeError::new_err(
                    "MediaStreamTrack.fromBufferIterator expects a Python iterable",
                )
            })?
            .into_any()
            .unbind();
        let provider = PythonMediaStreamTrackProvider {
            iterator,
            sample_rate: sampleRate,
            number_of_channels: numberOfChannels,
        };
        let (send, recv) = mpsc::sync_channel(8);
        thread::spawn(move || {
            while let Some(item) = provider.next_item() {
                if send.send(item).is_err() {
                    break;
                }
            }
        });

        Ok(Self(
            web_audio_api_rs::media_streams::MediaStreamTrack::from_iter(
                QueuedMediaStreamTrackProvider {
                    receiver: Mutex::new(recv),
                },
            ),
        ))
    }

    #[pyo3(name = "iterBuffers")]
    pub(crate) fn iter_buffers(&self) -> MediaStreamTrackBufferIterator {
        MediaStreamTrackBufferIterator {
            iter: Mutex::new(Box::new(self.0.iter())),
        }
    }

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
impl MediaStreamTrackBufferIterator {
    pub(crate) fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    pub(crate) fn __next__(&self) -> PyResult<Option<AudioBuffer>> {
        match self.iter.lock().unwrap().next() {
            Some(Ok(buffer)) => Ok(Some(AudioBuffer::owned(buffer))),
            Some(Err(err)) => Err(PyRuntimeError::new_err(err.to_string())),
            None => Ok(None),
        }
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

fn py_to_channel_samples(value: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f32>>> {
    if let Ok(samples) = value.extract::<Vec<f32>>() {
        return Ok(vec![samples]);
    }

    if let Ok(channels) = value.extract::<Vec<Vec<f32>>>() {
        return Ok(channels);
    }

    Err(PyTypeError::new_err(
        "buffer iterator items must be AudioBuffer, list[float], or list[list[float]]",
    ))
}
