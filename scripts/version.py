#!/usr/bin/env python3
"""Read the package version and optionally validate a matching SemVer release tag."""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
IDENTIFIER = r'(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)'
SEMVER = re.compile(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)'
                    rf'(?:-({IDENTIFIER}(?:\.{IDENTIFIER})*))?'
                    r'(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


def version():
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked', '--offline'], cwd=ROOT))
    return next(p['version'] for p in metadata['packages'] if p['name'] == 'zvim')


def validate(value, tag):
    if not SEMVER.fullmatch(value) or tag != 'v' + value:
        raise ValueError(f'Tag {tag!r} must equal v{value} and use valid SemVer')


if __name__ == '__main__':
    value = version()
    if len(sys.argv) > 1:
        validate(value, sys.argv[1])
    print(value)
