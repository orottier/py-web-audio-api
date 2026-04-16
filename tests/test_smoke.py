import unittest

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
    def test_audio_node_idl_surface_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        gain = web_audio_api.GainNode(ctx)

        self.assertIsInstance(gain.context, web_audio_api.BaseAudioContext)
        self.assertEqual(gain.context.sampleRate, 44_100.0)
        self.assertEqual(gain.numberOfInputs, 1)
        self.assertEqual(gain.numberOfOutputs, 1)
        self.assertEqual(gain.channelCount, 2)
        self.assertEqual(gain.channelCountMode, "max")
        self.assertEqual(gain.channelInterpretation, "speakers")

        gain.channelCount = 1
        gain.channelCountMode = "explicit"
        gain.channelInterpretation = "discrete"

        self.assertEqual(gain.channelCount, 1)
        self.assertEqual(gain.channelCountMode, "explicit")
        self.assertEqual(gain.channelInterpretation, "discrete")

    def test_audio_node_connect_and_disconnect_overloads_work(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        src = web_audio_api.ConstantSourceNode(ctx)
        gain = web_audio_api.GainNode(ctx)

        self.assertIs(src.connect(gain), gain)
        src.disconnect(gain)

        self.assertIsNone(src.connect(gain.gain))
        src.disconnect(gain.gain)

        self.assertIs(src.connect(gain, 0, 0), gain)
        src.disconnect(gain, 0, 0)
        src.disconnect()

    def test_base_audio_context_inheritance_and_shared_surface_work(self):
        audio_ctx = web_audio_api.AudioContext({"sinkId": "none"})
        offline_ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)

        self.assertIsInstance(audio_ctx, web_audio_api.BaseAudioContext)
        self.assertIsInstance(offline_ctx, web_audio_api.BaseAudioContext)
        self.assertIsInstance(audio_ctx, web_audio_api.AudioContext)
        self.assertIsInstance(offline_ctx, web_audio_api.OfflineAudioContext)

        self.assertGreater(audio_ctx.sampleRate, 0.0)
        self.assertEqual(offline_ctx.sampleRate, 44_100.0)
        self.assertGreaterEqual(audio_ctx.currentTime, 0.0)
        self.assertEqual(offline_ctx.currentTime, 0.0)

        realtime_buffer = audio_ctx.createBuffer(1, 32, 8_000.0)
        self.assertEqual(realtime_buffer.numberOfChannels, 1)
        self.assertEqual(realtime_buffer.length, 32)
        self.assertEqual(realtime_buffer.sampleRate, 8_000.0)

        buffer = offline_ctx.createBuffer(1, 64, 8_000.0)
        self.assertEqual(buffer.numberOfChannels, 1)
        self.assertEqual(buffer.length, 64)
        self.assertEqual(buffer.sampleRate, 8_000.0)

        self.assertIsInstance(audio_ctx.createGain(), web_audio_api.GainNode)
        self.assertIsInstance(offline_ctx.createGain(), web_audio_api.GainNode)
        self.assertIsInstance(audio_ctx.destination, web_audio_api.AudioDestinationNode)
        self.assertIsInstance(offline_ctx.destination, web_audio_api.AudioDestinationNode)
        self.assertIsInstance(audio_ctx.listener, web_audio_api.AudioListener)
        self.assertIsInstance(offline_ctx.listener, web_audio_api.AudioListener)

    def test_audio_destination_and_listener_work(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)

        self.assertEqual(ctx.destination.maxChannelCount, 2)
        self.assertIsInstance(ctx.destination, web_audio_api.AudioNode)

        listener = ctx.listener
        self.assertEqual(listener.positionX.value, 0.0)
        self.assertEqual(listener.forwardX.value, 0.0)
        self.assertEqual(listener.forwardY.value, 0.0)
        self.assertEqual(listener.forwardZ.value, -1.0)
        self.assertEqual(listener.upY.value, 1.0)

        listener.setPosition(1.0, 2.0, 3.0)
        listener.setOrientation(0.0, 0.0, -1.0, 0.0, 1.0, 0.0)

        self.assertEqual(listener.positionX.value, 1.0)
        self.assertEqual(listener.positionY.value, 2.0)
        self.assertEqual(listener.positionZ.value, 3.0)

    def test_analyser_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        analyser = web_audio_api.AnalyserNode(ctx, {"fftSize": 64})

        self.assertIsInstance(analyser, web_audio_api.AudioNode)
        self.assertEqual(analyser.fftSize, 64)
        self.assertEqual(analyser.frequencyBinCount, 32)
        self.assertEqual(analyser.minDecibels, -100.0)
        self.assertEqual(analyser.maxDecibels, -30.0)
        self.assertEqual(analyser.smoothingTimeConstant, 0.8)

        analyser.fftSize = 128
        analyser.minDecibels = -90.0
        analyser.maxDecibels = -20.0
        analyser.smoothingTimeConstant = 0.5

        self.assertEqual(analyser.fftSize, 128)
        self.assertEqual(analyser.frequencyBinCount, 64)
        self.assertEqual(analyser.minDecibels, -90.0)
        self.assertEqual(analyser.maxDecibels, -20.0)
        self.assertEqual(analyser.smoothingTimeConstant, 0.5)
        self.assertEqual(len(analyser.getFloatFrequencyData([0.0] * 64)), 64)
        self.assertEqual(len(analyser.getByteFrequencyData([0] * 64)), 64)
        self.assertEqual(len(analyser.getFloatTimeDomainData([0.0] * 128)), 128)
        self.assertEqual(len(analyser.getByteTimeDomainData([0] * 128)), 128)

    def test_create_analyser_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        analyser = ctx.createAnalyser()

        self.assertEqual(analyser.fftSize, 2048)

    def test_convolver_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        impulse = web_audio_api.AudioBuffer(
            {"numberOfChannels": 1, "length": 8, "sampleRate": 44_100.0}
        )
        convolver = web_audio_api.ConvolverNode(ctx, {"buffer": impulse, "normalize": False})

        self.assertIsInstance(convolver, web_audio_api.AudioNode)
        self.assertEqual(convolver.buffer.length, 8)
        self.assertFalse(convolver.normalize)

        convolver.normalize = True
        self.assertTrue(convolver.normalize)

    def test_create_convolver_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        convolver = ctx.createConvolver()

        self.assertIsNone(convolver.buffer)
        self.assertTrue(convolver.normalize)

    def test_dynamics_compressor_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        compressor = web_audio_api.DynamicsCompressorNode(ctx, {"threshold": -18.0})

        self.assertIsInstance(compressor, web_audio_api.AudioNode)
        self.assertEqual(compressor.threshold.value, -18.0)
        self.assertEqual(compressor.knee.value, 30.0)
        self.assertEqual(compressor.ratio.value, 12.0)
        self.assertAlmostEqual(compressor.attack.value, 0.003)
        self.assertEqual(compressor.release.value, 0.25)

    def test_create_dynamics_compressor_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        compressor = ctx.createDynamicsCompressor()

        self.assertEqual(compressor.threshold.value, -24.0)

    def test_base_audio_context_is_not_constructible(self):
        with self.assertRaises(TypeError):
            web_audio_api.BaseAudioContext()

    def test_audio_context_does_not_expose_start_rendering(self):
        ctx = web_audio_api.AudioContext({"sinkId": "none"})
        self.assertFalse(hasattr(ctx, "startRendering"))

    def test_audio_context_options_are_accepted(self):
        for constructor in (lambda: web_audio_api.AudioContext(), lambda: web_audio_api.AudioContext(None)):
            try:
                ctx = constructor()
            except RuntimeError as exc:
                self.assertNotIsInstance(exc, TypeError)
            else:
                self.assertGreater(ctx.sampleRate, 0.0)

        ctx = web_audio_api.AudioContext(
            {
                "sinkId": "none",
                "sampleRate": 8_000.0,
                "latencyHint": "playback",
                "renderSizeHint": "default",
            }
        )

        self.assertEqual(ctx.sampleRate, 8_000.0)

        custom_latency_ctx = web_audio_api.AudioContext(
            {"sinkId": "none", "latencyHint": 0.25}
        )
        self.assertGreater(custom_latency_ctx.sampleRate, 0.0)

    def test_offline_oscillator_graph_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = ctx.createOscillator()

        self.assertIsInstance(osc, web_audio_api.AudioScheduledSourceNode)
        self.assertIsInstance(osc, web_audio_api.AudioNode)

        osc.connect(ctx.destination)
        osc.frequency.value = 300.0

        self.assertEqual(osc.frequency.value, 300.0)
        self.assertEqual(osc.type, "sine")

        osc.type = "square"
        self.assertEqual(osc.type, "square")

        osc.start()
        osc.stop()

    def test_direct_node_constructors_accept_omitted_optional_options(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)

        gain = web_audio_api.GainNode(ctx)
        gain_with_none = web_audio_api.GainNode(ctx, None)
        osc = web_audio_api.OscillatorNode(ctx)
        osc_with_none = web_audio_api.OscillatorNode(ctx, None)
        configured_osc = web_audio_api.OscillatorNode(
            ctx, {"type": "square", "frequency": 220.0, "detune": 50.0}
        )

        self.assertEqual(gain.gain.value, 1.0)
        self.assertEqual(gain_with_none.gain.value, 1.0)
        self.assertEqual(osc.type, "sine")
        self.assertEqual(osc_with_none.type, "sine")
        self.assertEqual(configured_osc.type, "square")
        self.assertEqual(configured_osc.frequency.value, 220.0)
        self.assertEqual(configured_osc.detune.value, 50.0)

    def test_audio_node_options_are_accepted_in_inherited_option_dicts(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)

        gain = web_audio_api.GainNode(
            ctx,
            {
                "gain": 0.5,
                "channelCount": 1,
                "channelCountMode": "explicit",
                "channelInterpretation": "discrete",
            },
        )
        analyser = web_audio_api.AnalyserNode(
            ctx,
            {
                "fftSize": 64,
                "channelCount": 1,
                "channelCountMode": "explicit",
                "channelInterpretation": "discrete",
            },
        )
        biquad = web_audio_api.BiquadFilterNode(
            ctx,
            {
                "type": "highpass",
                "channelCount": 1,
                "channelCountMode": "explicit",
                "channelInterpretation": "discrete",
            },
        )

        for node in (gain, analyser, biquad):
            self.assertEqual(node.channelCount, 1)
            self.assertEqual(node.channelCountMode, "explicit")
            self.assertEqual(node.channelInterpretation, "discrete")

        self.assertEqual(gain.gain.value, 0.5)
        self.assertEqual(analyser.fftSize, 64)
        self.assertEqual(biquad.type, "highpass")

    def test_invalid_shared_audio_node_option_value_raises(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)

        with self.assertRaisesRegex(ValueError, "expected 'max', 'clamped-max', or 'explicit'"):
            web_audio_api.GainNode(ctx, {"channelCountMode": "sideways"})

    def test_iir_filter_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        filt = web_audio_api.IIRFilterNode(
            ctx,
            {
                "feedforward": [1.0, 0.0],
                "feedback": [1.0, 0.0],
                "channelCount": 1,
                "channelCountMode": "explicit",
            },
        )

        self.assertIsInstance(filt, web_audio_api.AudioNode)
        self.assertEqual(filt.channelCount, 1)
        self.assertEqual(filt.channelCountMode, "explicit")

        mag, phase = filt.getFrequencyResponse([10.0, 100.0, 1_000.0])
        self.assertEqual(len(mag), 3)
        self.assertEqual(len(phase), 3)

    def test_create_iir_filter_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        filt = ctx.createIIRFilter([1.0, 0.0], [1.0, 0.0])

        mag, phase = filt.getFrequencyResponse([50.0, 500.0])
        self.assertEqual(len(mag), 2)
        self.assertEqual(len(phase), 2)

    def test_audio_param_methods_work(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)
        frequency = osc.frequency

        self.assertEqual(frequency.automationRate, "a-rate")
        self.assertEqual(frequency.defaultValue, 440.0)
        self.assertLess(frequency.minValue, frequency.maxValue)

        frequency.automationRate = "k-rate"
        self.assertEqual(frequency.automationRate, "k-rate")

        frequency.value = 220.0
        self.assertEqual(frequency.value, 220.0)

        self.assertIs(frequency.setValueAtTime(330.0, 0.0), frequency)
        self.assertIs(frequency.linearRampToValueAtTime(440.0, 0.1), frequency)
        self.assertIs(frequency.exponentialRampToValueAtTime(660.0, 0.2), frequency)
        self.assertIs(frequency.setTargetAtTime(550.0, 0.3, 0.1), frequency)
        self.assertIs(frequency.cancelScheduledValues(0.4), frequency)
        self.assertIs(frequency.cancelAndHoldAtTime(0.5), frequency)
        self.assertIs(
            frequency.setValueCurveAtTime([220.0, 330.0, 440.0], 0.6, 0.2),
            frequency,
        )

    def test_audio_param_has_idl_shaped_surface(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        gain = web_audio_api.GainNode(ctx)
        param = gain.gain

        self.assertIsInstance(param, web_audio_api.AudioParam)
        self.assertEqual(param.automationRate, "a-rate")
        self.assertEqual(param.defaultValue, 1.0)
        self.assertLess(param.minValue, -1e30)
        self.assertGreater(param.maxValue, 1e30)
        self.assertEqual(param.value, 1.0)

        with self.assertRaises(TypeError):
            web_audio_api.AudioParam()

    def test_audio_buffer_has_idl_shaped_copy_surface(self):
        buffer = web_audio_api.AudioBuffer(
            {"numberOfChannels": 2, "length": 8, "sampleRate": 8_000.0}
        )

        buffer.copyToChannel([1.0, 2.0, 3.0, 4.0], 0, 2)
        buffer.copyToChannel([0.25, 0.5], 1)

        self.assertEqual(buffer.getChannelData(0), [0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0])
        self.assertEqual(buffer.getChannelData(1), [0.25, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        self.assertEqual(buffer.copyFromChannel([9.0, 9.0, 9.0], 0), [0.0, 0.0, 1.0])
        self.assertEqual(buffer.copyFromChannel([9.0, 9.0, 9.0], 0, 3), [2.0, 3.0, 4.0])

    def test_self_connect_reports_rust_error(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)

        with self.assertRaisesRegex(RuntimeError, "input port 0 is out of bounds"):
            osc.connect(osc)

    def test_constant_source_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        src = web_audio_api.ConstantSourceNode(ctx, {"offset": 2.0})

        self.assertIsInstance(src, web_audio_api.AudioScheduledSourceNode)
        self.assertIsInstance(src, web_audio_api.AudioNode)

        src.connect(ctx.destination)
        self.assertEqual(src.offset.value, 2.0)

        src.offset.value = 3.0
        self.assertEqual(src.offset.value, 3.0)

        src.start()
        src.stop()

    def test_create_constant_source_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        src = ctx.createConstantSource()

        self.assertEqual(src.offset.value, 1.0)

    def test_audio_scheduled_source_node_onended_property_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)
        marker = object()

        self.assertIsNone(osc.onended)
        osc.onended = marker
        self.assertIs(osc.onended, marker)
        osc.onended = None
        self.assertIsNone(osc.onended)

    def test_constant_source_renders_scheduled_samples_offline(self):
        ctx = web_audio_api.OfflineAudioContext(1, 2000, 2000.0)
        src = web_audio_api.ConstantSourceNode(ctx, {"offset": 0.25})

        src.connect(ctx.destination)
        src.start(0.25)
        src.stop(0.75)

        rendered = ctx.startRendering()
        data = rendered.getChannelData(0)

        self.assertEqual(rendered.numberOfChannels, 1)
        self.assertEqual(rendered.length, 2000)
        self.assertEqual(rendered.sampleRate, 2000.0)
        self.assertEqual(rendered.duration, 1.0)
        self.assertEqual(len(data), 2000)
        self.assertTrue(all(sample == 0.0 for sample in data[:500]))
        self.assertTrue(all(sample == 0.25 for sample in data[500:1501]))
        self.assertTrue(all(sample == 0.0 for sample in data[1501:]))

    def test_audio_buffer_source_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        buffer = web_audio_api.AudioBuffer(
            {"numberOfChannels": 1, "length": 128, "sampleRate": 44_100.0}
        )
        src = web_audio_api.AudioBufferSourceNode(ctx, {"buffer": buffer})

        self.assertIsInstance(src, web_audio_api.AudioScheduledSourceNode)
        self.assertIsInstance(src, web_audio_api.AudioNode)
        self.assertEqual(src.buffer.length, 128)
        self.assertEqual(src.playbackRate.value, 1.0)
        self.assertEqual(src.detune.value, 0.0)
        self.assertFalse(src.loop)

        src.loop = True
        src.loopStart = 0.25
        src.loopEnd = 0.5
        self.assertTrue(src.loop)
        self.assertEqual(src.loopStart, 0.25)
        self.assertEqual(src.loopEnd, 0.5)

        src.connect(ctx.destination)
        src.start()
        src.stop()

    def test_create_buffer_source_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        src = ctx.createBufferSource()

        self.assertIsNone(src.buffer)

    def test_audio_buffer_source_renders_samples_offline(self):
        ctx = web_audio_api.OfflineAudioContext(1, 2000, 2000.0)
        buffer = web_audio_api.AudioBuffer(
            {"numberOfChannels": 1, "length": 2000, "sampleRate": 2000.0}
        )
        buffer.copyToChannel([0.125] * 1000 + [0.25] * 1000, 0)
        src = web_audio_api.AudioBufferSourceNode(ctx)

        src.buffer = buffer
        src.connect(ctx.destination)
        src.start()

        data = ctx.startRendering().getChannelData(0)
        self.assertTrue(all(sample == 0.125 for sample in data[:1000]))
        self.assertTrue(all(sample == 0.25 for sample in data[1000:]))

    def test_gain_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        gain = web_audio_api.GainNode(ctx, {"gain": 0.5})

        self.assertIsInstance(gain, web_audio_api.AudioNode)
        self.assertEqual(gain.gain.value, 0.5)

        gain.gain.value = 0.25
        self.assertEqual(gain.gain.value, 0.25)

    def test_create_gain_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        gain = ctx.createGain()

        self.assertEqual(gain.gain.value, 1.0)

    def test_gain_node_renders_samples_offline(self):
        ctx = web_audio_api.OfflineAudioContext(1, 2000, 2000.0)
        buffer = web_audio_api.AudioBuffer(
            {"numberOfChannels": 1, "length": 2000, "sampleRate": 2000.0}
        )
        buffer.copyToChannel([0.5] * 2000, 0)
        src = web_audio_api.AudioBufferSourceNode(ctx, {"buffer": buffer})
        gain = web_audio_api.GainNode(ctx, {"gain": 0.25})

        src.connect(gain)
        gain.connect(ctx.destination)
        src.start()

        data = ctx.startRendering().getChannelData(0)
        self.assertTrue(all(sample == 0.125 for sample in data))

    def test_delay_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        delay = web_audio_api.DelayNode(ctx, {"delayTime": 0.25, "maxDelayTime": 1.0})

        self.assertIsInstance(delay, web_audio_api.AudioNode)
        self.assertEqual(delay.delayTime.value, 0.25)
        self.assertEqual(delay.delayTime.defaultValue, 0.0)

        delay.delayTime.value = 0.5
        self.assertEqual(delay.delayTime.value, 0.5)

    def test_create_delay_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        delay = ctx.createDelay(2.0)

        self.assertEqual(delay.delayTime.value, 0.0)
        self.assertEqual(delay.delayTime.maxValue, 2.0)

    def test_stereo_panner_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        panner = web_audio_api.StereoPannerNode(ctx, {"pan": -0.5})

        self.assertIsInstance(panner, web_audio_api.AudioNode)
        self.assertEqual(panner.pan.value, -0.5)
        self.assertEqual(panner.pan.defaultValue, 0.0)
        self.assertEqual(panner.pan.minValue, -1.0)
        self.assertEqual(panner.pan.maxValue, 1.0)

        panner.pan.value = 0.5
        self.assertEqual(panner.pan.value, 0.5)

    def test_create_stereo_panner_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        panner = ctx.createStereoPanner()

        self.assertEqual(panner.pan.value, 0.0)

    def test_channel_merger_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        merger = web_audio_api.ChannelMergerNode(ctx, {"numberOfInputs": 2})

        self.assertIsInstance(merger, web_audio_api.AudioNode)
        merger.connect(ctx.destination)

    def test_create_channel_merger_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        merger = ctx.createChannelMerger(2)

        self.assertIsInstance(merger, web_audio_api.ChannelMergerNode)

    def test_channel_splitter_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        splitter = web_audio_api.ChannelSplitterNode(ctx, {"numberOfOutputs": 2})

        self.assertIsInstance(splitter, web_audio_api.AudioNode)
        splitter.connect(ctx.destination)

    def test_create_channel_splitter_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        splitter = ctx.createChannelSplitter(2)

        self.assertIsInstance(splitter, web_audio_api.ChannelSplitterNode)

    def test_biquad_filter_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        biquad = web_audio_api.BiquadFilterNode(ctx, {"type": "highpass", "Q": 2.0})

        self.assertIsInstance(biquad, web_audio_api.AudioNode)
        self.assertEqual(biquad.type, "highpass")
        self.assertEqual(biquad.frequency.value, 350.0)
        self.assertEqual(biquad.detune.value, 0.0)
        self.assertEqual(biquad.Q.value, 2.0)
        self.assertEqual(biquad.gain.value, 0.0)

        biquad.type = "notch"
        self.assertEqual(biquad.type, "notch")

        mag_response, phase_response = biquad.getFrequencyResponse([100.0, 1000.0])
        self.assertEqual(len(mag_response), 2)
        self.assertEqual(len(phase_response), 2)

    def test_create_biquad_filter_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        biquad = ctx.createBiquadFilter()

        self.assertEqual(biquad.type, "lowpass")


if __name__ == "__main__":
    unittest.main()
