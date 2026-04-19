import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import types
import unittest

import web_audio_api


TOOLS_PATH = Path(__file__).resolve().parent.parent / "tools" / "check_idl_surface.py"
IDL_PATH = Path(__file__).resolve().parent.parent / "web-audio-api-idl.txt"

spec = importlib.util.spec_from_file_location("check_idl_surface", TOOLS_PATH)
check_idl_surface = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = check_idl_surface
spec.loader.exec_module(check_idl_surface)


class IdlSurfaceScriptTest(unittest.TestCase):
    def test_parse_interfaces_finds_known_members(self):
        interfaces = {
            interface.name: interface
            for interface in check_idl_surface.parse_interfaces(IDL_PATH.read_text())
        }

        self.assertIn("AudioContext", interfaces)
        self.assertIn("close", interfaces["AudioContext"].methods)
        self.assertIn("sinkId", interfaces["AudioContext"].attributes)
        self.assertIn("AudioBuffer", interfaces)
        self.assertIn("getChannelData", interfaces["AudioBuffer"].methods)
        self.assertIn("length", interfaces["AudioBuffer"].attributes)

    def test_check_surface_passes_for_current_module_with_exclusions(self):
        interfaces = check_idl_surface.parse_interfaces(IDL_PATH.read_text())
        result = check_idl_surface.check_surface(web_audio_api, interfaces)

        self.assertTrue(result.ok, check_idl_surface.format_result(result, verbose=True))
        self.assertGreater(len(result.skipped_interfaces), 0)
        self.assertGreater(len(result.skipped_attributes), 0)
        self.assertGreater(len(result.skipped_methods), 0)

    def test_check_surface_reports_missing_interface(self):
        text = textwrap.dedent(
            """
            interface MissingNode : AudioNode {
                readonly attribute double value;
            };
            """
        )
        interfaces = check_idl_surface.parse_interfaces(text)
        result = check_idl_surface.check_surface(web_audio_api, interfaces)

        self.assertFalse(result.ok)
        self.assertEqual(result.missing_interfaces, ("MissingNode",))

    def test_check_surface_reports_missing_members(self):
        text = textwrap.dedent(
            """
            interface GainNode : AudioNode {
                readonly attribute AudioParam gain;
                undefined definitelyMissingMethod ();
                readonly attribute double definitelyMissingAttribute;
            };
            """
        )
        interfaces = check_idl_surface.parse_interfaces(text)
        result = check_idl_surface.check_surface(web_audio_api, interfaces)

        self.assertFalse(result.ok)
        self.assertIn(("GainNode", "definitelyMissingMethod"), result.missing_methods)
        self.assertIn(
            ("GainNode", "definitelyMissingAttribute"), result.missing_attributes
        )

    def test_verbose_output_mentions_skipped_items(self):
        interfaces = check_idl_surface.parse_interfaces(IDL_PATH.read_text())
        result = check_idl_surface.check_surface(web_audio_api, interfaces)
        output = check_idl_surface.format_result(result, verbose=True)

        self.assertIn("Skipped interfaces:", output)
        self.assertIn("AudioPlaybackStats", output)
        self.assertIn("BaseAudioContext.renderQuantumSize", output)
        self.assertIn("AudioContext.setSinkId", output)

    def test_cli_succeeds_for_current_idl(self):
        completed = subprocess.run(
            [sys.executable, str(TOOLS_PATH), str(IDL_PATH)],
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn("IDL surface check passed.", completed.stdout)

    def test_reverse_check_passes_for_current_module_with_allowlist(self):
        interfaces = check_idl_surface.parse_interfaces(IDL_PATH.read_text())
        result = check_idl_surface.check_reverse_surface(web_audio_api, interfaces)

        self.assertTrue(
            result.ok, check_idl_surface.format_reverse_result(result, verbose=True)
        )
        self.assertGreater(len(result.allowed_module_names), 0)
        self.assertGreater(len(result.allowed_attributes), 0)
        self.assertGreater(len(result.allowed_methods), 0)

    def test_reverse_check_reports_unexpected_public_names(self):
        interfaces = check_idl_surface.parse_interfaces(
            textwrap.dedent(
                """
                interface GainNode : AudioNode {
                    readonly attribute AudioParam gain;
                };
                """
            )
        )

        class FakeGainNode:
            gain = property(lambda self: None)
            definitelyExtraAttribute = 1

            def definitelyExtraMethod(self):
                return None

        fake_module = types.SimpleNamespace(
            GainNode=FakeGainNode,
            definitelyExtraExport=object(),
        )

        result = check_idl_surface.check_reverse_surface(fake_module, interfaces)

        self.assertFalse(result.ok)
        self.assertEqual(result.unexpected_module_names, ("definitelyExtraExport",))
        self.assertIn(
            ("GainNode", "definitelyExtraAttribute"), result.unexpected_attributes
        )
        self.assertIn(("GainNode", "definitelyExtraMethod"), result.unexpected_methods)

    def test_cli_succeeds_for_current_idl_with_reverse_check(self):
        completed = subprocess.run(
            [sys.executable, str(TOOLS_PATH), str(IDL_PATH), "--both-directions"],
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assertIn("Reverse surface check passed.", completed.stdout)

    def test_cli_fails_for_missing_interface(self):
        with tempfile.NamedTemporaryFile("w", suffix=".idl", delete=False) as handle:
            handle.write(
                textwrap.dedent(
                    """
                    interface MissingNode : AudioNode {
                        readonly attribute double value;
                    };
                    """
                )
            )
            path = handle.name

        try:
            completed = subprocess.run(
                [sys.executable, str(TOOLS_PATH), path],
                check=False,
                capture_output=True,
                text=True,
            )
        finally:
            Path(path).unlink(missing_ok=True)

        self.assertEqual(completed.returncode, 1)
        self.assertIn("Missing interfaces:", completed.stdout)
        self.assertIn("MissingNode", completed.stdout)

    def test_cli_fails_for_missing_member(self):
        with tempfile.NamedTemporaryFile("w", suffix=".idl", delete=False) as handle:
            handle.write(
                textwrap.dedent(
                    """
                    interface GainNode : AudioNode {
                        readonly attribute AudioParam gain;
                        undefined definitelyMissingMethod ();
                    };
                    """
                )
            )
            path = handle.name

        try:
            completed = subprocess.run(
                [sys.executable, str(TOOLS_PATH), path],
                check=False,
                capture_output=True,
                text=True,
            )
        finally:
            Path(path).unlink(missing_ok=True)

        self.assertEqual(completed.returncode, 1)
        self.assertIn("Missing methods:", completed.stdout)
        self.assertIn("GainNode.definitelyMissingMethod", completed.stdout)


if __name__ == "__main__":
    unittest.main()
