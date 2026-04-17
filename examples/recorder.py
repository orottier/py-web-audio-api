import asyncio
import pathlib
import time

import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    osc = ctx.createOscillator()
    gain = ctx.createGain()
    dest = ctx.createMediaStreamDestination()
    recorder = web_audio_api.MediaRecorder(dest.stream)

    osc.frequency.value = 220.0
    gain.gain.value = 0.08
    osc.connect(gain)
    gain.connect(dest)

    chunks = []
    recorder.ondataavailable = lambda event: chunks.append(event.data.bytes())

    osc.start()
    await ctx.resume()
    recorder.start()

    print("Recording 2 seconds of oscillator output...")
    await asyncio.sleep(2.0)
    recorder.stop()

    deadline = time.time() + 2.0
    while recorder.state != "inactive" and time.time() < deadline:
        await asyncio.sleep(0.01)

    output_path = pathlib.Path("recording.wav")
    output_path.write_bytes(b"".join(chunks))
    print(f"wrote {output_path.resolve()}")

    osc.stop()
    dest.stream.close()
    await ctx.close()


asyncio.run(main())
