#!/usr/bin/env python3
"""Install a small launcher on PATH; never copy the app's executable away from its runtime."""
import argparse, os, pathlib, platform, shlex, sys

MARKER = 'Zvim-managed CLI launcher'

def default_app():
    here=pathlib.Path(__file__).resolve()
    for parent in here.parents:
        if parent.suffix == '.app': return parent
    if (here.parent/'zvim').exists(): return here.parent/'zvim'
    if (here.parent/'zvim.exe').exists(): return here.parent/'zvim.exe'
    system={'Darwin':'macos','Linux':'linux','Windows':'windows'}[platform.system()]
    arch='arm64' if platform.machine().lower() in ('arm64','aarch64') else 'x86_64'
    folder=here.parent.parent/'dist'/f'zvim-{system}-{arch}'
    return folder/('Zvim.app' if system=='macos' else 'zvim.exe' if system=='windows' else 'zvim')

def install(app, directory):
    app=app.expanduser().resolve()
    binary=app/'Contents/MacOS/zvim' if app.suffix=='.app' else app
    if not binary.is_file(): raise SystemExit(f'Zvim executable not found: {binary}. Build/package first, or pass --app PATH.')
    directory=directory.expanduser().resolve();directory.mkdir(parents=True,exist_ok=True)
    windows=platform.system()=='Windows';dest=directory/('zvim.cmd' if windows else 'zvim')
    if dest.exists() or dest.is_symlink():
        if dest.is_symlink() or MARKER not in dest.read_text(errors='replace'):
            raise SystemExit(f'Refusing to replace an unrelated existing command: {dest}')
    if windows:
        content=f'@echo off\nrem {MARKER}\n"{binary}" --zvim-launch %*\n'
    else:
        content=f'#!/bin/sh\n# {MARKER}\nexec {shlex.quote(str(binary))} --zvim-launch "$@"\n'
    temporary=dest.with_suffix('.tmp');temporary.write_text(content);temporary.chmod(0o755);temporary.replace(dest)
    print(f'Installed {dest} -> {binary}')
    if directory not in [pathlib.Path(p).expanduser().resolve() for p in os.environ.get('PATH','').split(os.pathsep) if p]:
        print(f'Add {directory} to your PATH to use the zvim command. No shell configuration was changed.')
    return dest

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app',type=pathlib.Path,default=default_app(),help='Zvim.app or the installed zvim executable')
    parser.add_argument('--bin-dir',type=pathlib.Path,default=pathlib.Path.home()/'.local/bin')
    args=parser.parse_args();install(args.app,args.bin_dir)
