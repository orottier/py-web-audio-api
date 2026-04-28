# Python bindings for web-audio-api-rs
[![PyPI version](https://img.shields.io/pypi/v/web-audio-api.svg)](https://pypi.org/project/web-audio-api/)

A Rust/Python implementation of the Web Audio API, for use in non-browser contexts.

## Usage

Install from PyPI:

```bash
pip install web-audio-api
```

This package currently requires **Python 3.11 or newer**.

Create a simple audio context and start an oscillator:

```python
import web_audio_api

ctx = web_audio_api.AudioContext()
osc = ctx.createOscillator()
osc.frequency.value = 300
osc.connect(ctx.destination)
osc.start()
```

For realtime output diagnostics, `AudioContext.playbackStats` exposes underrun and
latency metrics as a live Web Audio-style surface.

## Examples

Runnable example scripts live in [examples](./examples):

- [examples/osc_gain.py](./examples/osc_gain.py): simple oscillator through a gain node
- [examples/worklet_white_noise.py](./examples/worklet_white_noise.py): `AudioWorklet` white noise with message-based volume changes
- [examples/notebook_quickstart.ipynb](./examples/notebook_quickstart.ipynb): Jupyter notebook quickstart to play with online or offline audio
- [examples/analyser_meter.py](./examples/analyser_meter.py): analyser-based terminal level meter
- [examples/buffer_source.py](./examples/buffer_source.py): build and play a short buffer
- [examples/media_element.py](./examples/media_element.py): play an audio file through `MediaElementAudioSourceNode`
- [examples/mic_input.py](./examples/mic_input.py): microphone input into a graph
- [examples/recorder.py](./examples/recorder.py): record graph output to a WAV file
- [examples/stream_in.py](./examples/stream_in.py): stream Python-generated chunks into a `MediaStreamAudioSourceNode`
- [examples/stream_out.py](./examples/stream_out.py): read graph output back into Python from a `MediaStreamAudioDestinationNode`
- [examples/vu_meter.py](./examples/vu_meter.py): CLI worklet-based VU meter driven by processor port messages

Run an example after `maturin develop` (see Local Development section)

```bash
.venv/bin/python examples/osc_gain.py
```

## Async Methods

The binding exposes asyncio-native awaitables for the Web Audio methods that are async in
`web-audio-api-rs`, such as `AudioContext.resume()` and
`OfflineAudioContext.startRendering()`.

```python
import asyncio
import web_audio_api


async def main():
    ctx = web_audio_api.OfflineAudioContext(1, 3_000, 3_000.0)
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

## AudioWorklet

Python worklets are regular `AudioWorkletProcessor` subclasses:

```python
import web_audio_api


class GainHalfProcessor(web_audio_api.AudioWorkletProcessor):
    name = "gain-half"

    def process(self, inputs, outputs, parameters):
        if inputs and inputs[0]:
            for in_channel, out_channel in zip(inputs[0], outputs[0]):
                for i, sample in enumerate(in_channel):
                    out_channel[i] = sample * 0.5
        return True
```

Register the processor and create a node from it:

```python
ctx.audioWorklet.addModule(GainHalfProcessor)
node = web_audio_api.AudioWorkletNode(ctx, "gain-half")
```

During `process(...)`, the binding also exposes browser-style worklet globals:
- `sampleRate`
- `currentTime`
- `currentFrame`
- `self.port`

## Local Development

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

Run the IDL surface checker:

```bash
.venv/bin/python tools/check_idl_surface.py web-audio-api-idl.txt --both-directions
```

The checker reads the IDL file, imports `web_audio_api`, and verifies that the expected classes,
methods, and properties are present on the Python surface.

## Release Workflow

Update the version in `Cargo.toml`.
Create and push a tag matching the release version; this uploads the artifacts to PyPI.

```bash
git tag v0.3.1
git push origin main
git push origin v0.3.1
```
