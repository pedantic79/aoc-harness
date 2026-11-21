# Migrating to a multi-year solution crate

This guide consolidates one-crate-per-event repositories into a single solution
crate. It assumes a starting point like this:

```text
advent-of-code-2019/      # aoc_lib! { year = 2019 }, depends on advent-of-code-common
advent-of-code-2025/      # aoc_lib! { year = 2025 }, depends on advent-of-code-common
advent-of-code-common/    # shared parsing and grid utilities
advent-of-code-input/     # private repository of puzzle inputs
```

The result is one crate with a module per year and shared code under
`src/common`:

```text
advent-of-code/
├── Cargo.toml
├── input/                 # the input repository, checked out or as a submodule
│   ├── 2019/day1.txt
│   └── 2025/day1.txt
└── src/
    ├── lib.rs
    ├── common/mod.rs
    ├── y2019/{mod.rs, day01.rs, ...}
    └── y2025/{mod.rs, day01.rs, ...}
```

Your `#[aoc]` and `#[aoc_generator]` annotations don't change. Each year keeps
its own registrations, so 2019 and 2025 can both define `fn part1` for day 1
part 1. The harness doesn't migrate repositories or edit your README; the steps
below are manual.

Multi-year crates need `aoc-harness` 0.5.0 or later and a matching
`aoc-harness-cli`:

```sh
cargo install aoc-harness-cli --locked
```

## 1. Create the crate

```sh
cargo new --lib advent-of-code
cd advent-of-code
```

Add `aoc-harness` and the union of the dependencies used by every year's crate
and by the common crate. Drop the dependency on `advent-of-code-common` itself.

```toml
[dependencies]
aoc-harness = "0.5.1"
```

If you want to keep each repository's history, commit the new crate, then import
each repository, for example with
`git subtree add --prefix=import/2019 ../advent-of-code-2019 main`, and move the
files out of `import/` in the steps below. `git subtree add` needs an existing
commit and a clean working tree.

## 2. Move each year's day modules

For each event, copy its day files into `src/yYYYY/` and move the old library
root's `mod` declarations into that year's `mod.rs`:

```sh
mkdir -p src/y2019
cp ../advent-of-code-2019/src/day*.rs src/y2019/
```

```rust
// src/y2019/mod.rs
mod day01;
mod day02;
```

Leave out the old `aoc_lib! { year = 2019 }`; the new crate root declares every
year once. A year can also be `src/y2019.rs` with its days in `src/y2019/`.
Nested modules within a year, such as `src/y2019/intcode/`, keep working.

Other items in the old library root, such as helper modules, move into the
year's `mod.rs` or into `src/common`. Paths that started at the old crate root
now start one level lower: `crate::intcode` becomes `crate::y2019::intcode`, or
`super::intcode` from a day module.

## 3. Merge shared utilities into `src/common`

Copy the common crate's sources into `src/common`, renaming its `lib.rs` to
`mod.rs`:

```sh
mkdir -p src/common
cp -R ../advent-of-code-common/src/. src/common/
mv src/common/lib.rs src/common/mod.rs
```

Then update the imports. Uses of the old crate name become `crate::common`, and
paths inside the common code that started with `crate::` now start with
`crate::common::`:

```sh
grep -rl 'advent_of_code_common' src | xargs sed -i '' 's/advent_of_code_common::/crate::common::/g'
```

On Linux, use `sed -i` without the empty `''` argument. Review internal
`crate::` paths in `src/common` by hand, since they can't be rewritten blindly.

## 4. Map the years in `src/lib.rs`

```rust
pub mod common;

mod y2019;
mod y2025;

aoc_harness::aoc_lib! {
    years = {
        2019 => y2019,
        2025 => y2025,
    },
    default_year = 2025,
}
```

Keys and `default_year` are integer literals, 2015 or later. Each year maps to a
different, non-nested module, and a registration outside every mapped module is
a compile error that names its module. Without `default_year`, commands in a
crate with several years need `--year`; set it to the event you're working on.

## 5. Keep the input repository at `input/`

Inputs keep the `{year}/day{day}.txt` layout, so the existing input repository
works unchanged when mounted at the project's `input/` directory:

```sh
git submodule add git@github.com:you/advent-of-code-input.git input
```

To keep it elsewhere, such as a checkout beside the project, set `input-dir`
instead. Relative paths start at the solution package's `Cargo.toml`:

```toml
[package.metadata.aoc-harness]
input-dir = "../advent-of-code-input"
```

## 6. Check regression tests

With `aoc-harness` 0.5.1 or later, `input!(day)` reads the year of the module
that calls it, so tests moved into `src/y2019/` keep reading 2019's input
unchanged. Only calls outside every mapped module, such as tests at the crate
root, fail to compile; name the year there:

```rust
let raw = aoc_harness::input!(2019, 1);
```

With `aoc-harness` 0.5.0, `input!(day)` doesn't compile in a crate that
registers several years. Upgrade the runtime, or name the year in each call:

```sh
grep -rl 'input!(' src/y2019 | xargs sed -i '' -E 's/input!\(([0-9]+)\)/input!(2019, \1)/g'
```

On Linux, use `sed -i -E` without the empty `''` argument.

## 7. Check every year

```sh
cargo test
cargo aoc-harness list
cargo aoc-harness run --year 2019 --all-days
cargo aoc-harness run --year 2025 --all-days --all-solutions
```

`list` without `--year` shows every registered year. Commands that run, download,
or generate use `--year`, or `default_year` when it is omitted.

## 8. Keep saved benchmark baselines (optional)

Criterion reports and baselines now live in a per-year directory under the new
package's name. To keep a baseline saved in an old repository, move its reports:

```sh
mkdir -p target/criterion/advent-of-code
mv ../advent-of-code-2019/target/criterion/advent-of-code-2019 \
   target/criterion/advent-of-code/2019
```

Benchmark names are unchanged, so `--year 2019 --baseline NAME` compares against
the moved measurements. Use your old and new `<target-dir>` if they aren't the
default `target/`. Profiles follow the same pattern under
`target/aoc-harness/profiles/advent-of-code/<year>/`, but old recordings don't
need moving to stay readable.

## 9. Restructure the solution README

The harness doesn't read or format your README; this layout is a suggestion that
works well with a helper recipe. Keep the shared setup once, add a year index,
and give each year a collapsible section. Leave the latest year open:

```markdown
# Advent of Code

Rust solutions using [aoc-harness](https://github.com/pedantic79/aoc-harness).
Inputs are in a private repository mounted at `input/`.

## Years

- [2025](#y2025)
- [2019](#y2019)

<a id="y2025"></a>
<details open>
<summary><strong>2025</strong></summary>

| Day | Puzzle | Solution |
| --- | --- | --- |
| 1 | [Secret Entrance](https://adventofcode.com/2025/day/1) | [day01.rs](src/y2025/day01.rs) |
<!-- aoc:2025:insert-before -->

</details>

<a id="y2019"></a>
<details>
<summary><strong>2019</strong></summary>

| Day | Puzzle | Solution |
| --- | --- | --- |
| 1 | [The Tyranny of the Rocket Equation](https://adventofcode.com/2019/day/1) | [day01.rs](src/y2019/day01.rs) |
<!-- aoc:2019:insert-before -->

</details>
```

Copy each old repository's rows into its year's table, keeping the puzzle titles
and updating solution links to `src/yYYYY/`. Each year has its own
`<!-- aoc:YYYY:insert-before -->` marker on the line after its last row, so new
rows land in the right table. Keep the blank lines around the table inside
`<details>`; GitHub needs them to render Markdown there.

### Adapt `just add-day`

Give the recipe a year, generate into that year's module, and insert at that
year's marker:

```just
[positional-arguments]
add-day year day title:
    #!/usr/bin/env bash
    set -euo pipefail
    year="$1" day="$2" title="$3"
    file="$(printf 'day%02d.rs' "$day")"

    cargo aoc-harness generate --year "$year" --day "$day"
    cargo aoc-harness input --year "$year" --day "$day"

    marker="<!-- aoc:$year:insert-before -->"
    grep -qF "$marker" README.md || { echo "README.md has no $marker" >&2; exit 1; }
    row="| $day | [$title](https://adventofcode.com/$year/day/$day) | [$file](src/y$year/$file) |"
    awk -v marker="$marker" -v row="$row" '$0 == marker { print row } { print }' \
        README.md > README.md.tmp
    mv README.md.tmp README.md
```

`just add-day 2025 2 "Gift Shop"` creates `src/y2025/day02.rs`, declares it in
the year's module, downloads the input, and adds the row. `generate` follows the
year module's existing padded or unpadded day naming; adjust `file` if your
modules use `day2.rs`.

### Add a new year

When a new event starts:

1. Create `src/y2026/mod.rs` (it can start empty) and add `mod y2026;` to
   `src/lib.rs`.
2. Add `2026 => y2026` to `years` and move `default_year` to 2026.
3. In the README, add `2026` to the top of the year index, add its `<details
   open>` section above the previous year with an
   `<!-- aoc:2026:insert-before -->` marker, and remove `open` from the previous
   year's section.
4. Run `just add-day 2026 1 "Puzzle Title"`.
