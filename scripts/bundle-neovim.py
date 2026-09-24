#!/usr/bin/env python3
"""Download and verify the pinned complete Neovim distribution."""
import argparse, hashlib, json, pathlib, platform, shutil, subprocess, tarfile
ROOT = pathlib.Path(__file__).resolve().parent.parent

def host():
    if platform.system() not in ('Darwin', 'Linux'): raise SystemExit('Zvim supports macOS and Linux only.')
    os = {'Darwin': 'macos', 'Linux': 'linux'}[platform.system()]
    arch = 'arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x86_64'
    return f'{os}-{arch}'

def bundle(target):
    names = {'macos-arm64': 'nvim-macos-arm64.tar.gz', 'macos-x86_64': 'nvim-macos-x86_64.tar.gz',
             'linux-x86_64': 'nvim-linux-x86_64.tar.gz'}
    if target not in names: raise SystemExit(f'Unsupported Zvim target: {target}')
    name = names[target]
    spec = json.loads((ROOT / 'packaging/neovim.json').read_text())['assets'][name]
    cache = ROOT / '.cache'; cache.mkdir(exist_ok=True)
    archive = cache / name
    if not archive.exists():
        subprocess.run(['curl', '-fL', '--retry', '3', spec['url'], '-o', str(archive)], check=True)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != spec['sha256']:
        archive.unlink()
        raise SystemExit('Neovim checksum mismatch; archive removed. Retry download.')
    dest = ROOT / 'vendor' / target
    if dest.exists(): shutil.rmtree(dest)
    dest.mkdir(parents=True)
    with tarfile.open(archive) as t:
        for m in t.getmembers():
            if m.name.startswith('/') or '..' in pathlib.PurePosixPath(m.name).parts:
                raise SystemExit('Unsafe archive member')
        if hasattr(tarfile, 'data_filter'): t.extractall(dest, filter='data')
        else: t.extractall(dest)
    unpacked = next(dest.iterdir())
    unpacked.rename(dest / 'neovim')
    (dest / 'neovim' / 'ZVIM_BUNDLE_VERSION').write_text('0.12.5\n')
    print(dest / 'neovim')
    return dest / 'neovim'

if __name__ == '__main__':
    p = argparse.ArgumentParser(); p.add_argument('--target', default=host())
    bundle(p.parse_args().target)
