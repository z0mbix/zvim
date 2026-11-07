"""Packaging regression tests; no native build or downloaded runtime required."""
import importlib.util
import os
import pathlib
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('package', pathlib.Path(__file__).with_name('package.py'))
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class GhosttySourceTests(unittest.TestCase):
    def test_only_the_locked_patched_inventory_matches(self):
        ghostty = package.module('ghostty', package.ROOT / 'scripts/bundle-ghostty.py')
        with tempfile.TemporaryDirectory() as temporary:
            manifest = pathlib.Path(temporary) / 'dependency'
            (manifest / 'upstream').mkdir(parents=True)
            (manifest / 'patches/ghostty').mkdir(parents=True)
            (manifest / 'upstream/files.sha256').write_text(
                f"{'c' * 64}  src/shell-integration/zsh/.zshenv\n"
                f"{'a' * 64}  build.zig\n")
            (manifest / 'patches/ghostty/files.sha256').write_text(
                f"{'b' * 64}  build.zig\n")
            target = pathlib.Path(temporary) / 'target'
            stale = target / 'debug/build/gpui-libghostty-old/out/ghostty-source/ghostty-patched-old'
            stale.mkdir(parents=True)
            with self.assertRaises(SystemExit):
                ghostty.built_source(manifest, target)

            digest = 'a1e4a376c187cc572bbd921448efb566de87ddd30b70968200e9500ac8dbd3ef'
            source = target / 'release/build/gpui-libghostty-new/out/ghostty-source' / f'ghostty-patched-{digest}'
            source.mkdir(parents=True)
            self.assertEqual(ghostty.built_source(manifest, target), source)


class ArchiveTests(unittest.TestCase):
    def test_zip_preserves_contents_with_pre_1980_timestamps(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = pathlib.Path(temporary) / 'zvim-macos-arm64'
            folder = source / 'licenses'
            folder.mkdir(parents=True)
            notice = folder / 'LICENSE.txt'
            notice.write_bytes(b'example licence\n')
            os.utime(notice, (31536000, 31536000))  # 1971, before the ZIP epoch.
            package.zip_directory(source)
            with zipfile.ZipFile(str(source) + '.zip') as archive:
                name = 'zvim-macos-arm64/licenses/LICENSE.txt'
                self.assertEqual(archive.read(name), b'example licence\n')
                self.assertEqual(archive.getinfo(name).date_time[0], 1980)
                self.assertIsNone(archive.testzip())


if __name__ == '__main__':
    unittest.main()
