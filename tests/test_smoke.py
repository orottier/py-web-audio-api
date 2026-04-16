import unittest

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
    def test_offline_oscillator_graph_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = ctx.createOscillator()

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


if __name__ == "__main__":
    unittest.main()
