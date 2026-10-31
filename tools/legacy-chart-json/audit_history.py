#!/usr/bin/env python3
"""Archive every deployed pin and run the consumer's unchanged schema auditor."""
from __future__ import annotations
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
LIBRARY = HERE.parents[1]


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def archived(revision):
    root = HERE / '.scratch' / revision
    if not (root / '.git/index').exists():
        root.mkdir(parents=True, exist_ok=True)
        data = subprocess.check_output(['git', '-C', str(LIBRARY), 'archive', revision])
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            archive.extractall(root, filter='data')
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        alternates = root / '.git/objects/info/alternates'
        alternates.write_bytes(((LIBRARY / '.git/objects').as_posix() + '\n').encode())
        subprocess.run(['git', '-C', str(root), 'config', 'core.longpaths', 'true'], check=True)
        subprocess.run(['git', '-C', str(root), 'update-ref', 'HEAD', revision], check=True)
        subprocess.run(['git', '-C', str(root), 'read-tree', revision], check=True)
    subprocess.run(['git', '-C', str(root), 'config', 'core.longpaths', 'true'], check=True)
    return root


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--consumer', type=Path, required=True, help='beyondcell checkout whose workbook/Cargo.toml history names the deployed pins')
    consumer = parser.parse_args().consumer
    auditor = consumer / 'workbook/src/storage/digest/opaque_chart_schema_audit.py'
    destination = HERE / 'schema-history'
    destination.mkdir(exist_ok=True)
    commits = git(consumer, 'log', '--format=%H', '-G', 'umya-spreadsheet.*rev', '--', 'workbook/Cargo.toml').splitlines()
    pins = {}
    for commit in reversed(commits):
        manifest = git(consumer, 'show', f'{commit}:workbook/Cargo.toml')
        dependency = next(line for line in manifest.splitlines() if line.startswith('umya-spreadsheet ='))
        match = re.search(r'rev\s*=\s*"([0-9a-f]+)"', dependency)
        if match:
            revision = git(LIBRARY, 'rev-parse', match[1])
            source = 'manifest rev'
        else:
            lock = git(consumer, 'show', f'{commit}:workbook/Cargo.lock')
            entry = lock.split('name = "umya-spreadsheet"', 1)[1].split('[[package]]', 1)[0]
            revision = re.search(r'source = "git\+[^"#]+#([0-9a-f]{40})"', entry)[1]
            source = 'workbook/Cargo.lock (manifest has no rev)'
        pins.setdefault(revision, []).append({'consumer_commit': commit, 'source': source})
    results = []
    for revision, deployments in pins.items():
        try:
            root = archived(revision)
        except subprocess.CalledProcessError as error:
            results.append({'pin': revision, 'deployments': deployments, 'auditor_exit': None, 'error': str(error)})
            print(revision[:8], str(error), flush=True)
            continue
        manifest = destination / f'{revision}.txt'
        run = subprocess.run(['python', '-I', '-B', str(auditor), '--source-root', str(root / 'src/structs'), '--revision', revision, '--manifest-path', str(manifest), '--write'], text=True, capture_output=True)
        (destination / f'{revision}.audit.log').write_text(run.stdout + run.stderr)
        result = {'pin': revision, 'deployments': deployments, 'auditor_exit': run.returncode}
        if run.returncode == 0:
            lines = manifest.read_text().splitlines()
            declarations = [line for line in lines if not line.startswith('#')]
            # Traverse public serialized owners rooted at Chart, rather than group
            # unrelated workbook-only shapes from the source-superset manifest.
            rows = {}
            for line in declarations:
                name, relative = re.match(r'(?:struct|enum) (\w+) \[([^]]+)\]', line).groups()
                rows[(name, relative)] = line
            by_name = {}
            for key in rows:
                by_name.setdefault(key[0], []).append(key)
            reachable, pending = {}, [('Chart', 'chart.rs')]
            overrides = {('drawing/charts/chart_space.rs', 'Style'): 'office2010/drawing/charts/style.rs', ('drawing/spreadsheet/shape_properties.rs', 'BlipFill'): 'drawing/blip_fill.rs'}
            while pending:
                key = pending.pop()
                if key in reachable:
                    continue
                line = rows[key]
                reachable[key] = line
                body = line.split(']', 1)[1].strip().removeprefix('{').removesuffix('}').strip()
                references = []
                for member in body.split(';'):
                    if line.startswith('struct '):
                        payload = member.split(':', 1)[1] if ':' in member else ''
                    else:
                        # Variant names (e.g. BarDirectionValues::Column) are
                        # values, not references to equally named struct types.
                        payload = member.split('(', 1)[1] if '(' in member else ''
                        if '{' in member:
                            payload = member.split('{', 1)[1]
                    references.extend(re.findall(r'\b[A-Z]\w*\b', payload))
                directory = Path(key[1]).parent
                for name in references:
                    candidates = by_name.get(name, [])
                    if not candidates:
                        continue
                    explicit = overrides.get((key[1], name))
                    if explicit:
                        selected = next(candidate for candidate in candidates if candidate[1] == explicit)
                    else:
                        preferred = []
                        for scope in [directory, *directory.parents]:
                            preferred = [candidate for candidate in candidates if Path(candidate[1]).parent == scope]
                            if preferred:
                                break
                        if not preferred:
                            preferred = candidates
                        if len(preferred) != 1:
                            raise ValueError(f'ambiguous chart type reference {key}: {name}: {preferred}')
                        selected = preferred[0]
                    pending.append(selected)
            normalized = '\n'.join(reachable[key] for key in sorted(reachable)) + '\n'
            digest = hashlib.sha256(normalized.encode()).hexdigest()
            (destination / f'{revision}.chart.txt').write_text(normalized)
            result.update(chart_manifest_sha256=digest, chart_types=len(reachable), superset_manifest=manifest.name)
        else:
            result['error'] = run.stderr.strip()
        results.append(result)
        print(revision[:8], result.get('chart_manifest_sha256', result.get('error')), flush=True)
    groups = {}
    for result in results:
        if 'chart_manifest_sha256' in result:
            groups.setdefault(result['chart_manifest_sha256'], []).append(result['pin'])
    (destination / 'classes.json').write_text(json.dumps({'pins': results, 'classes': [{'sha256': digest, 'pins': group} for digest, group in groups.items()]}, indent=2) + '\n')


if __name__ == '__main__':
    main()
