import unittest

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
    def test_offline_oscillator_graph_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)

        osc.connect(ctx.destination())
        osc.frequency().value = 300.0

        self.assertEqual(osc.frequency().value, 300.0)
        self.assertEqual(osc.type_, "sine")

        osc.set_type("square")
        self.assertEqual(osc.type_, "square")

        osc.start()
        osc.stop()

    def test_audio_param_methods_work(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)
        frequency = osc.frequency()

        self.assertEqual(frequency.automation_rate, "a-rate")
        self.assertEqual(frequency.default_value, 440.0)
        self.assertLess(frequency.min_value, frequency.max_value)

        frequency.automation_rate = "k-rate"
        self.assertEqual(frequency.automation_rate, "k-rate")

        frequency.value = 220.0
        self.assertEqual(frequency.value, 220.0)

        frequency.set_value_at_time(330.0, 0.0)
        frequency.linear_ramp_to_value_at_time(440.0, 0.1)
        frequency.exponential_ramp_to_value_at_time(660.0, 0.2)
        frequency.set_target_at_time(550.0, 0.3, 0.1)
        frequency.cancel_scheduled_values(0.4)
        frequency.cancel_and_hold_at_time(0.5)
        frequency.set_value_curve_at_time([220.0, 330.0, 440.0], 0.6, 0.2)

    def test_self_connect_reports_rust_error(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)

        with self.assertRaisesRegex(RuntimeError, "input port 0 is out of bounds"):
            osc.connect(osc)


if __name__ == "__main__":
    unittest.main()
