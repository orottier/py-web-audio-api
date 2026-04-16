import unittest

import web_audio_api


class WebAudioApiSmokeTest(unittest.TestCase):
    def test_offline_oscillator_graph_works(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)

        osc.connect(ctx.destination())
        osc.frequency().value = 300.0

        self.assertEqual(osc.frequency().value, 300.0)

        osc.start()
        osc.stop()

    def test_self_connect_reports_rust_error(self):
        ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
        osc = web_audio_api.OscillatorNode(ctx)

        with self.assertRaisesRegex(RuntimeError, "input port 0 is out of bounds"):
            osc.connect(osc)


if __name__ == "__main__":
    unittest.main()
