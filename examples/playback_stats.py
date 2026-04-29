import asyncio
import time

import web_audio_api


class SleepyPassthroughProcessor(web_audio_api.AudioWorkletProcessor):
    name = "sleepy-passthrough-example"

    def __init__(self, options=None):
        processor_options = (options or {}).get("processorOptions", {})
        self._sleep_ms = float(processor_options.get("sleepMs", 0.0))
        self._handler_bound = False

    def _handle_message(self, event):
        value = event.data
        if isinstance(value, dict) and "sleepMs" in value:
            self._sleep_ms = max(0.0, float(value["sleepMs"]))
            self.port.postMessage({"sleepMs": self._sleep_ms})

    def process(self, inputs, outputs, parameters):
        if not self._handler_bound:
            self.port.onmessage = self._handle_message
            self._handler_bound = True

        if inputs and outputs and inputs[0] and outputs[0]:
            for in_channel, out_channel in zip(inputs[0], outputs[0]):
                for i, sample in enumerate(in_channel):
                    out_channel[i] = sample

        if self._sleep_ms > 0.0:
            time.sleep(self._sleep_ms / 1000.0)

        return True


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    ctx.audioWorklet.addModule(SleepyPassthroughProcessor)
    render_capacity = ctx.renderCapacity

    node = web_audio_api.AudioWorkletNode(
        ctx,
        "sleepy-passthrough-example",
        {
            "numberOfInputs": 1,
            "numberOfOutputs": 1,
            "channelCount": 1,
            "processorOptions": {"sleepMs": 0.0},
        },
    )

    current_sleep_ms = {"value": 0.0}
    latest_capacity = {
        "averageLoad": 0.0,
        "peakLoad": 0.0,
        "underrunRatio": 0.0,
    }
    node.port.onmessage = lambda event: current_sleep_ms.__setitem__(
        "value", float(event.data.get("sleepMs", current_sleep_ms["value"]))
    )
    render_capacity.onupdate = lambda event: latest_capacity.update(
        {
            "averageLoad": float(event.averageLoad),
            "peakLoad": float(event.peakLoad),
            "underrunRatio": float(event.underrunRatio),
        }
    )

    osc = ctx.createOscillator()
    osc.type = "sawtooth"
    osc.frequency.value = 110.0
    gain = ctx.createGain()
    gain.gain.value = 0.05

    osc.connect(gain)
    gain.connect(node)
    node.connect(ctx.destination)

    osc.start()
    render_capacity.start({"updateInterval": 0.25})
    await ctx.resume()

    print("Watching AudioContext.playbackStats and renderCapacity while a worklet sleeps on the audio thread.")

    schedule = [
        (3.0, 1.0),
        (6.0, 2.0),
        (9.0, 3.0),
        (12.0, 5.0),
        (15.0, 0.0),
    ]
    schedule_index = 0
    started = time.monotonic()

    while True:
        elapsed = time.monotonic() - started
        if schedule_index < len(schedule) and elapsed >= schedule[schedule_index][0]:
            sleep_ms = schedule[schedule_index][1]
            node.port.postMessage({"sleepMs": sleep_ms})
            schedule_index += 1

        snapshot = ctx.playbackStats.toJSON()
        print(
            (
                f"sleep={current_sleep_ms['value']:>4.1f} ms  "
                f"underruns={snapshot['underrunEvents']:>4}  "
                f"underrunDuration={snapshot['underrunDuration']:.4f}s  "
                f"total={snapshot['totalDuration']:.2f}s  "
                f"avgLatency={snapshot['averageLatency']:.4f}s  "
                f"maxLatency={snapshot['maximumLatency']:.4f}s  "
                f"avgLoad={latest_capacity['averageLoad']:.3f}  "
                f"peakLoad={latest_capacity['peakLoad']:.3f}  "
                f"underrunRatio={latest_capacity['underrunRatio']:.3f}"
            ),
            flush=True,
        )

        if elapsed >= 20.0:
            break
        await asyncio.sleep(0.5)

    print()
    render_capacity.stop()
    osc.stop()
    await ctx.close()


asyncio.run(main())
