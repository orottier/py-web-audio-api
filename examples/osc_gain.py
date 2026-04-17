import asyncio

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext()
    osc = ctx.createOscillator()
    gain = ctx.createGain()

    osc.type = "sine"
    osc.frequency.value = 220.0
    gain.gain.value = 0.08

    osc.connect(gain)
    gain.connect(ctx.destination)

    osc.start()
    await ctx.resume()

    print("Playing a 220 Hz sine for 2 seconds...")
    await asyncio.sleep(2.0)

    osc.stop()
    await ctx.close()


asyncio.run(main())
