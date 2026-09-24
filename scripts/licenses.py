#!/usr/bin/env python3
"""Collect the resolved Rust dependency notices and licence files for distribution."""
import json, pathlib, shutil, subprocess
ROOT = pathlib.Path(__file__).resolve().parent.parent

def collect(destination):
    destination.mkdir(parents=True, exist_ok=True)
    host = next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
    metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--filter-platform',host,'--format-version','1'],cwd=ROOT))
    notices = ['# Zvim third-party dependencies', '', 'This inventory includes build and platform dependencies from Cargo.lock.', '']
    for p in sorted(metadata['packages'],key=lambda p:p['name']):
        if p['name'] == 'zvim': continue
        ident = p['name']+'-'+p['version']; folder = pathlib.Path(p['manifest_path']).parent
        notices.append(f"- {ident}: {p.get('license') or 'See included licence file'} — {p.get('repository') or ''}")
        out = destination/ident
        files = [f for f in folder.iterdir() if f.is_file() and f.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT'))]
        if p.get('license_file'):
            file = folder/p['license_file']
            if file.exists() and file not in files: files.append(file)
        if p['name'] == 'zvim-ghostty':
            files.append(folder/'GHOSTTY-LICENSE')
        if p['name'] == 'gpui-libghostty':
            files += list((folder/'vendor/ghostty').rglob('LICENSE*'))
        if files:
            out.mkdir(exist_ok=True)
            for f in files:
                name = str(f.relative_to(folder)).replace('/', '__')
                shutil.copy2(f,out/name)
    (destination/'DEPENDENCIES.md').write_text('\n'.join(notices)+'\n')
    shutil.copy2(ROOT/'packaging/NEOVIM-LICENSE.txt',destination/'NEOVIM-LICENSE.txt')
    shutil.copy2(ROOT/'packaging/GPUI-GHOSTTY-LICENSE.txt',destination/'GPUI-GHOSTTY-LICENSE.txt')

    shutil.copy2(ROOT/'packaging/NEOVIM-LOGO-NOTICE.txt',destination/'NEOVIM-LOGO-NOTICE.txt')
