# Python bindings for web-audio-api-rs

https://pypi.org/project/web-audio-api/

## Local development

Create and activate a virtual environment:

```bash
python3 -m venv .env
source .env/bin/activate
```

Install the development build into the active environment:

```bash
python3 -m pip install maturin
maturin develop
```

Try the binding:

```python
import web_audio_api
ctx = web_audio_api.AudioContext()
osc = web_audio_api.OscillatorNode(ctx)
osc.connect(ctx.destination())
osc.start()
osc.frequency().value = 300
```

## Build

Build a wheel:

```bash
python3 -m pip wheel . --no-deps --wheel-dir dist
```

## Test

Run the Rust tests:

```bash
cargo test
```

Run the Python tests against an installed wheel:

```bash
maturin develop
python3 -m unittest discover -s tests
```
