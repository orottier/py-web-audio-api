import asyncio
import math

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext()
    stream = await web_audio_api.getUserMedia({"audio": True})

    mic = ctx.createMediaStreamSource(stream)
    analyser = ctx.createAnalyser()
    analyser.fftSize = 2048
    analyser.smoothingTimeConstant = 0.2

    mic.connect(analyser)
    mic.connect(ctx.destination)

    await ctx.resume()
    print("Mic routed to destination. Headphones are a good idea.")
    print("Speak into the mic. Ctrl-C to stop.")

    try:
        while True:
            data = analyser.getFloatTimeDomainData([0.0] * analyser.fftSize)
            rms = math.sqrt(sum(sample * sample for sample in data) / len(data))
            peak = max(abs(sample) for sample in data)
            bars = "#" * min(40, int(rms * 200))
            print(f"\rRMS {rms:0.4f}  PEAK {peak:0.4f}  {bars:<40}", end="", flush=True)
            await asyncio.sleep(0.05)
    finally:
        print()
        stream.close()
        await ctx.close()


asyncio.run(main())
