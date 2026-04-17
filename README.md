# Python bindings for web-audio-api-rs
[![PyPI version](https://img.shields.io/pypi/v/web-audio-api.svg)](https://pypi.org/project/web-audio-api/)

A Rust/Python implementation of the Web Audio API, for use in non-browser contexts.

## Usage

Install from PyPI:

```bash
pip install web-audio-api
```

Create a simple audio context and start an oscillator:

```python
import web_audio_api

ctx = web_audio_api.AudioContext()
osc = ctx.createOscillator()
osc.frequency.value = 300
osc.connect(ctx.destination)
osc.start()
```

## Advanced usage

The binding now exposes asyncio-native awaitables for the Web Audio methods that are async in
`web-audio-api-rs`.

This includes:

- `AudioContext.resume()`
- `AudioContext.suspend()`
- `AudioContext.close()`
- `OfflineAudioContext.startRendering()`
- `OfflineAudioContext.resume()`
- `OfflineAudioContext.suspend(suspendTime)`
- `BaseAudioContext.decodeAudioData(...)`

Use them inside a running event loop:

```python
import asyncio
import web_audio_api


async def main():
    ctx = web_audio_api.AudioContext({"sinkId": "none"})
    await ctx.resume()
    await ctx.suspend()
    await ctx.close()


asyncio.run(main())
```

Offline rendering is also async:

```python
import asyncio
import web_audio_api


async def main():
    ctx = web_audio_api.OfflineAudioContext(1, 2_000, 2_000.0)
    src = ctx.createConstantSource()
    src.offset.value = 0.25
    src.connect(ctx.destination)
    src.start(0.25)
    src.stop(0.75)

    rendered = await ctx.startRendering()
    data = rendered.getChannelData(0)
    print(data[:8])


asyncio.run(main())
```

`decodeAudioData(...)` returns an awaitable and also accepts optional callbacks:

```python
import asyncio
import pathlib
import web_audio_api


async def main():
    ctx = web_audio_api.OfflineAudioContext(1, 128, 44_100.0)
    audio_bytes = pathlib.Path("example.wav").read_bytes()

    def success(buffer):
        print(buffer.length, buffer.sampleRate)

    buffer = await ctx.decodeAudioData(audio_bytes, successCallback=success)
    print(buffer.numberOfChannels)


asyncio.run(main())
```

One practical detail: create these awaitables inside the running loop. In other words, prefer
`asyncio.run(main())` with the Web Audio calls inside `main()`, instead of constructing an
awaitable earlier and awaiting it later.

## Local development

Create and activate a virtual environment:

```bash
uv venv --python 3.11 .venv
source .venv/bin/activate
```

Install the development build into the active environment:

```bash
.venv/bin/python -m pip install maturin
maturin develop
```

Try the binding:

```python
import web_audio_api
ctx = web_audio_api.AudioContext()
osc = ctx.createOscillator()
osc.connect(ctx.destination)
osc.start()
osc.frequency.value = 300
```

## Build

Build a wheel:

```bash
.venv/bin/python -m pip wheel . --no-deps --wheel-dir dist
```

## Test

Run the Rust tests:

```bash
cargo test
```

Run the Python tests against an installed wheel:

```bash
maturin develop
.venv/bin/python -m unittest discover -s tests
```

## Release

Update the version in `pyproject.toml`.
Create and push a tag matching the release version:

```bash
git tag v0.1.0
git push origin main
git push origin v0.1.0
```

Pushing the tag triggers the GitHub Actions release workflow, which builds the release artifacts and uploads them to PyPI.
