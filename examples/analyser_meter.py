import asyncio
import math

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    osc = ctx.createOscillator()
    osc.type = "sawtooth"
    osc.frequency.value = 110.0

    gain = ctx.createGain()
    gain.gain.value = 0.1

    analyser = ctx.createAnalyser()
    analyser.fftSize = 1024
    analyser.smoothingTimeConstant = 0.2

    osc.connect(gain)
    gain.connect(analyser)
    osc.start()

    await ctx.resume()
    print("Showing RMS/peak levels for 3 seconds...")

    for _ in range(60):
        data = analyser.getFloatTimeDomainData([0.0] * analyser.fftSize)
        rms = math.sqrt(sum(sample * sample for sample in data) / len(data))
        peak = max(abs(sample) for sample in data)
        bars = "#" * min(40, int(rms * 180))
        print(f"\rRMS {rms:0.4f}  PEAK {peak:0.4f}  {bars:<40}", end="", flush=True)
        await asyncio.sleep(0.05)

    print()
    osc.stop()
    await ctx.close()


asyncio.run(main())
