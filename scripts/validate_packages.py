#!/usr/bin/env python3
"""Validate package archives and an installed CLI using only unpacked crate sources.

Does not publish anything. The temporary patch registry stands in for the three
coordinated releases while all third-party dependencies resolve normally.
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
    for name in ["aoc-harness-macros", "aoc-harness", "aoc-harness-cli"]:
        command(["cargo", "package", "--package", name, "--allow-dirty", "--no-verify", "--offline", "--config", str(patches)])
        archive = target / f"package/{name}-0.1.1.crate"
        with tarfile.open(archive) as package:
            package.extractall(destination, filter="data")
        source = destination / f"{name}-0.1.1"
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
    (consumer / "Cargo.toml").write_text('''[package]
name = "packaged-winter"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
aoc-harness = "0.1.1"
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

    (destination / "validation.json").write_text(json.dumps({"crates": list(unpacked), "installed_cli": str(cli), "status": "passed"}, indent=2) + "\n")
    print(f"Package validation passed: {destination}")


if __name__ == "__main__":
    main()
