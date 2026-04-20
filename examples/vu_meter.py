import asyncio
import math

import web_audio_api


SMOOTHING_FACTOR = 0.9
MINIMUM_VALUE = 0.00001


class VUMeterProcessor(web_audio_api.AudioWorkletProcessor):
    name = "vu-meter-example"

    def __init__(self, options=None):
        processor_options = (options or {}).get("processorOptions", {})
        self._volume = 0.0
        self._update_interval_ms = float(
            processor_options.get("updateIntervalInMS", 50.0)
        )
        self._next_update_frame = None
        self._message_handler_bound = False

    def _handle_message(self, value):
        if isinstance(value, dict) and "updateIntervalInMS" in value:
            self._update_interval_ms = float(value["updateIntervalInMS"])
            self._next_update_frame = None

    def process(self, inputs, outputs, parameters):
        if not self._message_handler_bound:
            self.port.onmessage = self._handle_message
            self._message_handler_bound = True

        interval_in_frames = self._update_interval_ms / 1000.0 * sampleRate
        if self._next_update_frame is None:
            self._next_update_frame = interval_in_frames
        if inputs and inputs[0]:
            samples = inputs[0][0]
            if samples:
                squared_sum = sum(sample * sample for sample in samples)
                rms = math.sqrt(squared_sum / len(samples))
                self._volume = max(rms, self._volume * SMOOTHING_FACTOR)
                self._next_update_frame -= len(samples)
                if self._next_update_frame < 0:
                    self._next_update_frame += interval_in_frames
                    port.postMessage({"volume": self._volume})
        return self._volume >= MINIMUM_VALUE


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    ctx.audioWorklet.addModule(VUMeterProcessor)

    osc = ctx.createOscillator()
    osc.type = "sawtooth"
    osc.frequency.value = 110.0

    gain = ctx.createGain()
    gain.gain.value = 0.1

    meter = web_audio_api.AudioWorkletNode(
        ctx,
        "vu-meter-example",
        {
            "numberOfInputs": 1,
            "numberOfOutputs": 0,
            "channelCount": 1,
            "processorOptions": {"updateIntervalInMS": 50.0},
        },
    )

    volume = {"value": 0.0}
    meter.port.onmessage = lambda event: volume.__setitem__(
        "value", float(event.data.get("volume", 0.0))
    )

    osc.connect(gain)
    gain.connect(ctx.destination)
    gain.connect(meter)

    osc.start()
    await ctx.resume()

    print("Showing worklet-driven VU meter for 5 seconds...")
    for _ in range(100):
        level = volume["value"]
        bars = "#" * min(40, int(level * 160))
        print(f"\rVU {level:0.4f}  {bars:<40}", end="", flush=True)
        await asyncio.sleep(0.05)

    print()
    osc.stop()
    await ctx.close()


asyncio.run(main())
