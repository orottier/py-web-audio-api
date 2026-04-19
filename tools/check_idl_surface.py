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
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    idl_text = Path(args.idl_path).read_text()
    interfaces = parse_interfaces(idl_text)
    module = importlib.import_module(args.module)
    result = check_surface(module, interfaces)
    print(format_result(result, verbose=args.verbose))
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
