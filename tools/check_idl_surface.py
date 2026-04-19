#!/usr/bin/env python3

from __future__ import annotations

import argparse
from dataclasses import dataclass
import importlib
import inspect
from pathlib import Path
import re
import sys


INTERFACE_RE = re.compile(
    r"^\s*(?:\[[^\]]*\]\s*)?interface\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:\s*([A-Za-z_][A-Za-z0-9_]*))?\s*\{(.*?)\};",
    re.MULTILINE | re.DOTALL,
)

ATTRIBUTE_RE = re.compile(
    r"^(?:readonly\s+)?attribute\s+.+?\b([A-Za-z_][A-Za-z0-9_]*)$"
)
METHOD_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(")
LEADING_EXT_ATTR_RE = re.compile(r"^\[[^\]]*\]\s*")

EXCLUDED_INTERFACES = {
    "AudioPlaybackStats": "Not modeled in the Python binding.",
    "AudioSinkInfo": "Not modeled in the Python binding.",
    "AudioWorkletGlobalScope": "Python worklets do not expose a separate global-scope object.",
}

EXCLUDED_MEMBERS = {
    "BaseAudioContext": {
        "attributes": {
            "renderQuantumSize": "Not modeled in the current binding.",
        }
    },
    "AudioContext": {
        "attributes": {
            "onerror": "Not modeled in the current binding.",
            "playbackStats": "Not modeled in the current binding.",
        },
        "methods": {
            "getOutputTimestamp": "Not modeled in the current binding.",
            "setSinkId": "Deferred until the Rust API shape matches.",
        },
    },
}

ALLOWED_PUBLIC_MODULE_EXTRAS = {
    "AudioRenderCapacity": "Binding extension for Rust render-capacity reporting.",
    "AudioRenderCapacityEvent": "Binding extension event type for render-capacity updates.",
    "Blob": "External web-platform support type used by MediaRecorder.",
    "BlobEvent": "External web-platform support event used by MediaRecorder.",
    "ErrorEvent": "External web-platform event type used by worklets.",
    "Event": "External web-platform base event type.",
    "EventTarget": "External web-platform event-target base type.",
    "MediaDeviceInfo": "External media-devices support type.",
    "MediaElement": "Binding-side media element wrapper.",
    "MediaRecorder": "External media-recording support type exposed by the binding.",
    "MediaStream": "External media-stream support type exposed by the binding.",
    "MediaStreamTrack": "External media-stream support type exposed by the binding.",
    "MediaStreamTrackBufferIterator": "Binding helper for consuming MediaStreamTrack buffers.",
    "MessageEvent": "External web-platform messaging event type.",
    "MessagePort": "External web-platform message port type.",
    "enumerateDevices": "Binding-level convenience export for media-device enumeration.",
    "enumerateDevicesSync": "Binding-level synchronous convenience export for media-device enumeration.",
    "getUserMedia": "Binding-level convenience export for media capture.",
    "getUserMediaSync": "Binding-level synchronous convenience export for media capture.",
}

ALLOWED_PUBLIC_MEMBERS = {
    "*": {
        "methods": {
            "addEventListener": "Inherited from the binding's EventTarget support, which is not described in the local Web Audio IDL file.",
            "dispatchEvent": "Inherited from the binding's EventTarget support, which is not described in the local Web Audio IDL file.",
            "removeEventListener": "Inherited from the binding's EventTarget support, which is not described in the local Web Audio IDL file.",
        }
    },
    "AudioContext": {
        "attributes": {
            "renderCapacity": "Binding extension for Rust render-capacity reporting.",
        }
    },
    "AudioParamMap": {
        "methods": {
            "get": "Python mapping convenience method on the binding wrapper.",
            "items": "Python mapping convenience method on the binding wrapper.",
            "keys": "Python mapping convenience method on the binding wrapper.",
        }
    },
    "AudioProcessingEvent": {
        "attributes": {
            "currentTarget": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
            "target": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
            "type": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
        }
    },
    "AudioWorklet": {
        "methods": {
            "addModule": "Inherited from Worklet, which is outside the local Web Audio IDL file.",
        }
    },
    "MediaStreamTrackAudioSourceNode": {
        "attributes": {
            "mediaStreamTrack": "Binding extension that retains the originating MediaStreamTrack.",
        }
    },
    "OfflineAudioCompletionEvent": {
        "attributes": {
            "currentTarget": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
            "target": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
            "type": "Inherited from the binding's Event support, which is not described in the local Web Audio IDL file.",
        }
    },
}


@dataclass(frozen=True)
class InterfaceSurface:
    name: str
    base: str | None
    attributes: frozenset[str]
    methods: frozenset[str]


@dataclass(frozen=True)
class SurfaceCheckResult:
    missing_interfaces: tuple[str, ...]
    missing_attributes: tuple[tuple[str, str], ...]
    missing_methods: tuple[tuple[str, str], ...]
    skipped_interfaces: tuple[tuple[str, str], ...]
    skipped_attributes: tuple[tuple[str, str, str], ...]
    skipped_methods: tuple[tuple[str, str, str], ...]

    @property
    def ok(self) -> bool:
        return (
            not self.missing_interfaces
            and not self.missing_attributes
            and not self.missing_methods
        )


@dataclass(frozen=True)
class ReverseSurfaceCheckResult:
    unexpected_module_names: tuple[str, ...]
    unexpected_attributes: tuple[tuple[str, str], ...]
    unexpected_methods: tuple[tuple[str, str], ...]
    allowed_module_names: tuple[tuple[str, str], ...]
    allowed_attributes: tuple[tuple[str, str, str], ...]
    allowed_methods: tuple[tuple[str, str, str], ...]

    @property
    def ok(self) -> bool:
        return (
            not self.unexpected_module_names
            and not self.unexpected_attributes
            and not self.unexpected_methods
        )


def parse_interfaces(idl_text: str) -> list[InterfaceSurface]:
    interfaces: list[InterfaceSurface] = []
    for name, base, body in INTERFACE_RE.findall(idl_text):
        attributes: set[str] = set()
        methods: set[str] = set()
        for raw_stmt in body.split(";"):
            stmt = " ".join(raw_stmt.strip().split())
            if not stmt:
                continue
            while True:
                updated = LEADING_EXT_ATTR_RE.sub("", stmt, count=1)
                if updated == stmt:
                    break
                stmt = updated.strip()
            if not stmt or stmt.startswith("constructor"):
                continue
            attr_match = ATTRIBUTE_RE.match(stmt)
            if attr_match:
                attributes.add(attr_match.group(1))
                continue
            method_match = METHOD_RE.search(stmt)
            if method_match:
                method_name = method_match.group(1)
                if method_name != "constructor":
                    methods.add(method_name)
        interfaces.append(
            InterfaceSurface(
                name=name,
                base=base or None,
                attributes=frozenset(sorted(attributes)),
                methods=frozenset(sorted(methods)),
            )
        )
    return interfaces


def has_class_member(cls: type, name: str) -> bool:
    sentinel = object()
    return inspect.getattr_static(cls, name, sentinel) is not sentinel


def public_class_members(cls: type) -> tuple[frozenset[str], frozenset[str]]:
    attributes: set[str] = set()
    methods: set[str] = set()

    for name in dir(cls):
        if name.startswith("_"):
            continue
        try:
            value = inspect.getattr_static(cls, name)
        except Exception:
            continue
        if inspect.isroutine(value):
            methods.add(name)
        else:
            attributes.add(name)

    return frozenset(sorted(attributes)), frozenset(sorted(methods))


def interface_map(interfaces: list[InterfaceSurface]) -> dict[str, InterfaceSurface]:
    return {interface.name: interface for interface in interfaces}


def inherited_interface_attributes(
    interfaces_by_name: dict[str, InterfaceSurface], name: str
) -> frozenset[str]:
    interface = interfaces_by_name.get(name)
    if interface is None:
        return frozenset()

    attributes = set(interface.attributes)
    if interface.base is not None:
        attributes.update(inherited_interface_attributes(interfaces_by_name, interface.base))
    return frozenset(sorted(attributes))


def inherited_interface_methods(
    interfaces_by_name: dict[str, InterfaceSurface], name: str
) -> frozenset[str]:
    interface = interfaces_by_name.get(name)
    if interface is None:
        return frozenset()

    methods = set(interface.methods)
    if interface.base is not None:
        methods.update(inherited_interface_methods(interfaces_by_name, interface.base))
    return frozenset(sorted(methods))


def check_surface(module, interfaces: list[InterfaceSurface]) -> SurfaceCheckResult:
    missing_interfaces: list[str] = []
    missing_attributes: list[tuple[str, str]] = []
    missing_methods: list[tuple[str, str]] = []
    skipped_interfaces: list[tuple[str, str]] = []
    skipped_attributes: list[tuple[str, str, str]] = []
    skipped_methods: list[tuple[str, str, str]] = []

    for interface in interfaces:
        excluded_interface_reason = EXCLUDED_INTERFACES.get(interface.name)
        if excluded_interface_reason is not None:
            skipped_interfaces.append((interface.name, excluded_interface_reason))
            continue

        cls = getattr(module, interface.name, None)
        if cls is None:
            missing_interfaces.append(interface.name)
            continue

        excluded = EXCLUDED_MEMBERS.get(interface.name, {})
        excluded_attributes = excluded.get("attributes", {})
        excluded_methods = excluded.get("methods", {})

        for attribute in sorted(interface.attributes):
            reason = excluded_attributes.get(attribute)
            if reason is not None:
                skipped_attributes.append((interface.name, attribute, reason))
                continue
            if not has_class_member(cls, attribute):
                missing_attributes.append((interface.name, attribute))

        for method in sorted(interface.methods):
            reason = excluded_methods.get(method)
            if reason is not None:
                skipped_methods.append((interface.name, method, reason))
                continue
            if not has_class_member(cls, method):
                missing_methods.append((interface.name, method))

    return SurfaceCheckResult(
        missing_interfaces=tuple(missing_interfaces),
        missing_attributes=tuple(missing_attributes),
        missing_methods=tuple(missing_methods),
        skipped_interfaces=tuple(skipped_interfaces),
        skipped_attributes=tuple(skipped_attributes),
        skipped_methods=tuple(skipped_methods),
    )


def allowed_member_reason(
    interface_name: str, member_kind: str, member_name: str
) -> str | None:
    wildcard_reason = (
        ALLOWED_PUBLIC_MEMBERS.get("*", {}).get(member_kind, {}).get(member_name)
    )
    if wildcard_reason is not None:
        return wildcard_reason
    return (
        ALLOWED_PUBLIC_MEMBERS.get(interface_name, {})
        .get(member_kind, {})
        .get(member_name)
    )


def check_reverse_surface(module, interfaces: list[InterfaceSurface]) -> ReverseSurfaceCheckResult:
    interfaces_by_name = interface_map(interfaces)
    public_names = sorted(name for name in dir(module) if not name.startswith("_"))

    unexpected_module_names: list[str] = []
    unexpected_attributes: list[tuple[str, str]] = []
    unexpected_methods: list[tuple[str, str]] = []
    allowed_module_names: list[tuple[str, str]] = []
    allowed_attributes: list[tuple[str, str, str]] = []
    allowed_methods: list[tuple[str, str, str]] = []

    for name in public_names:
        cls = getattr(module, name)
        interface = interfaces_by_name.get(name)
        if interface is None:
            reason = ALLOWED_PUBLIC_MODULE_EXTRAS.get(name)
            if reason is not None:
                allowed_module_names.append((name, reason))
            else:
                unexpected_module_names.append(name)
            continue

        expected_attributes = inherited_interface_attributes(interfaces_by_name, name)
        expected_methods = inherited_interface_methods(interfaces_by_name, name)
        actual_attributes, actual_methods = public_class_members(cls)

        for attribute in sorted(actual_attributes - expected_attributes):
            reason = allowed_member_reason(name, "attributes", attribute)
            if reason is not None:
                allowed_attributes.append((name, attribute, reason))
            else:
                unexpected_attributes.append((name, attribute))

        for method in sorted(actual_methods - expected_methods):
            reason = allowed_member_reason(name, "methods", method)
            if reason is not None:
                allowed_methods.append((name, method, reason))
            else:
                unexpected_methods.append((name, method))

    return ReverseSurfaceCheckResult(
        unexpected_module_names=tuple(unexpected_module_names),
        unexpected_attributes=tuple(unexpected_attributes),
        unexpected_methods=tuple(unexpected_methods),
        allowed_module_names=tuple(allowed_module_names),
        allowed_attributes=tuple(allowed_attributes),
        allowed_methods=tuple(allowed_methods),
    )


def format_result(result: SurfaceCheckResult, verbose: bool = False) -> str:
    lines: list[str] = []

    if result.ok:
        lines.append("IDL surface check passed.")
    else:
        lines.append("IDL surface check failed.")

    lines.append(f"Missing interfaces: {len(result.missing_interfaces)}")
    lines.append(f"Missing attributes: {len(result.missing_attributes)}")
    lines.append(f"Missing methods: {len(result.missing_methods)}")

    if result.missing_interfaces:
        lines.append("")
        lines.append("Missing interfaces:")
        lines.extend(f"  - {name}" for name in result.missing_interfaces)

    if result.missing_attributes:
        lines.append("")
        lines.append("Missing attributes:")
        lines.extend(f"  - {interface}.{attribute}" for interface, attribute in result.missing_attributes)

    if result.missing_methods:
        lines.append("")
        lines.append("Missing methods:")
        lines.extend(f"  - {interface}.{method}" for interface, method in result.missing_methods)

    if verbose:
        if result.skipped_interfaces:
            lines.append("")
            lines.append("Skipped interfaces:")
            lines.extend(
                f"  - {name}: {reason}" for name, reason in result.skipped_interfaces
            )
        if result.skipped_attributes:
            lines.append("")
            lines.append("Skipped attributes:")
            lines.extend(
                f"  - {interface}.{attribute}: {reason}"
                for interface, attribute, reason in result.skipped_attributes
            )
        if result.skipped_methods:
            lines.append("")
            lines.append("Skipped methods:")
            lines.extend(
                f"  - {interface}.{method}: {reason}"
                for interface, method, reason in result.skipped_methods
            )

    return "\n".join(lines)


def format_reverse_result(
    result: ReverseSurfaceCheckResult, verbose: bool = False
) -> str:
    lines: list[str] = []

    if result.ok:
        lines.append("Reverse surface check passed.")
    else:
        lines.append("Reverse surface check failed.")

    lines.append(f"Unexpected module names: {len(result.unexpected_module_names)}")
    lines.append(f"Unexpected attributes: {len(result.unexpected_attributes)}")
    lines.append(f"Unexpected methods: {len(result.unexpected_methods)}")

    if result.unexpected_module_names:
        lines.append("")
        lines.append("Unexpected module names:")
        lines.extend(f"  - {name}" for name in result.unexpected_module_names)

    if result.unexpected_attributes:
        lines.append("")
        lines.append("Unexpected attributes:")
        lines.extend(
            f"  - {interface}.{attribute}"
            for interface, attribute in result.unexpected_attributes
        )

    if result.unexpected_methods:
        lines.append("")
        lines.append("Unexpected methods:")
        lines.extend(
            f"  - {interface}.{method}" for interface, method in result.unexpected_methods
        )

    if verbose:
        if result.allowed_module_names:
            lines.append("")
            lines.append("Allowed extra module names:")
            lines.extend(
                f"  - {name}: {reason}" for name, reason in result.allowed_module_names
            )
        if result.allowed_attributes:
            lines.append("")
            lines.append("Allowed extra attributes:")
            lines.extend(
                f"  - {interface}.{attribute}: {reason}"
                for interface, attribute, reason in result.allowed_attributes
            )
        if result.allowed_methods:
            lines.append("")
            lines.append("Allowed extra methods:")
            lines.extend(
                f"  - {interface}.{method}: {reason}"
                for interface, method, reason in result.allowed_methods
            )

    return "\n".join(lines)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check that the Python binding exposes the IDL surface."
    )
    parser.add_argument("idl_path", help="Path to the Web Audio IDL file to check.")
    parser.add_argument(
        "--module",
        default="web_audio_api",
        help="Python module to import and validate (default: web_audio_api).",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print skipped interfaces and members as well as missing ones.",
    )
    parser.add_argument(
        "--both-directions",
        action="store_true",
        help="Run both the forward IDL check and the reverse public-surface check.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    idl_text = Path(args.idl_path).read_text()
    interfaces = parse_interfaces(idl_text)
    module = importlib.import_module(args.module)
    result = check_surface(module, interfaces)
    print(format_result(result, verbose=args.verbose))

    reverse_ok = True
    if args.both_directions:
        reverse_result = check_reverse_surface(module, interfaces)
        print("")
        print(format_reverse_result(reverse_result, verbose=args.verbose))
        reverse_ok = reverse_result.ok

    return 0 if result.ok and reverse_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
