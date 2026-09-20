"""Freeze actual reference bytes, not just HEAD. Does not touch hardware/source.

Source archive includes core, MCU adapters/examples, analysis scripts, Markdown,
and build metadata. Captures and external dependencies are NOT vendored: this
is a control-source snapshot, not a claim of a complete firmware build closure.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def selected(root):
    paths = set()
    for directory in ("core/src", "src", "examples", "scripts", ".cargo", "notes"):
        paths.update(p for p in (root / directory).rglob("*")
                     if p.is_file() and "__pycache__" not in p.parts
                     and p.suffix not in (".pyc", ".bak"))
    paths.update(root.glob("*.md"))
    for name in ("Cargo.toml", "Cargo.lock", "core/Cargo.toml", "core/Cargo.lock",
                 "build.rs", "memory.x", "rust-toolchain.toml", "rust-toolchain"):
        if (root / name).is_file():
            paths.add(root / name)
    return sorted(paths)


def verify(path):
    with zipfile.ZipFile(path) as archive:
        manifest = json.loads(archive.read("manifest.json"))
        expected = {row["path"] for row in manifest["files"]} | {"manifest.json"}
        if set(archive.namelist()) != expected or len(archive.namelist()) != len(expected):
            raise ValueError("archive membership mismatch")
        for row in manifest["files"]:
            data = archive.read(row["path"])
            if len(data) != row["bytes"] or digest(data) != row["sha256"]:
                raise ValueError(f"hash mismatch: {row['path']}")
    return manifest


def verify_source(path, root):
    """Reject replay against a moving sibling dependency without a new freeze."""
    manifest = verify(path)
    expected = {row["path"][5:]: row for row in manifest["files"]
                if row["path"].startswith("minz/")}
    actual = {p.relative_to(root).as_posix(): p for p in selected(root)}
    if expected.keys() != actual.keys():
        raise ValueError("live reference membership differs from snapshot")
    for name, row in expected.items():
        if digest(actual[name].read_bytes()) != row["sha256"]:
            raise ValueError(f"live reference differs: {name}")
    return manifest


def freeze(root, destination):
    root = root.resolve()
    head = git(root, "rev-parse", "HEAD").decode().strip()
    status = git(root, "status", "--porcelain=v1", "--", ".")
    paths = selected(root)
    payload = {"minz/" + p.relative_to(root).as_posix(): p.read_bytes() for p in paths}
    # Preserve parent workspace context without copying unrelated project files.
    for name in ("Cargo.toml", "Cargo.lock"):
        p = root.parent / name
        if p.is_file():
            payload["workspace/" + name] = p.read_bytes()
    payload["provenance/status.txt"] = status
    payload["provenance/tracked.diff"] = git(root, "diff", "HEAD", "--binary", "--", ".")
    for p in paths:
        if p.read_bytes() != payload["minz/" + p.relative_to(root).as_posix()]:
            raise RuntimeError(f"reference changed during freeze: {p}")
    if paths != selected(root) or head != git(root, "rev-parse", "HEAD").decode().strip():
        raise RuntimeError("reference membership or HEAD changed during freeze")
    if status != git(root, "status", "--porcelain=v1", "--", "."):
        raise RuntimeError("reference status changed during freeze")
    manifest = {"schema": 1, "head": head,
                "scope": "control source and local modifications; captures/external dependencies excluded",
                "files": [{"path": name, "bytes": len(data), "sha256": digest(data)}
                          for name, data in sorted(payload.items())]}
    # Exclusive creation: a previous reference can never be silently replaced.
    destination.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(destination, "x", zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(payload.items()):
            archive.writestr(name, data)
        archive.writestr("manifest.json", json.dumps(manifest, indent=2) + "\n")
    return verify(destination)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[2] / "minz")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--check-source", action="store_true")
    args = parser.parse_args()
    result = (verify_source(args.archive, args.source) if args.check_source else
              verify(args.archive) if args.verify else freeze(args.source, args.archive))
    print(json.dumps({"archive": str(args.archive), "sha256": digest(args.archive.read_bytes()),
                      "head": result["head"], "files": len(result["files"])}))
