# Historical chart JSON fixtures

## Reproduction

The generator is a standalone Cargo workspace. Its declared library dependency always uses the canonical Git URL and an exact historical pin. `generate.py` copies that manifest and the Rust source into an ignored scratch workspace for each pin, builds through the RAM-gated cargo wrapper named by `UMYA_LANE` with `+1.98.0 build --offline` (log and target under `.scratch`), then runs the resulting executable. No archive checkout or shared repository is modified. Every output is emitted by the old public reader, serde, and writer. `author_inputs.py` authors the minimal OOXML inputs independently of umya.

```text
python -I -B tools/legacy-chart-json/author_inputs.py
python -I -B tools/legacy-chart-json/audit_history.py --consumer <beyondcell checkout>
python -I -B tools/legacy-chart-json/generate.py d80fb35f2ad89e16031e999189240a5ca37976d7 7fd0288662ac --scratch-cache
python -I -B tools/legacy-chart-json/generate.py 5ec08b764dad018fb6bb9c292062595fb44181c1 168fb290c4b8
python -I -B tools/legacy-chart-json/generate.py 4282eaf9c86907bed239fd208ded3349772f6542 d1bbfefa5a7f
python -I -B tools/legacy-chart-json/generate.py 336476e91b94ef97e0133fb4813b56ff620de75b 47ebd1cad36f
python -I -B tools/legacy-chart-json/generate.py 327722cffd3e8e4db073c703bf3474f8820c938f c60b36940803
```

`--scratch-cache` is required for the oldest representative because its exact pin is not in the current Git-URL Cargo cache. It creates an isolated cache under `.scratch`, reads canonical Git objects through alternates, copies already cached registry archives/indexes, and uses a temporary `subst` drive to avoid the Windows Git path limit. The drive mapping is removed in `finally`. Cargo remains offline. It does not patch the library or modify the shared cache.

Each class has 29 cases: a standard chart, all eight legacy colour fields together and individually for scheme/sRGB, the common lumMod/lumOff gridline combination, preset alpha, gradient stops, pattern colours, fill/tile rectangles, three body autofit variants, and a mixed picture/shape group. The generator validates that the changed payloads were actually imported before writing provenance. Old readers ignore normal/no-autofit, so their old exports and JSON intentionally describe the absence of autofit; shape-autofit is populated.

Old charts::ShapeProperties has no gradient or blip fill, and ChartSpace has no text-properties slot. Preset alpha therefore lives in an outer shadow, body autofit in legend text-properties, and gradient/blip fills in independent drawing shapes. Each `.json` is exactly `serde_json::to_vec(&Vec<Chart>)`, as in the consumer. The `.drawing.json` companion is the old imported `WorksheetDrawing` serialized with the same serde; it supplies the grouped and independent shapes that the old reader keeps outside `Vec<Chart>`. The `.xlsx` export comes from the same imported workbook. Provenance stores the pin and SHA-256 of Rust generator source, pin-specific manifest, input, JSON, drawing JSON, and old export.

## Deployed classes

`schema-history/classes.json` records all deployment commits, full pins, source-superset auditor manifests, chart closures, and exit codes. The unchanged consumer auditor ran successfully against clean scratch archives for 17 distinct deployed pins. The chart closure follows field/payload types and resolves same-named owners by module; enum variant names are not type dependencies.

| Class | Representative | Pins |
|---|---|---|
| 7fd0288662ac | `d80fb35f2ad89e16031e999189240a5ca37976d7` | `5c776271e6403ed4a291bb00593cd25e26d12456`, `408f470d74b251fa6814e9c789a50e437eecadbc`, `efe1cd84bf301dc2961474075ff34af6de98bcce`, `d80fb35f2ad89e16031e999189240a5ca37976d7` |
| 168fb290c4b8 | `5ec08b764dad018fb6bb9c292062595fb44181c1` | `40c5ffe8cd69249571a998963176cec7fa957f4c`, `323336c2125194caf3703231357d38df09b6d0e6`, `f35257f3eafdb1ae8c8554c1c078e551de695457`, `051e822971a8376f023c02bab87550eec7a0e017`, `3880d6880df09f77b7d7a3368062d817e26f04be`, `b7f85abf14215e30daa821de1228b0a18c43e367`, `26acdc701552e46fbe25a8d69fbabb45efb3705d`, `8b7295fe63c002053748443f6fde471665f36f53`, `5ec08b764dad018fb6bb9c292062595fb44181c1` |
| d1bbfefa5a7f | `4282eaf9c86907bed239fd208ded3349772f6542` | `3595b5e2c470b15a45a07d10bb56edeec5ee870d`, `4282eaf9c86907bed239fd208ded3349772f6542` |
| 47ebd1cad36f | `336476e91b94ef97e0133fb4813b56ff620de75b` | `336476e91b94ef97e0133fb4813b56ff620de75b` |
| c60b36940803 | `327722cffd3e8e4db073c703bf3474f8820c938f` | `327722cffd3e8e4db073c703bf3474f8820c938f` |

Consumer commit `c840c8491` has no `rev` in `workbook/Cargo.toml`. Its `workbook/Cargo.lock` resolves the old `vortiqs` URL to `d80fb35f2ad89e16031e999189240a5ca37976d7`, already included in the first class.

Pin `a3a2cd8dcb2e19e6d69913e04b37d6a9df71fede` was never deployable and is not a historical shape class. Consumer commit [`f377341ef08c5d2d483e5f904f00dd756e3df346`](https://github.com/vortiq-ai/beyondcell/commit/f377341ef08c5d2d483e5f904f00dd756e3df346) (2026-10-01) pointed at it; [`8d0d75880aaf823e5c5e3eb2017e2703c4bd2eb7`](https://github.com/vortiq-ai/beyondcell/commit/8d0d75880aaf823e5c5e3eb2017e2703c4bd2eb7), the same day, re-pinned to `f35257f3eafdb1ae8c8554c1c078e551de695457` with the message "Pin umya at the revision the org repo actually has, now that it is public". The failed archive attempt remains recorded in `classes.json`; it does not represent a deployed class needing fixtures. No fixture was invented for it.

## Compatibility rules and checks

All 17 audited pins serialize `Blip` as `{image,cstate}`, while every old writer emits `a:extLst/a:ext uri="{28A0092B-C50C-407E-A947-70E740481C1C}"/a14:useLocalDpi val="0"` inside `a:blip`; legacy map JSON therefore restores that extension. Old writers also never emitted `ShapeProperties.gradient_fill` in `xdr:spPr`, so a historical gradient payload (integer stop positions) is discarded, while current string positions and the current binary sequence keep gradients. `GradientStop` accepts an integer or string map position; `drawing::charts::Chart` reads a missing map `legend_present` as `true`.

Other historical differences are additive optional/defaulted chart fields or enum variants: multi-level category references, cached series labels, chart group order, and source-cache metadata. The early Alignment.indent and Column.custom_width changes are unrelated cell owners; they are excluded from the qualified chart closure, not misreported as chart blockers.

`tests/persisted_chart_compatibility.rs` checks every generated case (XML meaning against the old export, provenance hashes, bincode layout); `tests/persisted_*_compatibility.rs` cover the individual colour, gradient, legend, shape-gradient and structure rules. No case is skipped or normalized.
