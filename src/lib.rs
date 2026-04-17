use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::PyClass;
use std::future::Future;
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

mod context;
mod core;
mod data;
mod nodes;

use context::*;
use core::*;
use data::*;
use nodes::*;

fn into_py_future<'py, F, T>(py: Python<'py>, fut: F) -> PyResult<Bound<'py, PyAny>>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: for<'py2> IntoPyObject<'py2> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py(py, fut)
}

#[pyfunction(name = "getUserMediaSync", signature = (constraints=None))]
fn get_user_media_sync(constraints: Option<&Bound<'_, PyAny>>) -> PyResult<MediaStream> {
    let constraints = media_stream_constraints(constraints)?;
    let stream = catch_web_audio_panic_result(|| {
        web_audio_api_rs::media_devices::get_user_media_sync(constraints)
    })?;
    Ok(MediaStream(stream))
}

#[pyfunction(name = "getUserMedia", signature = (constraints=None))]
fn get_user_media<'py>(
    py: Python<'py>,
    constraints: Option<&Bound<'_, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let constraints = media_stream_constraints(constraints)?;
    into_py_future(py, async move {
        let stream = tokio::task::spawn_blocking(move || {
            catch_web_audio_panic_result(|| {
                web_audio_api_rs::media_devices::get_user_media_sync(constraints)
            })
        })
        .await
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))??;
        Ok(MediaStream(stream))
    })
}

#[pyfunction(name = "enumerateDevicesSync")]
fn enumerate_devices_sync() -> PyResult<Vec<MediaDeviceInfo>> {
    let devices =
        catch_web_audio_panic_result(web_audio_api_rs::media_devices::enumerate_devices_sync)?;
    Ok(devices.into_iter().map(MediaDeviceInfo::from_rs).collect())
}

#[pyfunction(name = "enumerateDevices")]
fn enumerate_devices<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    into_py_future(py, async move {
        let devices = tokio::task::spawn_blocking(|| {
            catch_web_audio_panic_result(web_audio_api_rs::media_devices::enumerate_devices_sync)
                .map(|devices| {
                    devices
                        .into_iter()
                        .map(MediaDeviceInfo::from_rs)
                        .collect::<Vec<_>>()
                })
        })
        .await
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))??;
        Ok(devices)
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn web_audio_api(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseAudioContext>()?;
    m.add_class::<AudioContext>()?;
    m.add_class::<OfflineAudioContext>()?;
    m.add_class::<OfflineAudioCompletionEvent>()?;
    m.add_class::<AudioProcessingEvent>()?;
    m.add_class::<AudioBuffer>()?;
    m.add_class::<MediaDeviceInfo>()?;
    m.add_class::<MediaStream>()?;
    m.add_class::<MediaStreamTrack>()?;
    m.add_class::<PeriodicWave>()?;
    m.add_class::<AudioListener>()?;
    m.add_class::<Event>()?;
    m.add_class::<EventTarget>()?;
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
    m.add_class::<IIRFilterNode>()?;
    m.add_class::<WaveShaperNode>()?;
    m.add_class::<PannerNode>()?;
    m.add_class::<ScriptProcessorNode>()?;
    m.add_class::<MediaStreamAudioSourceNode>()?;
    m.add_class::<MediaStreamTrackAudioSourceNode>()?;
    m.add_class::<OscillatorNode>()?;
    m.add_class::<ConstantSourceNode>()?;
    m.add_class::<AudioParam>()?;
    m.add_function(wrap_pyfunction!(get_user_media, m)?)?;
    m.add_function(wrap_pyfunction!(get_user_media_sync, m)?)?;
    m.add_function(wrap_pyfunction!(enumerate_devices, m)?)?;
    m.add_function(wrap_pyfunction!(enumerate_devices_sync, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn silent_audio_context_options() -> web_audio_api_rs::context::AudioContextOptions {
        web_audio_api_rs::context::AudioContextOptions {
            sink_id: "none".into(),
            ..Default::default()
        }
    }

    fn audio_context_parts() -> (AudioContext, BaseAudioContext) {
        let ctx = Arc::new(new_realtime_context(silent_audio_context_options()));
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
        let ctx = Arc::new(web_audio_api_rs::context::OfflineAudioContext::new(
            number_of_channels,
            length,
            sample_rate,
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
        assert_eq!(realtime.create_buffer(1, 16, 8_000.).length().unwrap(), 16);
        assert_eq!(offline.create_buffer(1, 16, 8_000.).length().unwrap(), 16);

        let _ = realtime.destination_inner();
        let _ = offline.destination_inner();
    }

    #[test]
    fn media_stream_audio_source_graph_smoke_test() {
        let (ctx, _) = audio_context_parts();
        let buffer = web_audio_api_rs::AudioBuffer::new(web_audio_api_rs::AudioBufferOptions {
            number_of_channels: 1,
            length: 128,
            sample_rate: 44_100.,
        });
        let track = web_audio_api_rs::media_streams::MediaStreamTrack::from_iter(vec![
            Ok(buffer.clone()),
            Ok(buffer),
        ]);
        let stream = web_audio_api_rs::media_streams::MediaStream::from_tracks(vec![track]);
        let (_src, node) = media_stream_audio_source_node_parts(&ctx.0, &stream);

        assert_eq!(node.number_of_inputs().unwrap(), 0);
        assert_eq!(node.number_of_outputs().unwrap(), 1);
    }

    #[test]
    fn media_stream_track_audio_source_graph_smoke_test() {
        let (ctx, _) = audio_context_parts();
        let buffer = web_audio_api_rs::AudioBuffer::new(web_audio_api_rs::AudioBufferOptions {
            number_of_channels: 1,
            length: 128,
            sample_rate: 44_100.,
        });
        let track = web_audio_api_rs::media_streams::MediaStreamTrack::from_iter(vec![
            Ok(buffer.clone()),
            Ok(buffer),
        ]);
        let (_src, node) = media_stream_track_audio_source_node_parts(&ctx.0, &track);

        assert_eq!(node.number_of_inputs().unwrap(), 0);
        assert_eq!(node.number_of_outputs().unwrap(), 1);
    }

    #[test]
    fn audio_node_shared_surface_works() {
        let (ctx, _) = offline_context_parts(1, 128, 44_100.);
        let (_, gain_node) =
            gain_node_parts(&*ctx.0, web_audio_api_rs::node::GainOptions::default());
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
        let (osc, scheduled, osc_node) = oscillator_node_parts(
            &*ctx.0,
            web_audio_api_rs::node::OscillatorOptions::default(),
        );
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
            let (_, _, node) = oscillator_node_parts(
                &*ctx.0,
                web_audio_api_rs::node::OscillatorOptions::default(),
            );
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
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
            &*ctx.0,
            web_audio_api_rs::node::BiquadFilterOptions::default(),
        );
        let destination = base.destination_audio_node();

        filter_node.connect_node(&destination, 0, 0).unwrap();
        filter.set_type("highpass").unwrap();
        assert_eq!(filter.r#type(), "highpass");
        assert_eq!(filter.frequency().value().unwrap(), 350.);
    }

    #[test]
    fn iir_filter_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (filter, filter_node) = iir_filter_node_parts(
            &*ctx.0,
            web_audio_api_rs::node::IIRFilterOptions {
                audio_node_options: web_audio_api_rs::node::AudioNodeOptions::default(),
                feedforward: vec![1.0, 0.0],
                feedback: vec![1.0, 0.0],
            },
        );
        let destination = base.destination_audio_node();

        filter_node.connect_node(&destination, 0, 0).unwrap();

        let (mag, phase) = filter
            .get_frequency_response(vec![10.0, 100.0, 1_000.0])
            .unwrap();

        assert_eq!(mag.len(), 3);
        assert_eq!(phase.len(), 3);
    }

    #[test]
    fn wave_shaper_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let (mut shaper, shaper_node) = wave_shaper_node_parts(
            &*ctx.0,
            web_audio_api_rs::node::WaveShaperOptions::default(),
        );
        let destination = base.destination_audio_node();

        shaper_node.connect_node(&destination, 0, 0).unwrap();
        shaper.set_curve(Some(vec![-1.0, 0.0, 1.0])).unwrap();
        shaper.set_oversample("2x").unwrap();

        assert_eq!(shaper.curve().unwrap(), [-1.0, 0.0, 1.0]);
        assert_eq!(shaper.oversample(), "2x");
    }

    #[test]
    fn panner_graph_smoke_test() {
        let (ctx, base) = offline_context_parts(2, 128, 44_100.);
        let (mut panner, panner_node) =
            panner_node_parts(&*ctx.0, web_audio_api_rs::node::PannerOptions::default());
        let destination = base.destination_audio_node();

        panner_node.connect_node(&destination, 0, 0).unwrap();
        panner.position_x().set_value(1.0).unwrap();
        panner.set_distance_model("linear").unwrap();
        panner.set_ref_distance(2.0).unwrap();
        panner.set_max_distance(20.0).unwrap();
        panner.set_rolloff_factor(0.5).unwrap();
        panner.set_cone_inner_angle(90.0);
        panner.set_cone_outer_angle(180.0);
        panner.set_cone_outer_gain(0.25).unwrap();

        assert_eq!(panner.position_x().value().unwrap(), 1.0);
        assert_eq!(panner.distance_model(), "linear");
        assert_eq!(panner.ref_distance(), 2.0);
        assert_eq!(panner.max_distance(), 20.0);
        assert_eq!(panner.rolloff_factor(), 0.5);
        assert_eq!(panner.cone_inner_angle(), 90.0);
        assert_eq!(panner.cone_outer_angle(), 180.0);
        assert_eq!(panner.cone_outer_gain(), 0.25);
    }

    #[test]
    fn periodic_wave_smoke_test() {
        let (ctx, base) = offline_context_parts(1, 128, 44_100.);
        let periodic_wave = PeriodicWave(web_audio_api_rs::PeriodicWave::new(
            &*ctx.0,
            web_audio_api_rs::PeriodicWaveOptions {
                real: Some(vec![0.0, 0.0, 0.0]),
                imag: Some(vec![0.0, 1.0, 0.5]),
                disable_normalization: false,
            },
        ));
        let (osc, scheduled, osc_node) = oscillator_node_parts(
            &*ctx.0,
            web_audio_api_rs::node::OscillatorOptions::default(),
        );
        let destination = base.destination_audio_node();

        osc_node.connect_node(&destination, 0, 0).unwrap();
        osc.0
            .lock()
            .unwrap()
            .set_periodic_wave(periodic_wave.0.clone());
        assert_eq!(osc.r#type().unwrap(), "custom");
        scheduled.start(0.0).unwrap();
        scheduled.stop(0.0).unwrap();
    }
}
