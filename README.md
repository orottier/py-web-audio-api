# Python bindings for web-audio-api-rs

https://pypi.org/project/web-audio-api/

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
