import asyncio
import io
import unittest
import wave

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
    @staticmethod
    def run_async(awaitable_factory):
        async def runner():
            awaitable = (
                awaitable_factory() if callable(awaitable_factory) else awaitable_factory
            )
            return await awaitable

        return asyncio.run(runner())

    @staticmethod
    def wav_bytes(samples, sample_rate=8_000):
        buffer = io.BytesIO()
        with wave.open(buffer, "wb") as wav_file:
            wav_file.setnchannels(1)
            wav_file.setsampwidth(2)
            wav_file.setframerate(sample_rate)
            wav_file.writeframes(
                b"".join(
                    int(max(-1.0, min(1.0, sample)) * 32767).to_bytes(
                        2, "little", signed=True
                    )
                    for sample in samples
                )
            )
        return buffer.getvalue()

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

        self.assertIsInstance(audio_ctx, web_audio_api.EventTarget)
        self.assertIsInstance(offline_ctx, web_audio_api.EventTarget)
        self.assertIsInstance(audio_ctx, web_audio_api.BaseAudioContext)
        self.assertIsInstance(offline_ctx, web_audio_api.BaseAudioContext)
        self.assertIsInstance(audio_ctx, web_audio_api.AudioContext)
        self.assertIsInstance(offline_ctx, web_audio_api.OfflineAudioContext)

        self.assertGreater(audio_ctx.sampleRate, 0.0)
        self.assertEqual(offline_ctx.sampleRate, 44_100.0)
        self.assertGreaterEqual(audio_ctx.currentTime, 0.0)
        self.assertEqual(offline_ctx.currentTime, 0.0)
        self.assertIn(audio_ctx.state, ("suspended", "running"))
        self.assertEqual(offline_ctx.state, "suspended")
        self.assertEqual(offline_ctx.length, 128)

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

    def test_offline_audio_context_dict_constructor_works(self):
        ctx = web_audio_api.OfflineAudioContext(
            {"numberOfChannels": 2, "length": 256, "sampleRate": 8_000.0}
        )
        default_channels_ctx = web_audio_api.OfflineAudioContext(
            {"length": 128, "sampleRate": 4_000.0, "renderSizeHint": "default"}
        )

        self.assertEqual(ctx.sampleRate, 8_000.0)
        self.assertEqual(ctx.length, 256)
        self.assertEqual(ctx.destination.maxChannelCount, 2)
        self.assertEqual(default_channels_ctx.length, 128)
        self.assertEqual(default_channels_ctx.sampleRate, 4_000.0)
        self.assertEqual(default_channels_ctx.destination.maxChannelCount, 1)

    def test_base_audio_context_onstatechange_property_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        marker = object()

        self.assertIsNone(ctx.onstatechange)
        ctx.onstatechange = marker
        self.assertIs(ctx.onstatechange, marker)
        ctx.onstatechange = None
        self.assertIsNone(ctx.onstatechange)

    def test_base_audio_context_onstatechange_callback_fires(self):
        ctx = web_audio_api.OfflineAudioContext(1, 512, 44_100.0)
        calls = []

        def onstatechange(event):
            calls.append(event)

        ctx.onstatechange = onstatechange
        self.run_async(lambda: ctx.startRendering())

        self.assertGreaterEqual(len(calls), 1)
        self.assertTrue(all(isinstance(event, web_audio_api.Event) for event in calls))
        self.assertTrue(all(event.type == "statechange" for event in calls))
        self.assertTrue(all(event.target is ctx for event in calls))
        self.assertTrue(all(event.currentTarget is ctx for event in calls))
        self.assertEqual(ctx.state, "closed")

    def test_offline_audio_context_oncomplete_property_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        marker = object()

        self.assertIsNone(ctx.oncomplete)
        ctx.oncomplete = marker
        self.assertIs(ctx.oncomplete, marker)
        ctx.oncomplete = None
        self.assertIsNone(ctx.oncomplete)

    def test_offline_audio_context_oncomplete_callback_fires(self):
        ctx = web_audio_api.OfflineAudioContext(1, 256, 8_000.0)
        calls = []

        def oncomplete(event):
            calls.append(event)

        ctx.oncomplete = oncomplete
        rendered = self.run_async(lambda: ctx.startRendering())

        self.assertEqual(len(calls), 1)
        event = calls[0]
        self.assertIsInstance(event, web_audio_api.OfflineAudioCompletionEvent)
        self.assertEqual(event.type, "complete")
        self.assertIs(event.target, ctx)
        self.assertIs(event.currentTarget, ctx)
        self.assertIsInstance(event.renderedBuffer, web_audio_api.AudioBuffer)
        self.assertEqual(event.renderedBuffer.length, rendered.length)
        self.assertEqual(event.renderedBuffer.sampleRate, rendered.sampleRate)

    def test_base_audio_context_manual_dispatch_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        calls = []

        def listener(event):
            calls.append(event.type)

        ctx.addEventListener("statechange", listener)
        self.assertTrue(ctx.dispatchEvent(web_audio_api.Event("statechange")))
        ctx.removeEventListener("statechange", listener)
        self.assertEqual(calls, ["statechange"])

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
                "sinkId": {"type": "none"},
                "sampleRate": 8_000.0,
                "latencyHint": "playback",
                "renderSizeHint": "default",
            }
        )

        self.assertEqual(ctx.sampleRate, 8_000.0)
        self.assertEqual(ctx.sinkId, "none")

        custom_latency_ctx = web_audio_api.AudioContext(
            {"sinkId": "none", "latencyHint": 0.25}
        )
        self.assertGreater(custom_latency_ctx.sampleRate, 0.0)

    def test_audio_context_onsinkchange_property_works(self):
        ctx = web_audio_api.AudioContext({"sinkId": "none"})
        marker = object()

        self.assertIsNone(ctx.onsinkchange)
        self.assertEqual(ctx.baseLatency, 0.0)
        self.assertGreaterEqual(ctx.outputLatency, 0.0)
        self.assertEqual(ctx.sinkId, "none")
        ctx.onsinkchange = marker
        self.assertIs(ctx.onsinkchange, marker)
        ctx.onsinkchange = None
        self.assertIsNone(ctx.onsinkchange)

    def test_audio_context_onsinkchange_manual_dispatch_works(self):
        ctx = web_audio_api.AudioContext({"sinkId": "none"})
        calls = []

        def onsinkchange(event):
            calls.append(event)

        ctx.onsinkchange = onsinkchange
        self.assertTrue(ctx.dispatchEvent(web_audio_api.Event("sinkchange")))

        self.assertEqual(len(calls), 1)
        self.assertIsInstance(calls[0], web_audio_api.Event)
        self.assertEqual(calls[0].type, "sinkchange")
        self.assertIs(calls[0].target, ctx)
        self.assertIs(calls[0].currentTarget, ctx)

    def test_audio_context_async_state_methods_work(self):
        ctx = web_audio_api.AudioContext({"sinkId": "none"})

        self.run_async(lambda: ctx.resume())
        self.assertEqual(ctx.state, "running")
        self.run_async(lambda: ctx.suspend())
        self.assertEqual(ctx.state, "suspended")
        self.run_async(lambda: ctx.close())
        self.assertEqual(ctx.state, "closed")

    def test_create_script_processor_exists_on_contexts(self):
        realtime = web_audio_api.AudioContext({"sinkId": "none"})
        offline = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)

        realtime_node = realtime.createScriptProcessor(256, 0, 1)
        offline_node = offline.createScriptProcessor(256, 0, 1)

        self.assertIsInstance(realtime_node, web_audio_api.ScriptProcessorNode)
        self.assertIsInstance(offline_node, web_audio_api.ScriptProcessorNode)
        self.assertEqual(realtime_node.bufferSize, 256)
        self.assertEqual(offline_node.bufferSize, 256)

    def test_media_stream_is_not_constructible(self):
        with self.assertRaises(TypeError):
            web_audio_api.MediaStream()

    def test_get_user_media_sync_entrypoint_is_wired(self):
        ctx = web_audio_api.AudioContext({"sinkId": "none"})

        try:
            stream = web_audio_api.getUserMediaSync()
        except RuntimeError as exc:
            self.assertNotIsInstance(exc, TypeError)
        else:
            self.assertIsInstance(stream, web_audio_api.MediaStream)
            node = ctx.createMediaStreamSource(stream)
            self.assertIsInstance(node, web_audio_api.MediaStreamAudioSourceNode)
            self.assertIsInstance(node, web_audio_api.AudioNode)
            stream.close()

    def test_create_script_processor_passes_zero_buffer_size_through(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)

        with self.assertRaises(RuntimeError):
            ctx.createScriptProcessor()

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

    def test_wave_shaper_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        shaper = web_audio_api.WaveShaperNode(
            ctx,
            {
                "curve": [-1.0, 0.0, 1.0],
                "oversample": "2x",
                "channelCount": 1,
                "channelCountMode": "explicit",
            },
        )

        self.assertIsInstance(shaper, web_audio_api.AudioNode)
        self.assertEqual(shaper.curve, [-1.0, 0.0, 1.0])
        self.assertEqual(shaper.oversample, "2x")
        self.assertEqual(shaper.channelCount, 1)
        self.assertEqual(shaper.channelCountMode, "explicit")

    def test_create_wave_shaper_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        shaper = ctx.createWaveShaper()

        self.assertIsNone(shaper.curve)
        shaper.curve = [-0.5, 0.0, 0.5]
        shaper.oversample = "4x"
        self.assertEqual(shaper.curve, [-0.5, 0.0, 0.5])
        self.assertEqual(shaper.oversample, "4x")

    def test_panner_node_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        panner = web_audio_api.PannerNode(
            ctx,
            {
                "panningModel": "equalpower",
                "distanceModel": "linear",
                "positionX": 1.0,
                "positionY": 2.0,
                "positionZ": 3.0,
                "orientationX": 0.0,
                "orientationY": 1.0,
                "orientationZ": 0.0,
                "refDistance": 2.0,
                "maxDistance": 20.0,
                "rolloffFactor": 0.5,
                "coneInnerAngle": 90.0,
                "coneOuterAngle": 180.0,
                "coneOuterGain": 0.25,
                "channelCount": 2,
                "channelCountMode": "clamped-max",
            },
        )

        self.assertIsInstance(panner, web_audio_api.AudioNode)
        self.assertEqual(panner.panningModel, "equalpower")
        self.assertEqual(panner.distanceModel, "linear")
        self.assertEqual(panner.positionX.value, 1.0)
        self.assertEqual(panner.positionY.value, 2.0)
        self.assertEqual(panner.positionZ.value, 3.0)
        self.assertEqual(panner.orientationX.value, 0.0)
        self.assertEqual(panner.orientationY.value, 1.0)
        self.assertEqual(panner.orientationZ.value, 0.0)
        self.assertEqual(panner.refDistance, 2.0)
        self.assertEqual(panner.maxDistance, 20.0)
        self.assertEqual(panner.rolloffFactor, 0.5)
        self.assertEqual(panner.coneInnerAngle, 90.0)
        self.assertEqual(panner.coneOuterAngle, 180.0)
        self.assertEqual(panner.coneOuterGain, 0.25)
        self.assertEqual(panner.channelCountMode, "clamped-max")

    def test_create_panner_works(self):
        ctx = web_audio_api.OfflineAudioContext(2, 128, 44_100.0)
        panner = ctx.createPanner()

        self.assertEqual(panner.panningModel, "equalpower")
        self.assertEqual(panner.distanceModel, "inverse")
        panner.refDistance = 3.0
        self.assertEqual(panner.refDistance, 3.0)

    def test_periodic_wave_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        wave = web_audio_api.PeriodicWave(
            ctx,
            {
                "real": [0.0, 0.0, 0.0],
                "imag": [0.0, 1.0, 0.5],
                "disableNormalization": True,
            },
        )
        osc = web_audio_api.OscillatorNode(ctx, {"periodicWave": wave})

        self.assertIsInstance(wave, web_audio_api.PeriodicWave)
        self.assertEqual(osc.type, "custom")

    def test_create_periodic_wave_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        wave = ctx.createPeriodicWave(
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.5],
            {"disableNormalization": False},
        )
        osc = ctx.createOscillator()
        osc.setPeriodicWave(wave)

        self.assertIsInstance(wave, web_audio_api.PeriodicWave)
        self.assertEqual(osc.type, "custom")

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

        self.assertIsInstance(osc, web_audio_api.EventTarget)
        self.assertIsNone(osc.onended)
        osc.onended = marker
        self.assertIs(osc.onended, marker)
        osc.onended = None
        self.assertIsNone(osc.onended)

    def test_audio_scheduled_source_node_onended_callback_fires(self):
        ctx = web_audio_api.OfflineAudioContext(1, 2000, 2000.0)
        src = web_audio_api.ConstantSourceNode(ctx)
        calls = []

        def onended(event):
            calls.append(event)

        src.onended = onended
        src.connect(ctx.destination)
        src.start(0.0)
        src.stop(0.25)

        self.run_async(lambda: ctx.startRendering())

        self.assertEqual(len(calls), 1)
        self.assertIsInstance(calls[0], web_audio_api.Event)
        self.assertEqual(calls[0].type, "ended")
        self.assertIs(calls[0].target, src)
        self.assertIs(calls[0].currentTarget, src)

    def test_event_target_manual_dispatch_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)
        calls = []

        def listener(event):
            calls.append(event.type)

        osc.addEventListener("ended", listener)
        self.assertTrue(osc.dispatchEvent(web_audio_api.Event("ended")))
        osc.removeEventListener("ended", listener)
        self.assertEqual(calls, ["ended"])

    def test_script_processor_onaudioprocess_property_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 512, 44_100.0)
        node = ctx.createScriptProcessor(256, 0, 1)
        marker = object()

        self.assertIsNone(node.onaudioprocess)
        node.onaudioprocess = marker
        self.assertIs(node.onaudioprocess, marker)
        node.onaudioprocess = None
        self.assertIsNone(node.onaudioprocess)

    def test_script_processor_onaudioprocess_output_only_offline(self):
        buffer_size = 256
        ctx = web_audio_api.OfflineAudioContext(1, buffer_size * 3, 8_000.0)
        node = ctx.createScriptProcessor(buffer_size, 0, 1)
        events = []
        kept = {}

        def onaudioprocess(event):
            kept["event"] = event
            kept["buffer"] = event.outputBuffer
            events.append(
                (
                    event.type,
                    event.target is node,
                    event.currentTarget is node,
                    event.playbackTime,
                    event.inputBuffer.numberOfChannels,
                    event.outputBuffer.numberOfChannels,
                )
            )
            event.outputBuffer.copyToChannel([1.0] * buffer_size, 0)

        node.onaudioprocess = onaudioprocess
        node.connect(ctx.destination)

        rendered = self.run_async(lambda: ctx.startRendering())
        data = rendered.getChannelData(0)

        self.assertEqual(len(events), 3)
        self.assertTrue(all(item[:3] == ("audioprocess", True, True) for item in events))
        self.assertEqual([item[4:] for item in events], [(1, 1), (1, 1), (1, 1)])
        self.assertEqual(sorted(item[3] for item in events), [item[3] for item in events])
        self.assertTrue(all(sample == 0.0 for sample in data[: 2 * buffer_size]))
        self.assertTrue(all(sample == 1.0 for sample in data[2 * buffer_size :]))

        with self.assertRaises(RuntimeError):
            _ = kept["event"].playbackTime
        with self.assertRaises(RuntimeError):
            kept["buffer"].getChannelData(0)

    def test_script_processor_add_event_listener_with_input_processing(self):
        buffer_size = 256
        ctx = web_audio_api.OfflineAudioContext(1, buffer_size * 3, 8_000.0)
        node = ctx.createScriptProcessor(buffer_size, 1, 1)
        src = ctx.createConstantSource()
        seen = []

        def listener(event):
            seen.append((event.type, event.target is node, event.currentTarget is node))
            data = event.inputBuffer.getChannelData(0)
            event.outputBuffer.copyToChannel([sample * 2.0 for sample in data], 0)

        node.addEventListener("audioprocess", listener)
        src.offset.value = 0.25
        src.connect(node)
        node.connect(ctx.destination)
        src.start()

        rendered = self.run_async(lambda: ctx.startRendering())
        data = rendered.getChannelData(0)

        self.assertEqual(seen, [("audioprocess", True, True)] * 3)
        self.assertTrue(all(sample == 0.0 for sample in data[: 2 * buffer_size]))
        self.assertTrue(all(sample == 0.5 for sample in data[2 * buffer_size :]))

    def test_constant_source_renders_scheduled_samples_offline(self):
        ctx = web_audio_api.OfflineAudioContext(1, 2000, 2000.0)
        src = web_audio_api.ConstantSourceNode(ctx, {"offset": 0.25})

        src.connect(ctx.destination)
        src.start(0.25)
        src.stop(0.75)

        rendered = self.run_async(lambda: ctx.startRendering())
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

        data = self.run_async(lambda: ctx.startRendering()).getChannelData(0)
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

        data = self.run_async(lambda: ctx.startRendering()).getChannelData(0)
        self.assertTrue(all(sample == 0.125 for sample in data))

    def test_offline_audio_context_async_suspend_resume_work(self):
        async def exercise():
            ctx = web_audio_api.OfflineAudioContext(1, 1024, 8_000.0)
            src = ctx.createConstantSource()
            src.connect(ctx.destination)
            src.start()
            suspend_task = ctx.suspend(0.05)
            render_task = asyncio.ensure_future(ctx.startRendering())
            await suspend_task
            self.assertEqual(ctx.state, "suspended")
            await ctx.resume()
            rendered = await render_task
            self.assertEqual(ctx.state, "closed")
            return rendered

        rendered = self.run_async(exercise())
        self.assertEqual(rendered.length, 1024)

    def test_base_audio_context_decode_audio_data_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 8_000.0)
        samples = [0.0, 0.5, -0.5, 0.25]

        decoded = self.run_async(lambda: ctx.decodeAudioData(self.wav_bytes(samples)))

        self.assertIsInstance(decoded, web_audio_api.AudioBuffer)
        self.assertEqual(decoded.numberOfChannels, 1)
        self.assertEqual(decoded.length, len(samples))

    def test_base_audio_context_decode_audio_data_callbacks_work(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 8_000.0)
        success_calls = []
        error_calls = []

        def success_callback(buffer):
            success_calls.append(buffer.length)

        def error_callback(error):
            error_calls.append(type(error).__name__)

        decoded = self.run_async(
            lambda: ctx.decodeAudioData(
                self.wav_bytes([0.0, 0.25, -0.25]),
                success_callback,
                error_callback,
            )
        )

        self.assertEqual(success_calls, [decoded.length])
        self.assertEqual(error_calls, [])

    def test_base_audio_context_decode_audio_data_error_callback_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 8_000.0)
        error_calls = []

        def error_callback(error):
            error_calls.append(error)

        with self.assertRaises(RuntimeError):
            self.run_async(
                lambda: ctx.decodeAudioData(b"not audio data", None, error_callback)
            )

        self.assertEqual(len(error_calls), 1)
        self.assertIsInstance(error_calls[0], RuntimeError)

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
