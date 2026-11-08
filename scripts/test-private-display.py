#!/usr/bin/env python3
"""Source-only fixture isolation checks for the native Windows display adapter."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('private_display', Path(__file__).with_name('private-display.py'))
private = importlib.util.module_from_spec(spec)
spec.loader.exec_module(private)


class NativeDisplayTests(unittest.TestCase):
    def test_fixture_directories_override_normal_data_without_clipboard_or_display_inheritance(self):
        with tempfile.TemporaryDirectory(prefix='dpdf-native-environment-') as directory:
            folder = Path(directory)
            original = {'DPDF_CONFIG_DIR': '/normal/config', 'DPDF_DATA_DIR': '/normal/data',
                        'DISPLAY': ':0', 'WAYLAND_DISPLAY': 'wayland-0', 'XAUTHORITY': '/normal/authority',
                        'GDK_BACKEND': 'wayland', 'DPDF_SMOKE_CLIPBOARD': '1',
                        'DPDF_SMOKE_MARKS_CLIPBOARD': '1', 'DPDF_SMOKE_PREFERENCES': '1',
                        'DPDF_SMOKE_SNAPSHOT': '/normal/screenshot', 'DPDF_SMOKE_PLACEMENT_SNAPSHOT': '/normal/placement',
                        'DPDF_SMOKE_ASSEMBLY_SOURCE': '/normal/source.pdf', 'DPDF_SMOKE_REVIEW_SNAPSHOT': '/normal/review', 'DPDF_SMOKE_PASSWORD': 'unlock', 'SLINT_BACKEND': 'qt'}
            with patch.dict(os.environ, original, clear=True):
                env = private.fixture_environment(folder)
                self.assertEqual(dict(os.environ), original)
            self.assertEqual(env['DPDF_CONFIG_DIR'], str(folder / 'test-config'))
            self.assertEqual(env['DPDF_DATA_DIR'], str(folder / 'test-data'))
            self.assertEqual(env['SLINT_BACKEND'], 'winit-software')
            for key in original:
                if key not in ('DPDF_CONFIG_DIR', 'DPDF_DATA_DIR', 'SLINT_BACKEND'):
                    self.assertNotIn(key, env)


if __name__ == '__main__':
    unittest.main()
