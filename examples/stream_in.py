import asyncio
import math

import web_audio_api


def sine_chunks(sample_rate=8_000.0, frequency=220.0, chunk_size=128, chunks=80):
    phase = 0.0
    phase_step = 2.0 * math.pi * frequency / sample_rate

    for _ in range(chunks):
        chunk = []
        for _ in range(chunk_size):
            chunk.append(math.sin(phase) * 0.15)
            phase += phase_step
        yield chunk


async def main():
    ctx = web_audio_api.AudioContext()
    stream = web_audio_api.MediaStream.fromBufferIterator(
        sine_chunks(),
        sampleRate=8_000.0,
        numberOfChannels=1,
    )

    src = ctx.createMediaStreamSource(stream)
    gain = ctx.createGain()
    gain.gain.value = 0.8

    src.connect(gain)
    gain.connect(ctx.destination)

    await ctx.resume()
    print("Streaming Python-generated sine chunks into the audio graph for 2 seconds...")
    await asyncio.sleep(2.0)

    stream.close()
    await ctx.close()


asyncio.run(main())
