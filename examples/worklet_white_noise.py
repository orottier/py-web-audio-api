import asyncio
import random

import web_audio_api


class WhiteNoiseProcessor(web_audio_api.AudioWorkletProcessor):
    name = "white-noise-example"

    def __init__(self, options=None):
        self.volume = 0.03
        self._message_handler_bound = False

    def _handle_message(self, event):
        value = event.data
        if isinstance(value, dict) and "volume" in value:
            self.volume = float(value["volume"])

    def process(self, inputs, outputs, parameters):
        if not self._message_handler_bound:
            self.port.onmessage = self._handle_message
            self._message_handler_bound = True
        for output in outputs:
            for channel in output:
                for i in range(len(channel)):
                    channel[i] = random.uniform(-1.0, 1.0) * self.volume
        return True


async def main():
    ctx = web_audio_api.AudioContext()
    ctx.audioWorklet.addModule(WhiteNoiseProcessor)

    node = web_audio_api.AudioWorkletNode(
        ctx,
        "white-noise-example",
        {
            "numberOfInputs": 0,
            "numberOfOutputs": 1,
            "outputChannelCount": [2],
        },
    )
    node.connect(ctx.destination)
    node.onprocessorerror = lambda event: print("processor error:", event.message)

    await ctx.resume()

    for volume in [0.02, 0.08, 0.03, 0.12, 0.05]:
        print(f"setting volume to {volume}")
        node.port.postMessage({"volume": volume})
        await asyncio.sleep(1.0)

    await ctx.close()


asyncio.run(main())
