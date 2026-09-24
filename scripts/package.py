#!/usr/bin/env python3
"""Package an already-built native binary with the verified Neovim runtime."""
import argparse, importlib.util, pathlib, platform, plistlib, shutil, subprocess, zipfile
ROOT = pathlib.Path(__file__).resolve().parent.parent

def module(name, path):
    spec=importlib.util.spec_from_file_location(name,path);mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod);return mod

def zip_directory(source):
    # Some dependency licence files have Unix-epoch timestamps; ZIP starts at 1980.
    with zipfile.ZipFile(str(source)+'.zip', 'w', compression=zipfile.ZIP_DEFLATED,
                         strict_timestamps=False) as archive:
        for path in sorted(source.rglob('*')):
            archive.write(path, path.relative_to(source.parent))

def main():
    version=module('version',ROOT/'scripts/version.py').version()
    bundle=module('bundle',ROOT/'scripts/bundle-neovim.py')
    licenses=module('licenses',ROOT/'scripts/licenses.py')
    parser=argparse.ArgumentParser();parser.add_argument('--target',default=bundle.host());parser.add_argument('--debug',action='store_true');args=parser.parse_args()
    if args.target != bundle.host(): raise SystemExit('Run packaging on the matching native OS/architecture.')
    ghostty=module("ghostty_resources", ROOT/"scripts/bundle-ghostty.py").bundle()
    runtime=bundle.bundle(args.target)
    binary=ROOT/'target'/('debug' if args.debug else 'release')/'zvim'
    if not binary.exists(): raise SystemExit('Build first: cargo build --release --locked')
    dist=ROOT/'dist';dist.mkdir(exist_ok=True)
    dest=dist/('zvim-'+args.target)
    if dest.exists():shutil.rmtree(dest)
    dest.mkdir()
    if platform.system()=='Darwin':
        app=dest/'Zvim.app';contents=app/'Contents';exe_dir=contents/'MacOS';resources=contents/'Resources'
        exe_dir.mkdir(parents=True);resources.mkdir();shutil.copy2(binary,exe_dir/'zvim')
        module('macos_icon',ROOT/'scripts/macos-icon.py').build(resources/'Zvim.icns')
        plist={'CFBundleName':'Zvim','CFBundleDisplayName':'Zvim','CFBundleIdentifier':'dev.zvim.Zvim','CFBundleVersion':version.split('-')[0].split('+')[0],'CFBundleShortVersionString':version.split('-')[0].split('+')[0],'CFBundleExecutable':'zvim','CFBundlePackageType':'APPL','CFBundleIconFile':'Zvim.icns','NSHighResolutionCapable':True,'LSMinimumSystemVersion':'13.0','CFBundleDocumentTypes':[{'CFBundleTypeName':'Text document','CFBundleTypeRole':'Editor','LSHandlerRank':'Alternate','LSItemContentTypes':['public.text','public.source-code']} ]}
        with (contents/'Info.plist').open('wb') as f:plistlib.dump(plist,f)
        shutil.copytree(runtime,resources/'neovim');licenses.collect(resources/'licenses');shutil.copy2(ROOT/'LICENSE',resources/'LICENSE');shutil.copy2(ROOT/'scripts/install-cli.py',resources/'install-cli.py')
        shutil.copytree(ghostty/'ghostty', resources/'ghostty')
        shutil.copytree(ghostty/'terminfo', resources/'terminfo')
        shutil.copytree(ghostty/'licenses', resources/'licenses/ghostty-native')
        # Ad-hoc signing enables local execution, not public Gatekeeper trust.
        subprocess.run(['codesign','--force','--deep','--sign','-',str(app)],check=True)
        archive=dist/(dest.name+'.zip');subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(app),str(archive)],check=True)
    else:
        shutil.copy2(binary,dest/binary.name);shutil.copytree(runtime,dest/'neovim');licenses.collect(dest/'licenses');shutil.copy2(ROOT/'LICENSE',dest/'LICENSE')
        shutil.copy2(ROOT/'README.md',dest/'README.md');shutil.copy2(ROOT/'scripts/install-cli.py',dest/'install-cli.py')
        shutil.copytree(ghostty/'ghostty', dest/'share/ghostty')
        shutil.copytree(ghostty/'terminfo', dest/'share/terminfo')
        shutil.copytree(ghostty/'licenses', dest/'licenses/ghostty-native')
        shutil.copy2(ROOT/'packaging/zvim.desktop',dest/'zvim.desktop')
        shutil.make_archive(str(dest),'gztar',dist,dest.name)
    print(dest)
if __name__=='__main__':main()
