"""Protect cask publishing against invalid releases and corrupt downloads."""
import hashlib
import tempfile
import unittest
from pathlib import Path

from homebrew_cask import homebrew_cask, stable_version, validate_release


class HomebrewCaskTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.tag = "v1.2.3"
        self.archive = self.directory / f"zvim-{self.tag}-macos-arm64.zip"
        self.archive.write_bytes(b"published archive")
        self.digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.manifest = self.directory / "SHA256SUMS.txt"
        self.manifest.write_text(f"{self.digest}  {self.archive.name}\n")
        self.release = {"tagName": self.tag, "isDraft": False, "isPrerelease": False}

    def test_cask_uses_verified_archive_and_supported_platform(self):
        cask = homebrew_cask(self.tag, self.directory)
        self.assertIn(f'sha256 "{self.digest}"', cask)
        self.assertIn('version "1.2.3"', cask)
        self.assertIn('depends_on arch: :arm64', cask)
        self.assertIn('depends_on macos: :ventura', cask)
        self.assertIn('app "Zvim.app"', cask)
        resolved_cask = cask.replace("#{version}", "1.2.3")
        self.assertIn(f"/v1.2.3/{self.archive.name}", resolved_cask)

    def test_invalid_and_prerelease_tags_are_rejected(self):
        for tag in ("1.2.3", "v1.02.3", "v1.2.3-rc.1", 'v1.2.3"; system("bad")'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                stable_version(tag)

    def test_build_metadata_is_preserved(self):
        tag = "v1.2.3+build.test-1"
        self.archive.rename(self.directory / f"zvim-{tag}-macos-arm64.zip")
        self.manifest.write_text(f"{self.digest}  zvim-{tag}-macos-arm64.zip\n")
        cask = homebrew_cask(tag, self.directory)
        self.assertIn('version "1.2.3+build.test-1"', cask)

    def test_corrupt_archive_is_rejected(self):
        self.archive.write_bytes(b"corrupt download")
        with self.assertRaisesRegex(ValueError, "mismatched checksum"):
            homebrew_cask(self.tag, self.directory)

    def test_missing_archive_is_rejected(self):
        self.archive.unlink()
        with self.assertRaises(FileNotFoundError):
            homebrew_cask(self.tag, self.directory)

    def test_missing_or_duplicate_checksum_is_rejected(self):
        manifest = self.manifest.read_text()
        self.manifest.write_text("")
        with self.assertRaisesRegex(ValueError, "Missing or mismatched checksum"):
            homebrew_cask(self.tag, self.directory)
        self.manifest.write_text(manifest + manifest)
        with self.assertRaisesRegex(ValueError, "Duplicate checksum"):
            homebrew_cask(self.tag, self.directory)

    def test_latest_published_stable_release_is_accepted(self):
        validate_release(self.tag, self.release, self.tag)

    def test_draft_prerelease_and_wrong_tag_are_rejected(self):
        for changes in ({"isDraft": True}, {"isPrerelease": True}, {"tagName": "v1.2.4"}):
            with self.subTest(changes=changes), self.assertRaisesRegex(ValueError, "published stable"):
                validate_release(self.tag, self.release | changes, self.tag)

    def test_older_release_cannot_replace_latest(self):
        with self.assertRaisesRegex(ValueError, "older release"):
            validate_release(self.tag, self.release, "v1.2.4")


if __name__ == "__main__":
    unittest.main()
