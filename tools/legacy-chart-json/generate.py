#!/usr/bin/env python3
"""Build one historical serde shape class offline through the RAM-gated lane."""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
FIXTURES = REPO / 'tests/fixtures/persisted_charts'
BASH = Path('C:/Program Files/Git/bin/bash.exe')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def contains_member(value, key, predicate=lambda item: item is not None):
    if isinstance(value, dict):
        return (key in value and predicate(value[key])) or any(contains_member(item, key, predicate) for item in value.values())
    if isinstance(value, list):
        return any(contains_member(item, key, predicate) for item in value)
    return False


def validate_coverage(name, chart_file, drawing_file):
    charts = json.loads(chart_file.read_bytes())
    drawing = json.loads(drawing_file.read_bytes())
    if name in ('scheme_transforms', 'rgb_transforms'):
        for field, expected in [('luminance', 21000), ('luminance_modulation', 32000), ('luminance_offset', 43000), ('saturation', 54000), ('saturation_modulation', 65000), ('shade', 76000), ('alpha', 87000), ('tint', 98000)]:
            assert contains_member(charts, field, lambda item: item == {'val': {'value': expected}}), (name, field)
    required = {'preset_alpha': ('preset_color', lambda value: value == {'val': 'red', 'alpha': {'val': '67000'}}), 'shape_auto_fit': ('shape_auto_fit', lambda value: value == {}), 'gradient_stops': ('position', lambda value: value == 37500), 'pattern_colours': ('foreground_color', lambda value: value is not None), 'fill_rectangle': ('fill_rectangle', lambda value: value is not None), 'tile_rectangle': ('tile_rectangle', lambda value: value == {}), 'grouped_drawing': ('group_shape', lambda value: value is not None)}
    if name in required:
        key, predicate = required[name]
        assert contains_member(drawing, key, predicate), (name, key)


def build_environment(inherited):
    environment = inherited.copy()
    count = int(environment.get('GIT_CONFIG_COUNT', '0'))
    if count < 0:
        raise ValueError('GIT_CONFIG_COUNT must not be negative')
    pairs = [
        (f'url.{REPO.as_uri()}.insteadOf', 'https://github.com/vortiq-ai/umya-spreadsheet-serde'),
        (f'url.{REPO.as_uri()}.insteadOf', 'https://github.com/vortiq-ai/umya-spreadsheet-serde.git'),
        ('protocol.file.allow', 'always'),
    ]
    for offset, (key, value) in enumerate(pairs, count):
        environment[f'GIT_CONFIG_KEY_{offset}'] = key
        environment[f'GIT_CONFIG_VALUE_{offset}'] = value
    environment.update(CARGO_NET_OFFLINE='true', CARGO_NET_GIT_FETCH_WITH_CLI='true',
                       GIT_CONFIG_COUNT=str(count + len(pairs)))
    return environment


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('revision')
    parser.add_argument('shape_class')
    parser.add_argument('--scratch-cache', action='store_true', help='Read source objects and registry crates through an isolated scratch Cargo cache; never modify the shared cache.')
    args = parser.parse_args()
    lane = os.environ.get('UMYA_LANE') or parser.error('set UMYA_LANE to the RAM-gated cargo wrapper script')
    revision = subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', '--verify', args.revision + '^{commit}'], text=True).strip()
    scratch = HERE / '.scratch/generators' / revision
    (scratch / 'src').mkdir(parents=True, exist_ok=True)
    source = HERE / 'src/main.rs'
    (scratch / 'src/main.rs').write_bytes(source.read_bytes())
    manifest = (HERE / 'Cargo.toml').read_text()
    import re
    manifest = re.sub(r'rev = "[0-9a-f]{40}"', f'rev = "{revision}"', manifest)
    (scratch / 'Cargo.toml').write_text(manifest)
    environment = build_environment(os.environ)
    cache_drive = None
    if args.scratch_cache:
        cache = HERE / '.scratch/cargo-home'
        database = cache / 'git/db/umya-spreadsheet-serde-9fe1804e6d3f6879'
        database.mkdir(parents=True, exist_ok=True)
        subprocess.run(['git', 'init', '--bare', '-q', str(database)], check=True)
        objects = subprocess.check_output(
            ['git', '-C', str(REPO), 'rev-parse', '--path-format=absolute', '--git-path', 'objects'],
            text=True,
        ).strip()
        (database / 'objects/info/alternates').write_bytes((Path(objects).as_posix() + '\n').encode())
        subprocess.run(['git', '--git-dir=' + str(database), 'config', 'core.longpaths', 'true'], check=True)
        subprocess.run(['git', '--git-dir=' + str(database), 'update-ref', 'HEAD', revision], check=True)
        registry = Path.home() / '.cargo/registry'
        scratch_registry = cache / 'registry'
        scratch_registry.mkdir(exist_ok=True)
        shutil.copytree(registry / 'index', scratch_registry / 'index', dirs_exist_ok=True)
        shutil.copytree(registry / 'cache', scratch_registry / 'cache', dirs_exist_ok=True, copy_function=shutil.copy2)
        (cache / 'config.toml').write_text('')
        cache_drive = next(drive + ':' for drive in 'ZYXW' if not Path(drive + ':/').exists())
        subprocess.run(['subst', cache_drive, str(cache)], check=True)
        environment['CARGO_HOME'] = cache_drive + '/' 
    log = HERE / '.scratch/logs' / f'history-{args.shape_class}-build.log'
    log.parent.mkdir(parents=True, exist_ok=True)
    target = HERE / '.scratch/target'
    try:
        run = subprocess.run([str(BASH), lane, str(scratch), str(log), 'lib-persisted', str(target), '+1.98.0', 'build', '--offline'], env=environment)
    finally:
        if cache_drive is not None:
            subprocess.run(['subst', cache_drive, '/D'], check=True)

    if run.returncode:
        raise SystemExit(run.returncode)
    write_fixtures(revision, args.shape_class, scratch / 'Cargo.toml', target, args.scratch_cache)


def write_fixtures(revision, shape_class, manifest, target, scratch_cache=False):
    source = HERE / 'src/main.rs'
    output = FIXTURES / shape_class
    output.mkdir(parents=True, exist_ok=True)
    cases = []
    for input_file in sorted((FIXTURES / 'inputs').glob('*.xlsx')):
        subprocess.run([str(target / 'debug/legacy-chart-json.exe'), str(input_file), str(output)], check=True)
        name = input_file.stem
        files = {kind: output / (name + suffix) for kind, suffix in [('json', '.json'), ('export', '.xlsx'), ('drawing_json', '.drawing.json')]}
        validate_coverage(name, files['json'], files['drawing_json'])
        cases.append({'name': name, 'input': '../inputs/' + input_file.name, 'input_sha256': sha(input_file), **{kind: path.name for kind, path in files.items()}, 'output_sha256': {kind: sha(path) for kind, path in files.items()}})
    provenance = {'pin': revision, 'shape_class': shape_class, 'generator_source_sha256': sha(source), 'generator_manifest_sha256': sha(manifest), 'generation_command': f'python -I -B tools/legacy-chart-json/generate.py {revision} {shape_class}' + (' --scratch-cache' if scratch_cache else ''), 'cases': cases}
    (output / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')


if __name__ == '__main__':
    main()
