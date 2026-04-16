use pyo3::prelude::*;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use web_audio_api_rs::context::BaseAudioContext;
use web_audio_api_rs::node::{AudioNode as RsAudioNode, AudioScheduledSourceNode as _};
use web_audio_api_rs::AutomationRate;

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
    #[getter]
    fn automation_rate(&self) -> String {
        automation_rate_to_str(self.0.automation_rate()).to_owned()
    }

    #[setter]
    fn set_automation_rate(&self, value: &str) -> PyResult<()> {
        let value = automation_rate_from_str(value)?;
        catch_web_audio_panic(|| self.0.set_automation_rate(value))
    }

    #[getter]
    fn default_value(&self) -> f32 {
        self.0.default_value()
    }

    #[getter]
    fn min_value(&self) -> f32 {
        self.0.min_value()
    }

    #[getter]
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

    fn set_value_at_time(&self, value: f32, start_time: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.set_value_at_time(value, start_time);
        })
    }

    fn linear_ramp_to_value_at_time(&self, value: f32, end_time: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.linear_ramp_to_value_at_time(value, end_time);
        })
    }

    fn exponential_ramp_to_value_at_time(&self, value: f32, end_time: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.exponential_ramp_to_value_at_time(value, end_time);
        })
    }

    fn set_target_at_time(&self, value: f32, start_time: f64, time_constant: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.set_target_at_time(value, start_time, time_constant);
        })
    }

    fn cancel_scheduled_values(&self, cancel_time: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.cancel_scheduled_values(cancel_time);
        })
    }

    fn cancel_and_hold_at_time(&self, cancel_time: f64) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0.cancel_and_hold_at_time(cancel_time);
        })
    }

    fn set_value_curve_at_time(
        &self,
        values: Vec<f32>,
        start_time: f64,
        duration: f64,
    ) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0
                .set_value_curve_at_time(&values, start_time, duration);
        })
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

    #[getter]
    fn type_(&self) -> PyResult<String> {
        Ok(oscillator_type_to_str(self.0.lock().unwrap().type_()).to_owned())
    }

    #[setter]
    fn set_type_(&mut self, value: &str) -> PyResult<()> {
        self.set_type(value)
    }

    fn set_type(&mut self, value: &str) -> PyResult<()> {
        let value = oscillator_type_from_str(value)?;
        catch_web_audio_panic(|| self.0.lock().unwrap().set_type(value))
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
