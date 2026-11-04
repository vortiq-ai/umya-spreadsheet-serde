"""Load the retained model-operation outputs without rewriting their packages."""

import argparse
import json
from pathlib import Path
import warnings

import openpyxl


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("log", type=Path)
    args = parser.parse_args()
    results = []
    paths = sorted(args.directory.rglob("*.xlsx"))
    if not paths:
        raise SystemExit("No generated workbooks found")
    for path in paths:
        try:
            with warnings.catch_warnings(record=True) as notices:
                warnings.simplefilter("always")
                book = openpyxl.load_workbook(path)
                try:
                    sheets = book.sheetnames
                finally:
                    book.close()
            results.append({"path": str(path), "loaded": True, "sheets": sheets,
                            "warnings": [str(notice.message) for notice in notices]})
        except Exception as error:
            results.append({"path": str(path), "loaded": False, "error": str(error)})
    failures = sum(not result["loaded"] for result in results)
    args.log.write_text(json.dumps({"openpyxl": openpyxl.__version__,
                                   "failures": failures, "results": results}, indent=2),
                        encoding="utf-8")
    print(f"openpyxl {openpyxl.__version__}: {len(results)} workbooks, {failures} failures")
    raise SystemExit(bool(failures))


if __name__ == "__main__":
    main()
