import asyncio
import math

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext()
    sample_rate = 44_100.0
    duration = 1.0
    length = int(sample_rate * duration)

    buffer = ctx.createBuffer(1, length, sample_rate)
    samples = [
        math.sin(2.0 * math.pi * 330.0 * i / sample_rate)
        * (1.0 - (i / length))
        * 0.2
        for i in range(length)
    ]
    buffer.copyToChannel(samples, 0)

    src = ctx.createBufferSource()
    src.buffer = buffer
    src.connect(ctx.destination)

    await ctx.resume()
    print("Playing a short buffer source...")
    src.start()
    await asyncio.sleep(1.2)
    await ctx.close()


asyncio.run(main())
