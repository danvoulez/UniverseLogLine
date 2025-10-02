#!/usr/bin/env python3
"""Package LogLine bundles into distributable .lll.zip archives."""
from __future__ import annotations

import argparse
import zipfile
from pathlib import Path
from typing import List, Set, Tuple

ROOT = Path(__file__).resolve().parents[1]
MANIFESTS_DIR = ROOT / "manifests"
MODULES_DIR = ROOT / "modules"
EXTRA_PATHS = {
    Path("scripts/instant_run.sh"),
    Path("docs/architecture.mmd"),
    Path("README.md"),
    Path("profiles/staging.env"),
    Path("profiles/prod.env"),
    Path("installer/install.sh"),
}


def parse_manifest(manifest_name: str) -> Tuple[List[str], List[str]]:
    path = MANIFESTS_DIR / f"{manifest_name}.lll"
    if not path.exists():
        raise FileNotFoundError(f"Manifest not found: {path}")

    modules: List[str] = []
    requires: List[str] = []
    state: str | None = None

    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.endswith(":") and not line.startswith("-"):
            key = line[:-1].strip()
            if key in {"modules", "requires"}:
                state = key
            else:
                state = None
            continue
        if state in {"modules", "requires"} and line.startswith("- "):
            value = line[2:].split("#", 1)[0].strip()
            if not value:
                continue
            if state == "modules":
                modules.append(value)
            else:
                requires.append(value)
    return modules, requires


def collect_dependencies(manifest_name: str) -> Tuple[Set[str], Set[str]]:
    visited_manifests: Set[str] = set()
    modules: Set[str] = set()

    def dfs(name: str) -> None:
        if name in visited_manifests:
            return
        visited_manifests.add(name)
        manifest_modules, manifest_requires = parse_manifest(name)
        modules.update(manifest_modules)
        for req in manifest_requires:
            dfs(req)

    dfs(manifest_name)
    return visited_manifests, modules


def build_zip(
    manifest_name: str,
    output: Path,
    include_extra: bool,
    binary_path: Path | None,
    binary_name: str | None,
) -> Path:
    manifests_required, modules_required = collect_dependencies(manifest_name)
    bundle_root = f"{manifest_name}"
    output.parent.mkdir(parents=True, exist_ok=True)

    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for manifest in sorted(manifests_required):
            src = MANIFESTS_DIR / f"{manifest}.lll"
            arcname = f"{bundle_root}/manifests/{src.name}"
            zf.write(src, arcname)
        for module in sorted(modules_required):
            src = MODULES_DIR / f"{module}.lll"
            if not src.exists():
                raise FileNotFoundError(f"Module not found: {src}")
            arcname = f"{bundle_root}/modules/{src.name}"
            zf.write(src, arcname)
        if include_extra:
            for rel_path in sorted(EXTRA_PATHS):
                src = ROOT / rel_path
                if src.exists():
                    arcname = f"{bundle_root}/{rel_path.as_posix()}"
                    zf.write(src, arcname)
        if binary_path:
            if not binary_path.exists():
                raise FileNotFoundError(f"Binary not found: {binary_path}")
            name = binary_name or binary_path.name
            arcname = f"{bundle_root}/bin/{name}"
            zf.write(binary_path, arcname)
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description="Package LogLine bundles into .lll.zip files")
    parser.add_argument("bundle", help="Manifest name without extension (e.g. logline_orchestration_network_v4)")
    parser.add_argument("--output", type=Path, help="Output zip path; defaults to ./dist/<bundle>.lll.zip")
    parser.add_argument("--no-extra", action="store_true", help="Skip auxiliary files (scripts, docs, profiles)")
    parser.add_argument("--binary", type=Path, help="Path to the logline binary to embed under bin/")
    parser.add_argument(
        "--binary-name",
        help="Optional name for the embedded binary (defaults to the source filename)",
    )
    args = parser.parse_args()

    bundle = args.bundle
    output = args.output or (ROOT / "dist" / f"{bundle}.lll.zip")
    include_extra = not args.no_extra
    binary_path = args.binary.resolve() if args.binary else None
    binary_name = args.binary_name

    zip_path = build_zip(bundle, output, include_extra, binary_path, binary_name)
    print(f"Created bundle: {zip_path}")


if __name__ == "__main__":
    main()
