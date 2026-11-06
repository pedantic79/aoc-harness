#!/usr/bin/env python3
"""Validate package archives and an installed CLI using only unpacked crate sources.

Does not publish anything. Temporary patches resolve first-party dependencies
to unpacked packages, using each crate's version from Cargo metadata.
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile


def main():
    root = Path(__file__).resolve().parents[1]
    destination = root / "target/package-validation"
    destination.mkdir(parents=True, exist_ok=True)
    target = root / "target"
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE="true")
    names = ["aoc-harness-macros", "aoc-harness", "aoc-harness-cli"]
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
        cwd=root, env=environment, text=True,
    ))
    versions = {package["name"]: package["version"] for package in metadata["packages"]
                if package["name"] in names}

    # Package an isolated workspace so temporary patches cannot change the checkout's lockfile.
    packaging = destination / "source"
    if packaging.exists():
        shutil.rmtree(packaging)
    packaging.mkdir()
    for name in ["Cargo.toml", "Cargo.lock", "LICENSE-MIT", "LICENSE-APACHE"]:
        shutil.copy2(root / name, packaging / name)
    shutil.copytree(root / "crates", packaging / "crates", ignore=shutil.ignore_patterns("target"))

    patches = destination / "patches.toml"
    unpacked = {}

    def command(args, cwd=root):
        subprocess.run(args, cwd=cwd, env=environment, check=True)

    def write_patches():
        text = "[patch.crates-io]\n"
        for name, path in unpacked.items():
            text += f"{name} = {{ path = {json.dumps(str(path))} }}\n"
        patches.write_text(text)
        members = [path.name for path in unpacked.values()]
        (destination / "Cargo.toml").write_text("[workspace]\nresolver = '2'\nmembers = " + json.dumps(members) + "\nexclude = ['consumer']\n")

    write_patches()
    for name in names:
        version = versions[name]
        source = destination / f"{name}-{version}"
        if source.exists():
            shutil.rmtree(source)

        command(["cargo", "package", "--package", name, "--allow-dirty", "--no-verify", "--offline", "--config", str(patches)], cwd=packaging)
        archive = target / f"package/{name}-{version}.crate"
        with tarfile.open(archive) as package:
            package.extractall(destination, filter="data")
        unpacked[name] = source
        write_patches()

        assert (source / "LICENSE-MIT").is_file()
        assert (source / "LICENSE-APACHE").is_file()
        assert (source / "README.md").is_file()
        assert str(root) not in (source / "Cargo.toml").read_text()
        command(["cargo", "check", "--manifest-path", str(source / "Cargo.toml"), "--offline", "--config", str(patches)])

    installed = destination / "installed"
    command(["cargo", "install", "--path", str(unpacked["aoc-harness-cli"]), "--root", str(installed),
             "--force", "--offline", "--debug", "--config", str(patches)])
    cli = installed / "bin/cargo-aoc-harness"
    outside = destination / "outside"
    outside.mkdir(exist_ok=True)
    for args in [["--version"], ["--help"], ["aoc-harness", "generate", "--help"]]:
        command([str(cli), *args], cwd=outside)

    consumer = destination / "consumer"
    if consumer.exists():
        shutil.rmtree(consumer)
    (consumer / "src/bin").mkdir(parents=True)
    (consumer / ".cargo").mkdir()
    (consumer / ".cargo/config.toml").write_text(patches.read_text())
    (consumer / "Cargo.toml").write_text(f'''[package]
name = "packaged-winter"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
aoc-harness = "{versions['aoc-harness']}"
''')
    (consumer / "src/lib.rs").write_text('''#[aoc_harness::aoc(day1, part1)]
fn solve(raw: &str) -> Option<usize> { Some(raw.len()) }
aoc_harness::aoc_lib! { year = 2025 }
''')
    (consumer / "src/main.rs").write_text("aoc_harness::aoc_main! { lib = packaged_winter }\n")
    (consumer / "src/bin/standalone.rs").write_text('''#[aoc_harness::aoc(day1, part1)]
fn solve(raw: &str) -> usize { raw.len() }
aoc_harness::aoc_main! { year = 2025 }
''')
    (consumer / "input/2025").mkdir(parents=True)
    (consumer / "input/2025/day1.txt").write_text("synthetic\n")
    command([str(cli), "run", "--profile", "dev", "--offline"], cwd=consumer)
    command([str(cli), "aoc-harness", "generate", "--day", "2", "--dry-run"], cwd=consumer)
    command([str(cli), "generate", "--day", "2"], cwd=consumer)
    command(["cargo", "check", "--all-targets", "--offline"], cwd=consumer)
    for binary in ["packaged-winter", "standalone"]:
        command(["cargo", "run", "--quiet", "--offline", "--bin", binary, "--", "run", "--day", "1"], cwd=consumer)

    (destination / "validation.json").write_text(json.dumps({"crates": versions, "installed_cli": str(cli), "status": "passed"}, indent=2) + "\n")
    print(f"Package validation passed: {destination}")


if __name__ == "__main__":
    main()
