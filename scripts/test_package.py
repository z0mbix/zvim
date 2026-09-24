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
