# cargo-aoc feature comparison and parity design

Status: implemented on 2026-11-04. G1–G7 and the G8 release preparation are
implemented; publication remains a separate release operation. The feature
comparison below retains the reviewed baseline, with its assessment updated to
record the implementation. The G1–G8 sections specify the implemented contracts.
See [the README](../../README.md) for current commands and migration guidance,
and [release preparation](../releasing.md) for archive validation.

Reviewed on 2026-11-03 against:

- `gobanos/cargo-aoc`, default branch `v0.3`, commit
  [`c260ff45120bbfcbed2983b9d220919a1d98efd1`][up-commit]. Its CLI manifest
  [declares version `0.3.8`][up-manifest]. This is a source comparison, not a
  claim that this commit matches the latest published crate or GitHub release.
- The current `aoc-harness` working tree, based on commit
  `9c124a0e015c7cd4c25936abe612949ed212d97c`, including existing uncommitted
  changes. Local source links below describe that working tree.

Session design revised on 2026-11-04 to share the user-managed
`$HOME/.adventofcode.session` file with `scarvalhojr/aoc-cli`. Its
[session documentation][session-readme] and [loader][session-source] were
reviewed at commit `ebb1069b2946320c76ed676cf26f0a287d4f64fd`.

The review covers cargo-aoc's CLI, downloader, credential manager, project
manager, generated runner/benchmark/day templates, and the accompanying
`aoc-runner`, `aoc-runner-derive`, and `aoc-runner-internal` crates. Runtime and
macro features matter because they determine which solution libraries the CLI
can execute. Conclusions come from source inspection; upstream programs were
not executed and no real session cookie or personal input was accessed.

## Findings

`aoc-harness` already provides the main annotated-library, execution, timing,
selection, and Criterion workflows. At review, the largest missing workflow was acquiring
inputs and starting a new day. Smaller gaps were `Option` return handling,
custom `Borrow` input projections, user-written executable entry points,
opening benchmark reports, and a convenient profiling switch. Distribution
through crates.io is also unfinished.

Implement these as extensions to the existing three crates. Keep typed
ownership-aware adapters, fresh generation, compile-time registration checks,
and the consumer's runtime version. HTTP, credentials, source scaffolding, and
browser launching belong in the CLI.

### Feature inventory (review baseline and implemented action)

**Present** means the user can accomplish the workflow today, even where the
interface differs. **Partial** means a related capability exists but needs an
extension. **Missing** means no equivalent is implemented. References identify
the code that implements each upstream feature and its local counterpart.

| Feature in cargo-aoc and its companion crates | aoc-harness at review | Implemented action |
| --- | --- | --- |
| Install the CLI and runtime/macro crates from crates.io. [Upstream setup][up-readme] | The README documents installation and dependencies from a checkout. [Local setup][h-readme] | Prepared: package metadata, licenses, archives, and isolated installed-consumer validation (G8); actual publication is separate. |
| Invoke as `cargo aoc` or `cargo-aoc`; omitted command runs solutions. [Argument normalization][up-args], [CLI][up-cli] | `cargo aoc-harness` and `cargo-aoc-harness`; omitted command runs. [CLI][h-cli] | Present. Retain this project's name. |
| Clap help, argument validation, and version reporting. [CLI][up-cli], [day/part parsers][up-internal] | Command-specific help and numeric day/part validation; no configured version output. [Help][h-help], [options][h-runtime] | Implemented: `--version` / `-V` (G7). |
| Annotate solvers with day, part, and optional variant name; display answers implementing `Display`. [Macros][up-macros], [adapters][up-runner] | Same annotation vocabulary, named variants, and displayed answers. [Macros][h-macros] | Present. |
| Raw `&str` or `&[u8]` solver input without a parser. [Adapters][up-runner], [raw storage][up-runtime] | Both supported. [Macros][h-macros] | Present. Both projects load UTF-8 text; byte arguments do not imply arbitrary binary files. |
| Day-, part-, and named-variant generators with `&str` / `&[u8]` input. [Generator registration][up-generator], [solver fallback][up-runner] | All three scopes, with deterministic variant > part > day precedence. [Macros][h-macros] | Present. Upstream registration is order-dependent; preserve the local rules. |
| Adapt generated input through `Borrow`, including custom implementations. [Adapters][up-runner] | Ordinary borrowing and coercions, including vector-to-slice and deref coercions; no explicit custom `Borrow` projection. [Macros][h-macros] | Implemented: explicit `input = "borrow"` projections (G4). |
| Treat `Result<T, E>` generator/solver returns as fallible. [Adapters][up-runner], [type recognition][up-utils] | Supported, with phase context, nonzero failure, and partial timing output. [Macros][h-macros], [runtime][h-runtime] | Present. Both recognize the final path segment `Result`, rather than resolving arbitrary aliases. |
| Treat `Option<T>` generator/solver returns as fallible; `None` is an error. [Adapters][up-runner], [type recognition][up-utils] | Solver answers must implement `Display`; `Option` is not unwrapped. Generator `Option` is currently an ordinary parsed value. [Macros][h-macros] | Implemented: G3, including `output = "value"` migration. |
| Keep annotated functions directly callable by unit tests. [Macros][up-runner] | Supported. [Fixture][h-fixture] | Present. |
| Declare a solution library with `aoc_lib! { year = ... }`. [Output macros][up-out] | Supported; compiled code exports a registry and runner entry point. [Macros][h-macros] | Present. |
| Supply a normal executable using `aoc_main! { lib = ... }`, or define a standalone executable using `aoc_main! { year = ... }`. [Output macros][up-out] | CLI-generated executables only; no public `aoc_main!`. [Macros][h-macros], [CLI][h-cli] | Implemented: both `aoc_main!` forms (G5); Cargo CLI discovery remains library-only. |
| Generate runner and benchmark crates automatically; obtain registrations after checking the consumer. [Project manager][up-project], [execution][up-app] | Generated bridge under the selected target directory, Cargo freshness checks, and compiled registry. [CLI][h-cli] | Present. Keep local metadata and bridge isolation instead of upstream's `target/aoc/completed.json` interchange. |
| Default to the latest implemented day; filter a day and optionally a part; run all variants in that scope. [CLI][up-cli], [execution][up-app] | Latest-day default; day/part/name filters; multiple-day and all-day scopes. Normal runs choose defaults; `--all-solutions` runs variants. [Selection][h-runtime] | Present with intentional default differences. |
| Numeric and `dayN` / `partP` selector spellings; `-d`, `-p`, and `-i` shortcuts. [Parsers][up-internal], [CLI][up-cli] | Numeric long selectors; `-p` already means Cargo package. [Help][h-help], [CLI][h-cli] | Implemented: G7 aliases/spellings; `-p` remains package. |
| Use an alternate input file. [CLI][up-cli], [input generation][up-app] | `--input` for one selected day; relative to the invocation directory. [Input loading][h-runtime] | Present. Upstream still attempts its normal download even with an alternate input; do not reproduce that side effect. |
| Standard `input/YEAR/dayN.txt` files; remove trailing newlines. [Dates][up-date], [raw storage][up-runtime] | Same layout and trimming, plus `dayNN.txt` fallback. Paths are relative to the selected consumer manifest. [Input loading][h-runtime] | Present, with stronger package/path handling. |
| Download an authenticated day's input explicitly. [Downloader][up-app] | Inputs must exist locally. [Input loading][h-runtime] | Implemented: G1. |
| Default explicit input downloads to today's date in EST; override day and year. [Dates][up-date] | No download/date-selection command. | Implemented: G1 with fixed-EST date validation. |
| Download an entire year with `input --all --year`; skip existing files. [Downloader][up-app] | No bulk downloads. | Implemented: G1 with year-specific calendars and bounded sequential requests. |
| Fetch missing input before running or benchmarking. [Execution][up-app] | Missing input causes a local-file error. [Input loading][h-runtime] | Implemented: G1 opt-in `--download`, preserving local-only defaults. |
| Set/read a persistent session token; migrate the former executable-adjacent credentials file. [Credentials][up-credentials], [command][up-app] | No session loading. | Implemented: shared home session file and aoc-cli-compatible overrides (G1); token management stays with the user. |
| Generate a day's parser, both solvers, and example test stubs; insert its module declaration; skip existing files/modules. Available through `input --generate` and default run `--generate`. [Scaffolding][up-app], [template][up-day] | Consumer source is entirely handwritten. | Implemented: shared preflight/dry-run/apply scaffolder (G2). |
| Print answers and separate generation/solver durations; report which phase failed. [Runner template][up-run-template] | Supported, plus generator/solver totals, elapsed full-run time, and unsuccessful partial reports. [Runtime][h-runtime] | Present. Upstream's generated runner prints per-solution errors and continues; retain local failure semantics. |
| Automatic Criterion benchmarks for every variant; optional generator benchmarks through `bench --generator`. [Execution][up-app], [templates][up-bench-template] | Generator, solver, end-to-end, and full-run families are automatic; regex filters and measurement controls exist. [Runtime][h-runtime], [macros][h-macros] | Present and broader. No additional generator flag is needed. |
| Criterion reports, samples, plots, and comparison groups. [Benchmark templates][up-bench-template] | HTML and machine-readable Criterion artifacts in the package-specific report directory. [Runtime][h-runtime] | Present. Benchmark labels/timing boundaries differ; historical estimates are not interchangeable. |
| Open the generated benchmark report with `bench --open` / `-o`. [Execution][up-app] | Prints the report directory; opening is manual. [Runtime][h-runtime] | Implemented: successful report opening (G6). |
| Optimized runs/benchmarks and optional debug symbols via boolean `--profile`. [Execution][up-app] | Release default; selectable Cargo profiles; copied workspace profiles and `--config` can enable symbols. [CLI][h-cli] | Implemented: G6 `--debug-info`, preserving `--profile NAME`. |
| Respect a custom library source path when inspecting/scaffolding a package. [Project manager][up-project], [scaffolding][up-app] | Custom library names and paths work for execution; scaffolding is absent. [CLI][h-cli], [macros][h-macros] | Implemented for execution and G2 scaffolding. |

These are the complete command families in the inspected CLI: default run,
`bench`, `credentials`, and `input`. In particular, the source does **not**
implement answer submission, puzzle-statement/example downloading, a watch
mode, leaderboard integration, automatic fastest-variant selection, or the
community comparison service mentioned as future work in its README.
They are not parity requirements. [CLI declarations][up-cli], [README][up-readme]

Some upstream side effects are visible only in the implementation: its bulk
input branch attempts 25 days concurrently, updates module declarations even
without `--generate`, and returns success after reporting individual task
failures. Its credentials command also prints the stored token. These are
behaviors to account for during migration, not requirements to reproduce.
The implemented commands make source generation explicit and return nonzero for
failed acquisitions. [Input and credential execution][up-app]

### Capabilities to retain and compatibility boundaries

The local harness already adds ownership by value or mutable reference, parsed
values borrowing raw input, fresh-input benchmark control, explicit solution
defaults, a list command, multiple/all-day selection, full-run measurements,
benchmark smoke mode, and measurement/output controls. It also handles Cargo
workspace selection, features, renamed dependencies, configuration, target
directories, profiles, and lockfile preservation more extensively than the
inspected upstream CLI. These are constraints on the implementation, not work
to replace. [Local runtime][h-runtime], [macros][h-macros], [CLI][h-cli]

Parity means equivalent user workflows, not binary/source compatibility with
`aoc_runner::Runner`, generated `Factory` traits, or `completed.json`. Keep
the existing runtime's error and ownership contracts. A migration changes
imports and can require explicit default/input-projection annotations.

There is a separate registration-discovery limitation: local `aoc_lib!` scans
direct annotations in source modules. It cannot discover renamed attribute
imports, annotations introduced through `cfg_attr`, `include!` files, or
macro-generated registrations. Upstream attributes execute during ordinary
Rust macro expansion, without this source scan. Its implementation instead
uses shared proc-macro state and requires generators before solvers and the
entry declaration last. The legacy RLS bypass is also upstream-specific.
These mechanisms are not new CLI features; this proposal retains the local
direct-annotation boundary and documents it as a remaining source-compatibility
limit. Supporting arbitrary macro-generated registrations would require a
separate registry redesign. [Upstream macros][up-macros], [state][up-map],
[local scanner][h-macros]

## Implemented command surface

The following commands are available in this checkout. Existing run/list/bench
behavior remains valid.

```sh
# Inspect the session source without printing its value.
# The default file is $HOME/.adventofcode.session, shared with aoc-cli.
cargo aoc-harness credentials status

# Explicit input acquisition; a selected library need not compile.
cargo aoc-harness input --year 2025 --day 4
cargo aoc-harness input --year 2025 --days 1,4
cargo aoc-harness input --year 2025 --all
cargo aoc-harness input --year 2025 --day 4 --generate
cargo aoc-harness input --year 2025 --day 4 --session-file /path/to/session

# Starting a day does not require credentials or network access.
cargo aoc-harness generate --day 4
cargo aoc-harness generate --days 4,5 --dry-run
cargo aoc-harness run --day 4 --generate

# Fetch missing default inputs before executing the selected scope.
cargo aoc-harness run --all-days --download
cargo aoc-harness bench --day 4 --download --open
cargo aoc-harness bench --day 4 --debug-info
cargo aoc-harness --version
```

Retain `--manifest-path`, `--package`, and applicable Cargo discovery options
for `input` and `generate`. `credentials status` works outside a Cargo project.
Expose `--generate` on `input` and `run` as composition of the same generation
operation, rather than separate template implementations. On `run`, require
an explicit day selection for generation so it can happen before compilation.
Generation may leave deliberately unimplemented solvers, so a subsequent run
can fail with their clear not-implemented error.

`input --all` conflicts with `--day`; it requires an explicit `--year`.
Multiple/repeated days are normalized once and processed in ascending order.
`run` and `bench` derive their year and selected days from the compiled
registry, never from today's calendar. They do not accept a year override.
`input` requires a year or resolves it from the current EST date, without
parsing `aoc_lib!` text to guess a year. With no explicit day, it selects today's
day only when the current date falls within the selected event's December
release window, including the same year; otherwise it asks for `--day`.
Thus `input --year 2025` outside that window needs an explicit day.

Downloads occur only for `input` or an explicit `--download`. This provides
the upstream fetch-and-run workflow while preserving local-only behavior for
existing commands. `--offline` covers both Cargo and AoC traffic: cached inputs
can be used, but a missing input returns a helpful error. `--input` conflicts
with `--download`; an alternate file is never a download destination.
`list`, help, generation, and benchmark report opening do not fetch inputs.

## G1: Credentials and input acquisition

### Separation of responsibilities

Add CLI modules for project discovery, credentials, date/calendar selection,
HTTP transport, input storage, and command orchestration. The runtime and proc
macros acquire no network or credential dependencies. Factor the existing
`run` function's package discovery into a reusable `ConsumerProject` value
containing the package identity, manifest directory, library source path,
workspace root, and resolved target directory.

Explicit `input --year ... --day ...` uses Cargo metadata only to select its
destination package. It must not build or execute the consumer. Reuse existing
Cargo configuration/package rules and keep the consumer lockfile unchanged;
fail with a resolution diagnostic if metadata cannot be obtained without
changing it. An unimplemented or temporarily broken solver must not prevent
downloading its input.

### Shared session file and resolution

Use `$HOME/.adventofcode.session` as the default session file. This is the
same file and plain-text format used by aoc-cli: an existing valid file works
with either tool without copying, conversion, import, or a second credential
store. The file contains only the raw session token, not TOML, JSON, quotes,
or a `session=` prefix. [aoc-cli documentation][session-readme],
[session loading and validation][session-source]

Resolve the session in this order:

1. An explicit `--session-file PATH` selects that file, overriding both the
   environment and default file. Relative paths resolve from the invocation
   directory. Expose the option on `input`, `run --download`,
   `bench --download`, and `credentials status`; reject it on run/bench
   without `--download` so it is not silently ignored.
2. A nonempty `ADVENT_OF_CODE_SESSION` environment variable supplies the
   raw token directly, using aoc-cli's variable name. A blank value after trimming
   is ignored, matching aoc-cli's fallback behavior. A nonblank malformed
   value is an error, not permission to select a different account.
3. Otherwise read `$HOME/.adventofcode.session`.

The explicit-file and environment precedence matches aoc-cli. Its additional
config-directory fallback is intentionally outside this design: the shared
home file is the sole default file location. Do not look for a project-local
session file, a tool-specific TOML file, or cargo-aoc's legacy stores.
[aoc-cli lookup rules][session-readme]

On Unix, honor `$HOME` when resolving the default file; on other platforms,
use the OS user home directory. If the home directory cannot be resolved,
report that the caller must supply `--session-file` or
`ADVENT_OF_CODE_SESSION`. Never interpret an unavailable home as the current
directory. Resolve home only if the selected source needs it.

Read UTF-8 text and trim surrounding whitespace, accepting the usual final
LF or CRLF. The remaining value must be nonempty ASCII hexadecimal text,
matching aoc-cli's validator; internal spaces, newlines, control characters,
and cookie prefixes fail validation. Do not impose a fixed token length.
An explicitly selected file that is missing, unreadable, or malformed is an
error even if another source is available. Report a missing default file
with its path and instructions to place the raw token there or use an
override. Do not print file contents or token-bearing headers in diagnostics.
[aoc-cli validator][session-source]

The shared file is user-managed. The harness reads it without creating,
rewriting, deleting, moving, or changing its permissions. Do not introduce
`credentials set`, `clear`, or `import`: those operations could unexpectedly
change the account used by aoc-cli and other tools sharing the file. Users
switch accounts by updating their file or choosing an explicit override.
Recommend owner-only permissions (`0600` on Unix) in setup documentation.

Keep `credentials status` as a read-only diagnostic: report the selected
source (file path or environment variable), whether it can be loaded and
validated locally, and a nonzero status when unavailable/invalid. It neither
prints the token nor contacts AoC to verify whether it has expired.

Only the CLI resolves sessions, and only for a download cache miss or an
explicit status request. Help, local runs, generation, and entirely cached
downloads require no session file. Never copy a token into generated
manifests, bridge arguments, benchmark reports, or input-cache metadata.
The session-file option remains in the CLI and is not forwarded to the
runtime. Test resolution with injected home directories and synthetic tokens,
never the user's shared session file.

### Download selection and event calendars

Use a small `EventCalendar` abstraction with a clock supplied by the caller.
AoC specifies release at midnight EST/UTC-5. Resolve implicit dates using
that fixed offset, independent of the machine's timezone. [Official FAQ][aoc-about]

Do not reproduce cargo-aoc's hard-coded `1..26` bulk loop: the
[2025 calendar][aoc-2025] contains 12 puzzles. Seed known calendars with 25
days for 2015–2024 and 12 for 2025, matching the historical
[2024][aoc-2024] and [2025][aoc-2025] calendar boundaries. Do not assume an
unverified schedule for future years. For an unknown event, require an explicit day/list rather than
`--all` or an implicit date, until calendar data is added. Explicit requests
remain bounded to the harness's supported `1..=25`; a known calendar rejects
out-of-event days before HTTP. Unknown years must be at least 2015, no later
than the current EST year, and cannot request a date before its December
release time. The server remains authoritative about actual availability.

For a known current event, `--all` selects released days only, not future
ones. Before its first release, return an explanatory error. For a completed
event, it selects the entire known calendar. An explicit future day fails
locally; 404 remains a distinct unavailable-day error for clock/schedule
disagreements. Keep generic local solver registration at days 1–25, so this
network-specific calendar does not invalidate existing fixtures or custom runs.

### HTTP and storage contract

Use `reqwest` with rustls in the CLI, a reusable blocking client, and requests
to `https://adventofcode.com/{year}/day/{day}/input`. Send the `session` cookie
and a descriptive project User-Agent. Set connection/overall timeouts and
disable redirects so the credential cannot reach another origin. Download
days sequentially; cached files produce no requests. Do not introduce polling
or automatic release-time retry loops.

Resolve each destination relative to `ConsumerProject.manifest_dir` and share
the runtime's input-path rules: prefer `dayN.txt`, then `dayNN.txt`. If either
exists, keep it, even if empty; unreadable existing files are errors. A new
download uses the unpadded filename. Only a true not-found condition qualifies
for downloading, not permission errors or invalid UTF-8. Check the cache before
requiring credentials, allowing entirely cached commands without a token.

Accept a successful text response, validate UTF-8, reject HTML/login content,
and impose a documented body limit (initially 16 MiB) before committing it.
Preserve bytes and the terminal newline on disk; the runtime continues to
remove trailing LF characters when loading. Write to a temporary file in the
same directory and publish with an atomic no-clobber operation. If another
process wins the race, preserve its file and report the existing result.
Transport/validation failures must leave no final input file. No refresh or
overwrite behavior is included in the first version.

Report authentication failures (including login redirects), unavailable
inputs, rate limits, other HTTP failures, transport failures, and file failures
separately, without printing cookie/header values or arbitrary response bodies.
On authentication failure or HTTP 429, stop the batch and return nonzero;
include a safe `Retry-After` value when supplied. Other independent day
failures may be collected while processing the remaining days. Print cached,
downloaded, and failed counts; any failure means nonzero status. Successful
cache entries remain available after a partial batch failure. Document that
personal downloaded inputs should stay out of version control.

### Integrating fetch-and-run without duplicating selection

The CLI cannot infer the latest registered day from source text or a cached
list: Cargo features and `cfg` affect the registry. Add an internal,
versioned bridge request, `__describe --protocol 1`, handled before ordinary
runtime action parsing. It returns one JSON object with the registry year,
consumer manifest directory, protocol/capabilities, normalized selection,
and required input days. It reads metadata only: no input files, generation,
solving, or benchmark execution.

Factor selection into a runtime execution plan shared by describe/run/bench.
For benchmarks the plan includes both selected variants and the default
solutions used by the full-run family, plus applicable generator families.
Validate selection and filters before initiating a download. The CLI passes
the same selectors to describe and execution, verifies the protocol response
against the selected package, downloads missing files once per required day,
then launches the already-built bridge normally. Do not parse human `list`
output or add an independently selected runtime dependency to the bridge.

Keep the existing bridge main source forwarding to
`consumer::__aoc_harness_main()`. New runtimes implement the hidden request;
older runtimes can still execute ordinary existing commands with a new CLI.
If an older runtime rejects describe or lacks the required capability,
`--download` fails with a clear upgrade diagnostic before network requests.
Explicit `input --year --day` remains usable independently of that protocol.

All fetching completes before the runtime loads inputs and starts any timing
or Criterion validation/measurement. A download failure prevents execution of
the selected pass. Directly running a consumer executable remains local-only.

## G2: Day scaffolding

`generate` creates a parser, both annotated parts, and clearly marked example
test placeholders using this project's imports and error conventions. Starter
solvers return a typed not-implemented error rather than fake answers; example
tests remain ignored until the user supplies an example and expected result.
Use separate setup/action/assertion paragraphs in generated tests. No template
contains personal input or copied puzzle statements.

Resolve the selected library source through Cargo metadata, including custom
`[lib].path`. Create `dayN.rs` beside that source and insert `mod dayN;` in
the root module before its `aoc_lib!` invocation, after any crate-level
attributes and inner documentation. Preserve unrelated source bytes and line
endings; use syntax-aware item locations instead of prepending text blindly.

Before writing, construct a `ScaffoldPlan` for every selected day:

1. Find existing root declarations/files with `dayN` or `dayNN` spelling.
   An existing matching module and file is an unchanged result, not an
   invitation to overwrite its contents.
2. Refuse ambiguous cases such as multiple matching modules, inline or
   conditional declarations, a conflicting `#[path]`, or a file/declaration
   mismatch. Report exactly what needs manual resolution. The first version
   does not guess how an arbitrary custom module layout maps to puzzle days.
3. Validate that the root is parseable, that insertion is unambiguous, and
   that proposed paths stay within the selected package. Refuse source-file
   symlinks and recheck original file contents immediately before applying
   edits, so a concurrent user edit is not overwritten.
4. Render every proposed file/edit. `--dry-run` prints paths and a diff and
   performs no source writes, downloads, or builds.

Preflight the entire batch before writing anything. Create new files without
clobbering existing ones; atomically replace the root source while preserving
permissions. If the root edit fails, remove only the new, unchanged files this
operation created and report any rollback failure. This is a recoverable
multi-file operation, not a filesystem-wide atomic transaction. Repeating a
successful generation is idempotent. Do not modify consumer manifests,
lockfiles, or unrelated source, and do not format the entire project.

`input --generate` preflights the source plan first, fetches the requested
inputs, then applies the plan only if acquisition succeeds. Already downloaded
inputs remain cached if a later source edit fails. `run --generate` applies
its explicit-day plan before building the bridge; if `--download` is also
present, acquisition follows compiled selection. All commands use the same
planner/applicator. Ordinary run/list/bench continue to preserve source files.

## G3: Optional generator and solver returns

Replace the macros' `(Type, bool)` output classification with an explicit
`OutputKind::{Value, Result, Option}` and the successful inner type.
Recognize the final path segment `Option` in the same way as `Result`,
including `std::option::Option<T>` and imports retaining that name. Continue
to document that arbitrary aliases are not inferred.

For generators and solvers, `Some(value)` supplies the successful value;
`None` becomes a normal harness error with day, part/variant, and phase
context. Add a runtime helper for a contextual message rather than inventing
an error type in every adapter. A failed generator includes its attempted
generation time and never calls the solver. A failed solver retains elapsed
solver time. Run output follows existing partial-run/nonzero semantics.

Use one wrapper-normalization generator for timed runs, untimed execution,
generator checks, and every Criterion loop. Benchmark prevalidation must reject
`None` before reporting success. If an error occurs only in a measurement
iteration, abort measurement as existing `Result` failures do. Keep borrowed
parser outputs and every ownership mode working without `Clone` or `'static`
bounds on parsed input.

Interpret exactly one outer wrapper. For example, `Result<Option<T>, E>`
unwraps its `Result`, leaving `Option<T>` as the success value; nested wrappers
are not recursively flattened. Successful solver answers still require
`Display + 'static`.

Changing generator `Option<T>` from a parsed value to a fallible return can
break existing consumers that intentionally pass `&Option<T>` to their solver.
Add `output = "value"` to generator and solver annotations to suppress wrapper
normalization explicitly. Document that migration in the next API release and
include a compile/run fixture for the old optional-parsed-value use case.

## G4: Custom borrowed input projections

Keep signature-driven borrowing/coercion as the default. Replacing all
borrowed calls with `Borrow::borrow` would break types supported only by
`Deref` coercion, while stable Rust cannot generally specialize a fallback
between arbitrary coercions and user-defined `Borrow` implementations.

Add an explicit `input = "borrow"` solver option:

```rust,ignore
#[aoc(day4, part1, input = "borrow")]
fn part1(grid: &GridView) -> usize {
    // The generator's owner implements Borrow<GridView>.
    todo!()
}
```

For `&T`, generate a fully qualified `std::borrow::Borrow<T>::borrow` call
on the prepared owner. For `&mut T`, use `BorrowMut<T>::borrow_mut` and
prepare a fresh owner for each invocation/benchmark iteration. Reject this
option on by-value arguments or when no generator is registered; owned
solvers continue to receive the original generated value. The requested
target can be unsized and its lifetime remains tied to the prepared owner.

Centralize argument construction so timed calls, untimed calls, reused-input
benchmarks, and fresh batched benchmarks apply the same projection and
black-box policy. Preserve `bench_input = "fresh"` for interior mutation
through a shared projection. Compile fixtures must cover a custom projection
that cannot work through deref coercion, alongside existing slice/deref and
borrowed-input cases. This provides the upstream capability with an explicit
migration annotation rather than changing all existing call sites.

## G5: Consumer-owned executable entry points

Re-export a new `aoc_main!` macro from the runtime with two forms:

```rust,ignore
// src/main.rs alongside an annotated library
aoc_harness::aoc_main! { lib = winter_solutions }

// Alternatively, at the end of a standalone binary's source
aoc_harness::aoc_main! { year = 2025 }
```

The library form emits a `main` forwarding to that library's exported harness
entry point and returns its exit status. Accept a Rust crate path, including
renamed library dependencies. It uses the same run/list/bench options as the
generated bridge; `cargo run --release -- --all-days` provides the whole-event
standalone workflow. It adds no downloader or credential access.

The standalone form reuses a factored registry builder, scanning the binary's
module tree instead of assuming `src/lib.rs`. Resolve the active binary from
Cargo's target identity and manifest target paths, covering `src/main.rs`,
`src/bin/name.rs`, `src/bin/name/main.rs`, and explicit `[[bin]].path`.
Provide `source = "relative/path.rs"` as an explicit escape hatch where
target identity is unavailable or ambiguous, validated relative to the
manifest. Never silently scan the wrong binary in a multi-target package.

Keep private nested modules, `#[path]`, source-change tracking, conditional
registrations, and compile-time identity/default validation shared between
`aoc_lib!` and standalone `aoc_main!`. Emit only one registry and one `main`;
duplicate entry declarations get actionable diagnostics. Raw input is loaded
at runtime using the existing manifest-relative rules, instead of upstream's
compile-time `include_str!` embedding of personal input.

The Cargo subcommand continues to require a library target, as upstream's
CLI does. Binary-only consumers use `cargo run`; extending Cargo CLI package
selection to binary registries is separate work. Neither form changes the
existing direct-annotation discovery limitation described above.

## G6: Reports and profiling convenience

Handle `bench --open` / `-o` in the CLI. After a successful measured benchmark,
resolve `<target-dir>/criterion/<consumer-package>/report/index.html` from the
same project/report configuration passed to the bridge, verify it exists,
and open its file URL with `webbrowser`. Do not scrape terminal output for a
path. Reject `--open --smoke` before building, and never open an old report
after a failed benchmark. An opener failure returns nonzero with a distinct
message saying measurement succeeded and giving the report path. Tests use an
injected opener; test runs must not launch a real browser.

Add `--debug-info` for run and bench. Preserve `--profile NAME` as the Cargo
profile selector; cargo-aoc's boolean `--profile` has different semantics and
cannot be an alias. Enable debug info through the generated build's effective
Cargo configuration for the selected profile, without changing optimization
or consumer manifests. Give the explicit flag precedence over copied profile
debug settings, including package-specific profile overrides, and include
its resolved settings in bridge identity/freshness inputs. Also disable symbol
stripping for this build so a copied `strip` setting cannot remove the requested
debug info. Unsupported profile combinations should fail explicitly rather
than silently lose symbols.
Compiler diagnostics and build failures continue to propagate normally.

The existing `--config 'profile.release.debug=true'` or a custom workspace
profile remains the available manual alternative when the convenience flag is unnecessary.

## G7: CLI compatibility details

Add `--version` / `-V` using the CLI package version, without invoking Cargo
or requiring a consumer. Add `-d` for day, `-i` for input, and accept `day4`
and `part2` anywhere their numeric selector equivalents are accepted. Keep
`--days`, repeated/comma-separated selections, and numeric validation.
Do not reinterpret `-p`, which already selects a package. Use `--part` for
the puzzle part.

Keep `--generate` explicit and command-scoped. Upstream reuses `-g` for
different operations (`generate` versus generator benchmarks); no ambiguous
global `-g` alias is necessary. Generator benchmarks already run by default;
`bench --filter '^generator/'` selects only that family when desired.

The former CLI split build arguments from runtime arguments manually and
maintained a separate help schema. The shared structured command schema now
parses CLI-owned operations and lowers execution options into bridge arguments. Preserve options before/after commands and values that happen to
equal command names. Validate conflicts before Cargo, network, or source
changes; do not keep extending string matching in several unrelated places.
Help for every new command must work outside a project, just as existing help
does. [Current argument split][h-cli], [help tests][h-workflow]

## G8: Distribution

Prepare publication in dependency order: macros, runtime, CLI. Keep
publishable version constraints alongside workspace path dependencies; ensure
README/license/repository metadata and package contents are complete, including
templates and licenses. Document `cargo install aoc-harness-cli --locked` and
a versioned `aoc-harness` dependency once publication actually exists.

Validate each packaged crate in an isolated consumer without checkout paths,
then test the installed command and both entry-point forms. Cover the
CLI/runtime capability boundary, including older consumers: existing commands
remain usable and new protocol-dependent operations request a runtime upgrade
clearly. Publication is a release operation, separate from implementing this
proposal; the design document does not publish crates or reserve names.

## Implementation boundaries and order

| Area | Concrete changes |
| --- | --- |
| `crates/aoc-harness-cli/src/lib.rs` | Extract project discovery/build execution; dispatch structured commands; compose generation, describe, download, execution, and opening. Keep Cargo JSON artifact discovery and diagnostic forwarding. |
| New CLI modules and templates | `credentials`, `input`, `calendar`, `scaffold`, project/command types, HTTP/opener seams, and harness-specific day templates. |
| `crates/aoc-harness-cli/src/command.rs` | Shared parsing/help definitions; show only applicable flags, conflicts, and examples. |
| `crates/aoc-harness/src/lib.rs` | Shared execution planning, versioned describe response, contextual no-value errors, and public macro re-export. Shared pure input-path resolution must stay free of HTTP. |
| `crates/aoc-harness-macros/src/lib.rs` | Output-kind classification, projection construction, factored library/binary registry builder, and `aoc_main!`. |
| Fixtures and integration tests | Synthetic network/credential fixtures, optional/projection adapters, generated consumers, standalone/custom binary targets, and report/profiling checks. |
| Package manifests and documentation | CLI-only HTTP/platform dependencies, parsing/serialization dependencies where needed, package metadata, API/migration/install documentation. |

Implement and review in this order:

1. **Planning and parsing:** extract project discovery and runtime plans;
   add protocol handling, command parsing/help, aliases, and version output.
   Verify existing run/list/bench behavior and old-runtime compatibility.
2. **Input workflow:** credentials, explicit single/bulk input, event calendar,
   atomic cache writes, and `--download` composition. No source edits yet.
3. **Scaffolding:** shared dry-run/preflight/apply operation and the input/run
   aliases, retaining source-preservation checks for ordinary execution.
4. **Solution API parity:** optional returns, projection opt-in, and both
   executable entry points, with migration fixtures.
5. **Convenience and release readiness:** report opening, debug symbols,
   packaged-consumer validation, and updated user documentation.

Each stage is useful independently. Completion of the proposal requires all
stages; no stage should be labeled complete merely because download support
works. Update this inventory against implemented code as features land.

## Validation and acceptance criteria

Tests use synthetic inputs and tokens, a fixed clock, temporary home
directories, an injected HTTP transport/local test server, and an injected
browser opener. Production endpoint selection is fixed; test endpoint injection
is private test plumbing, not an environment switch that could leak a real
session cookie. No automated test contacts AoC, reads real credentials, or
checks downloaded personal inputs into the repository.

| Area | Required observable checks |
| --- | --- |
| Existing behavior | Existing workspace/installed-CLI checks still pass. Latest-day and feature-dependent selection, explicit defaults, timing boundaries, fresh inputs, custom library names, Cargo configuration, diagnostic forwarding, and consumer source/manifest/lockfile preservation remain unchanged. |
| Plan protocol | Describe performs no parser/solver/file-input work; run and bench use the same selected scope. Cover feature-enabled latest days, benchmark default inclusion, invalid filters/selections, malformed or unsupported protocol data, and older runtimes. |
| Credentials | Explicit file > nonblank `ADVENT_OF_CODE_SESSION` > `$HOME/.adventofcode.session`; blank environment fallback; malformed nonblank environment failure; aoc-cli-compatible raw files with LF/CRLF; missing/unreadable/invalid files; invocation-relative overrides; unavailable home; read-only status outside Cargo; no config/project/TOML fallback; and no token leakage. Snapshot the shared file's bytes and permissions to prove all commands leave it unchanged. |
| Downloads | Correct URL/cookie/User-Agent; cache hit without credentials or HTTP; padded fallback; preserved newlines; manifest-relative paths with spaces; redirects/auth errors, 404, 429, timeouts, oversized/non-UTF-8/HTML responses, and no final file after failure. Simulate a concurrent cache writer. |
| Dates and bulk input | EST versus local/UTC boundaries, December first release, known completed years, the 12-day 2025 calendar, unknown schedules, out-of-event days, future dates, cache-only offline success, missing offline failure, and partial-batch exit status. |
| Composition | All required inputs arrive before timed work. `--input` never fetches. Failed acquisition never runs solvers. Explicit input works with uncompilable solutions. `input --generate` and `run --generate --download` follow their documented order. |
| Scaffolding | First application, no-op repeat, dry-run, a custom root source, crate attributes/docs/line endings, padded modules, conflicts and cfg/path ambiguity, batch preflight, concurrent edits, write failure/rollback, and unchanged manifests/lockfiles. Generated code compiles before implementation. |
| Optional returns | `Some`/`None` in each phase; borrowed non-Clone parsed values; consuming/mutable/shared solvers; qualified `Option`; `output = "value"`; one-layer wrapper behavior; partial timings; smoke prevalidation; and a failure first occurring during real measurement. |
| Borrow projections | Custom `Borrow` and `BorrowMut`, an unsized target, borrowed lifetimes, an absent trait diagnostic, invalid owned/raw opt-in, unchanged deref/slice coercions, and fresh state under mutation/interior mutation. |
| Entry points | Library-forwarding and standalone binaries compile/run/list/bench; custom/multiple binary paths select the right source or fail clearly; private/cfg modules and source edits affect the registry; failures return nonzero. |
| Reports/profiling | Open exactly the successful measured report; never open on smoke/failure; handle a missing report/opener failure; verify debug configuration for release/custom/package overrides without changing optimization or the consumer. |
| Distribution | Packaged crates resolve without checkout paths; the installed CLI supports both invocation forms, help/version outside Cargo, and a standalone library consumer using published-style dependencies. |

Use smoke runs for broad benchmark-family coverage and a small real Criterion
run for new adapter/projection measurement loops. Formatting and normal
workspace tests cover implementation changes; the implementation adds synthetic HTTP, credential, calendar, scaffolding,
protocol, adapter, entry-point, profiling, and packaged-consumer checks. The
validation commands and their scope are documented in the README.

Feature parity is achieved when every Missing/Partial workflow in the table
has its specified implementation and validation, with the explicit boundaries
retained: local-only defaults, no secret echo, existing `-p`/`--profile`
semantics, year-aware bulk downloads, direct-annotation discovery, and
library-only Cargo CLI discovery.

## Source references

Upstream links are pinned to the inspected commit so the comparison remains
auditable. Local links identify implementation files rather than treating
the existing API proposal as proof of implemented behavior.

[up-commit]: https://github.com/gobanos/cargo-aoc/commit/c260ff45120bbfcbed2983b9d220919a1d98efd1
[up-manifest]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/Cargo.toml
[up-readme]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/README.md
[up-cli]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/main.rs
[up-args]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/args.rs
[up-app]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/app.rs
[up-project]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/project.rs
[up-credentials]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/credentials.rs
[up-date]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/src/date.rs
[up-day]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/template/src/day.rs.tpl
[up-run-template]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/template/src/runner.rs.tpl
[up-bench-template]: https://github.com/gobanos/cargo-aoc/tree/c260ff45120bbfcbed2983b9d220919a1d98efd1/cargo-aoc/template/benches
[up-runtime]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner/src/lib.rs
[up-macros]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/lib.rs
[up-generator]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/generator.rs
[up-runner]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/runner.rs
[up-out]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/out.rs
[up-utils]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/utils.rs
[up-map]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-derive/src/map.rs
[up-internal]: https://github.com/gobanos/cargo-aoc/blob/c260ff45120bbfcbed2983b9d220919a1d98efd1/aoc-runner-internal/src/lib.rs
[session-readme]: https://github.com/scarvalhojr/aoc-cli/blob/ebb1069b2946320c76ed676cf26f0a287d4f64fd/README.md#L89-L107
[session-source]: https://github.com/scarvalhojr/aoc-cli/blob/ebb1069b2946320c76ed676cf26f0a287d4f64fd/aoc-client/src/lib.rs#L592-L655
[h-readme]: ../../README.md
[h-cli]: ../../crates/aoc-harness-cli/src/lib.rs
[h-help]: ../../crates/aoc-harness-cli/src/command.rs
[h-runtime]: ../../crates/aoc-harness/src/lib.rs
[h-macros]: ../../crates/aoc-harness-macros/src/lib.rs
[h-fixture]: ../../fixtures/library/src/lib.rs
[h-workflow]: ../../crates/aoc-harness-cli/tests/workflow.rs
[aoc-about]: https://adventofcode.com/2025/about
[aoc-2024]: https://adventofcode.com/2024
[aoc-2025]: https://adventofcode.com/2025
