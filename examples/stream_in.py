import asyncio
import math

import web_audio_api

SAMPLE_RATE = 48_000.0
DURATION_SECONDS = 5.0
FREQUENCY = 220.0
CHUNK_SIZE = 128


def sine_chunks(
    sample_rate=SAMPLE_RATE,
    frequency=FREQUENCY,
    chunk_size=CHUNK_SIZE,
    duration_seconds=DURATION_SECONDS,
):
    phase = 0.0
    phase_step = 2.0 * math.pi * frequency / sample_rate
    chunks = math.ceil(duration_seconds * sample_rate / chunk_size)

    for _ in range(chunks):
        chunk = []
        for _ in range(chunk_size):
            chunk.append(math.sin(phase) * 0.15)
            phase += phase_step
        yield chunk


async def main():
    ctx = web_audio_api.AudioContext()
    render_capacity = ctx.renderCapacity
    stream = web_audio_api.MediaStream.fromBufferIterator(
        sine_chunks(),
        sampleRate=SAMPLE_RATE,
        numberOfChannels=1,
    )

    src = ctx.createMediaStreamSource(stream)
    gain = ctx.createGain()
    gain.gain.value = 0.8

    def on_update(event):
        print(
            "renderCapacity:",
            f"t={event.timestamp:0.2f}s",
            f"avg={event.averageLoad:0.2f}",
            f"peak={event.peakLoad:0.2f}",
            f"underrun={event.underrunRatio:0.2f}",
        )

    render_capacity.onupdate = on_update
    render_capacity.start({"updateInterval": 1.0})

    src.connect(gain)
    gain.connect(ctx.destination)

    await ctx.resume()
    print(
        f"Streaming Python-generated sine chunks into the audio graph for {DURATION_SECONDS:.0f} seconds..."
    )
    await asyncio.sleep(DURATION_SECONDS)

    stream.close()
    render_capacity.stop()
    await ctx.close()


asyncio.run(main())
