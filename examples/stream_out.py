import asyncio

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    src = ctx.createConstantSource()
    src.offset.value = 0.25

    dest = ctx.createMediaStreamDestination()
    iterator = dest.stream.iterBuffers()

    src.connect(dest)
    src.start()
    await ctx.resume()

    print("Reading 5 AudioBuffers from a MediaStream destination:")
    for index in range(5):
        buffer = next(iterator)
        samples = buffer.getChannelData(0)
        mean = sum(samples) / len(samples)
        peak = max(abs(sample) for sample in samples)
        print(
            f"buffer {index}: channels={buffer.numberOfChannels}, "
            f"length={buffer.length}, mean={mean:.4f}, peak={peak:.4f}"
        )

    src.stop()
    dest.stream.close()
    await ctx.close()


asyncio.run(main())
