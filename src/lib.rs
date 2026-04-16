use pyo3::prelude::*;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use web_audio_api_rs::context::BaseAudioContext;
use web_audio_api_rs::node::{AudioNode as RsAudioNode, AudioScheduledSourceNode as _};

static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

#[pyclass]
struct AudioContext(web_audio_api_rs::context::AudioContext);

#[pymethods]
impl AudioContext {
    #[new]
    fn new() -> Self {
        Self(Default::default())
    }

    fn destination(&self) -> AudioNode {
        destination_node(&self.0)
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

    fn destination(&self) -> AudioNode {
        destination_node(&self.0)
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

fn oscillator_node(ctx: &impl BaseAudioContext) -> (OscillatorNode, AudioNode) {
    let osc = ctx.create_oscillator();
    let node = Arc::new(Mutex::new(osc));
    let audio_node = Arc::clone(&node) as Arc<Mutex<dyn RsAudioNode + Send + 'static>>;
    (OscillatorNode(node), AudioNode(audio_node))
}

#[pyclass]
struct AudioParam(web_audio_api_rs::AudioParam);

#[pymethods]
impl AudioParam {
    #[getter]
    fn value(&self) -> PyResult<f32> {
        Ok(self.0.value())
    }

    #[setter]
    fn set_value(&self, value: f32) -> PyResult<()> {
        self.0.set_value(value);
        Ok(())
    }
}

#[pyclass(extends = AudioNode)]
struct OscillatorNode(Arc<Mutex<web_audio_api_rs::node::OscillatorNode>>);

#[pymethods]
impl OscillatorNode {
    #[new]
    fn new(ctx: &Bound<'_, PyAny>) -> PyResult<(Self, AudioNode)> {
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

    #[pyo3(signature = (when=0.0))]
    fn start(&mut self, when: f64) {
        self.0.lock().unwrap().start_at(when)
    }

    #[pyo3(signature = (when=0.0))]
    fn stop(&mut self, when: f64) {
        self.0.lock().unwrap().stop_at(when)
    }

    fn frequency(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().frequency().clone())
    }

    fn detune(&self) -> AudioParam {
        AudioParam(self.0.lock().unwrap().detune().clone())
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn web_audio_api(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AudioContext>()?;
    m.add_class::<OfflineAudioContext>()?;
    m.add_class::<AudioNode>()?;
    m.add_class::<OscillatorNode>()?;
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
        let (mut osc, osc_node) = oscillator_node(&ctx.0);
        let destination = ctx.destination();

        osc_node.connect(&destination).unwrap();
        osc.frequency().set_value(300.0).unwrap();
        assert_eq!(osc.frequency().value().unwrap(), 300.0);

        osc.start(0.0);
        osc.stop(0.0);
    }

    #[test]
    fn self_connect_does_not_deadlock() {
        Python::initialize();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let ctx = OfflineAudioContext::new(1, 128, 44_100.);
            let (_, node) = oscillator_node(&ctx.0);
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
}
