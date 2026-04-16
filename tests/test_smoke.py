import unittest

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
