import argparse
import asyncio
import pathlib

import web_audio_api


async def main():
    parser = argparse.ArgumentParser(
        description="Play an audio file through MediaElementAudioSourceNode."
    )
    parser.add_argument("path", type=pathlib.Path, help="Path to an audio file")
    args = parser.parse_args()

    ctx = web_audio_api.AudioContext()
    media = web_audio_api.MediaElement(args.path)
    source = ctx.createMediaElementSource(media)
    gain = ctx.createGain()

    gain.gain.value = 0.2
    source.connect(gain)
    gain.connect(ctx.destination)

    media.play()
    await ctx.resume()

    print(f"Playing {args.path} through MediaElementAudioSourceNode.")
    print("Press Ctrl-C to stop.")

    try:
        while not media.paused:
            await asyncio.sleep(0.1)
    except KeyboardInterrupt:
        print("\nStopping playback...")
    finally:
        media.pause()
        await ctx.close()


asyncio.run(main())
