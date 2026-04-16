# AGENTS

- Keep the Python public API close to `web-audio-api-idl.txt`, even when `web-audio-api-rs` has a different shape.
- This repo is a binding layer, not a place for audio-domain policy or validation that belongs in Rust.
- Prefer synchronous bindings and small stubs over premature async/event/worklet design.
- Preserve the current inheritance model (`BaseAudioContext`, `AudioScheduledSourceNode`, etc.) when adding surface.
- Reuse the internal node factory helpers in `src/lib.rs` instead of open-coding new wrapper assembly.
- For realtime tests, use `AudioContext({"sinkId": "none"})` so CI and local headless runs stay deterministic.
- Before finishing, run:
  - `cargo fmt --check`
  - `cargo test`
  - `python3 -m pip wheel . --no-deps --wheel-dir /tmp/py-web-audio-api-wheel-test`
  - reinstall the wheel into the test venv
  - `python -m unittest discover -s tests`
