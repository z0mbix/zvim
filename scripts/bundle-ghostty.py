#!/usr/bin/env python3
"""Stage resources and native notices from the exact Ghostty dependency built by Cargo."""
import json, os, pathlib, platform, re, shutil, subprocess
ROOT = pathlib.Path(__file__).resolve().parent.parent


def bundle():
    host = next(line.split(': ', 1)[1] for line in subprocess.check_output(
        ['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline', '--filter-platform', host,
         '--format-version', '1'], cwd=ROOT))
    package = next(p for p in metadata['packages'] if p['name'] == 'gpui-libghostty')
    source = pathlib.Path(package['manifest_path']).parent / 'vendor/ghostty'
    cache_home = pathlib.Path(os.environ.get('XDG_CACHE_HOME') or
        (pathlib.Path.home() / ('Library/Caches' if platform.system() == 'Darwin' else '.cache')))
    caches = [pathlib.Path(os.environ[k]) for k in
              ('GHOSTTY_ZIG_SYSTEM_PACKAGE_DIR', 'GHOSTTY_ZIG_PACKAGE_CACHE_DIR') if os.environ.get(k)]
    caches += [cache_home / 'gpui-libghostty/zig-pkg',
               pathlib.Path(metadata['target_directory']) / 'ghostty-zig-pkg']
    zon = (source / 'build.zig.zon').read_text()
    theme_hash = re.search(r'\.iterm2_themes\s*=\s*\.\{.*?\.hash\s*=\s*"([^"]+)"', zon, re.S)[1]
    themes = next((cache / theme_hash for cache in caches if (cache / theme_hash).is_dir()), None)
    if themes is None:
        raise SystemExit('Ghostty resources are missing. Run cargo build --locked first, then this script with the same cache environment.')
    dest = pathlib.Path(metadata['target_directory']) / 'ghostty-resources'
    if dest.exists(): shutil.rmtree(dest)
    resources = dest / 'ghostty'
    resources.mkdir(parents=True)
    shutil.copytree(source / 'src/shell-integration', resources / 'shell-integration')
    shutil.copytree(themes, resources / 'themes')
    # The embedding shim advertises xterm-256color. Ghostty sets TERMINFO
    # beside its resources, so ship that entry rather than an empty database.
    terminfo = dest / 'xterm-256color.terminfo'
    terminfo.write_bytes(subprocess.check_output(['infocmp', '-x', 'xterm-256color']))
    subprocess.run(['tic', '-x', '-o', str(dest / 'terminfo'), str(terminfo)], check=True)
    licenses = dest / 'licenses'
    def notices(folder, name):
        for path in folder.rglob('*'):
            if path.is_file() and path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'COPYRIGHT', 'NOTICE', 'OFL')):
                output = licenses / name / path.relative_to(folder)
                output.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, output)
    notices(source, 'ghostty')
    # Include native build dependencies as well as linked libraries and embedded fonts.
    hashes = set()
    for manifest in source.rglob('build.zig.zon'):
        hashes.update(re.findall(r'\.hash\s*=\s*"([^"]+)"', manifest.read_text()))
    seen = set()
    while hashes:
        digest = hashes.pop()
        if digest in seen: continue
        seen.add(digest)
        package_dir = next((cache / digest for cache in caches if (cache / digest).is_dir()), None)
        if package_dir is not None:
            notices(package_dir, digest)
            for manifest in package_dir.rglob('build.zig.zon'):
                hashes.update(re.findall(r'\.hash\s*=\s*"([^"]+)"', manifest.read_text()))
    (licenses / 'SOURCE.txt').write_text('Cargo source: ' + package['source'] + '\n' + (source / 'VENDOR.md').read_text())
    print(resources)
    return dest


if __name__ == '__main__':
    bundle()
