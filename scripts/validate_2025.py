#!/usr/bin/env python3
"""Reproducible local compatibility run. Never copies private inputs into source control."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def fingerprint(root):
    files = [root / "Cargo.toml", root / "Cargo.lock"]
    files.extend((root / "src").rglob("*"))
    files.extend((root / "input").rglob("*"))
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in files if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("/Users/dnn/repos/advent-of-code-2025"))
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--profile", default="release")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    source = args.source.resolve()
    before = fingerprint(source)
    destination = root / "target/compatibility-2025"
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source / "src", destination / "src", dirs_exist_ok=True)
    inputs = destination / "input"
    if not inputs.exists():
        inputs.symlink_to(source / "input", target_is_directory=True)

    manifest = (source / "Cargo.toml").read_text()
    manifest = re.sub(r"^(?:aoc-runner(?:-derive)?|aoc-harness)\s*=.*\n", "", manifest, flags=re.M)
    runtime = json.dumps(str(root / "crates/aoc-harness"))
    manifest = manifest.replace("[dependencies]", f"[dependencies]\naoc-harness = {{ path = {runtime} }}")
    (destination / "Cargo.toml").write_text(manifest + "\n[workspace]\n")
    if (source / "Cargo.lock").exists():
        shutil.copy2(source / "Cargo.lock", destination / "Cargo.lock")

    expected = {}
    for path in (destination / "src").rglob("*.rs"):
        text = path.read_text().replace("aoc_runner_derive", "aoc_harness")
        day = re.fullmatch(r"day(\d+)\.rs", path.name)
        if day:
            answers = re.search(r"const ANSWERS:.*?=\s*\((\d+),\s*(\d+)\)", text)
            for part in re.findall(r"#\[aoc\(day\d+, part([12])\)\]", text):
                expected[(int(day[1]), int(part))] = answers[int(part)]
        path.write_text(text)

    day4 = destination / "src/day04.rs"
    text = day4.read_text()
    start = text.index("pub fn part2(")
    end = text.index("#[cfg(test)]", start)
    function = text[start:end]
    function = function.replace("(inputs, r_max, c_max): &(Vec<u8>, usize, usize)", "(mut inputs, r_max, c_max): (Vec<u8>, usize, usize)")
    function = function.replace("    let mut inputs = inputs.to_vec();\n", "")
    function = function.replace("*r_max", "r_max").replace("*c_max", "c_max")
    text = text[:start] + function + text[end:]
    text = text.replace("part2(&generator(SAMPLE))", "part2(generator(SAMPLE))").replace("part2(&output)", "part2(output)")
    day4.write_text(text)
    assert "inputs.to_vec()" not in function

    cli = (args.cli or root / "target/debug/cargo-aoc-harness").resolve()
    common = ["--manifest-path", str(destination / "Cargo.toml"), "--target-dir", str(root / "target"), "--profile", args.profile]
    if args.offline:
        common.append("--offline")
    output = subprocess.check_output([str(cli), "run", "--all-days", *common], text=True)
    (destination / "run.stdout.log").write_text(output)
    for line in output.splitlines():
        if not line or re.match(r"(?:AOC \d+|Day \d+ - Part|\t(?:generator|runner|total):|Full run)", line):
            print(line)
    actual = {(int(day), int(part)): answer for day, part, answer in
              re.findall(r"^Day (\d+) - Part (\d+): (\d+)$", output, flags=re.M)}
    assert actual == expected, f"answer mismatch:\nexpected {expected}\nactual {actual}"

    subprocess.run([str(cli), "bench", "--day", "4", "--part", "2", "--smoke", *common], check=True)
    subprocess.run(["cargo", "test", "--manifest-path", str(destination / "Cargo.toml"), "--target-dir", str(root / "target"), "--profile", args.profile,
                    *(["--offline"] if args.offline else []), "--lib"], check=True)
    assert before == fingerprint(source), "original AoC repository changed"
    report = {"parts": len(actual), "answers": {f"day{d}/part{p}": v for (d, p), v in sorted(actual.items())},
              "source_preserved": True, "day4_consumes_without_copy": True, "profile": args.profile}
    (destination / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Compatibility passed: {len(actual)} registered parts; original source and inputs unchanged.")


if __name__ == "__main__":
    main()
