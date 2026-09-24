"""Build every standard macOS icon representation from the checked-in PNG master."""
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def build(destination):
    source = ROOT / 'assets/icons/neovim-app.png'
    with tempfile.TemporaryDirectory(prefix='zvim-icon-') as temporary:
        iconset = pathlib.Path(temporary) / 'Zvim.iconset'
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                pixels = str(size * scale)
                suffix = '@2x' if scale == 2 else ''
                output = iconset / f'icon_{size}x{size}{suffix}.png'
                subprocess.run(['sips', '-z', pixels, pixels, str(source), '--out', str(output)],
                               check=True, stdout=subprocess.DEVNULL)
        subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o', str(destination)], check=True)
