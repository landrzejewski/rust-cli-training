# 006 – Predictable output: human, JSON, YAML, tables

## Goal

Serve two audiences from one command: people (readable, aligned, concise) and programs (stable,
parseable, versioned) – selected with `--output` and sensible automatic defaults.

Example: `examples/006-output-formats` – `platctl-services list|get -o auto|human|table|json|jsonl|yaml|tsv`.

## Key concepts

### Output is an API

Once scripts parse your output, its shape is a contract:
- **machine formats** (JSON, YAML, TSV) – stable field names, types and order of columns; additive
  changes only; a schema version (`"api_version": "platctl/v1"`) for deliberate breaking changes,
- **human formats** – free to change and improve; never ask users to parse them,
- data → stdout, everything else (progress, warnings, errors) → stderr (step 001).

### Formats and when to use them

| Format | For | Notes |
|--------|-----|-------|
| table | humans at a terminal | aligned, header, adapts to terminal width |
| human | humans, single objects | key/value or sentences |
| `json` | scripts, `jq`, APIs | one document; list wrapped in an envelope |
| `jsonl` | streaming, logs, huge lists | one compact object per line, process line by line |
| `yaml` | config-like output, `kubectl`-style | human-editable, round-trips to input files |
| `tsv` | `cut`, `awk`, `sort`, `while read` | no decoration, `--no-headers` |

`kubectl -o json|yaml|wide|jsonpath`, `gh --json fields --jq expr`, `docker --format '{{json .}}'` are
good references.

### TTY detection – different defaults for people and pipes

```rust
use std::io::IsTerminal;
let format = cli.output.resolve(io::stdout().is_terminal());   // auto: table vs tsv
```

`ls`, `git`, `grep --color=auto` all do this. Rules: decide on **stdout** (not stdin/stderr), allow
an explicit override (`-o table` into a file is legitimate), never change machine formats based on TTY.

### serde does the heavy lifting

```rust
#[derive(Serialize)]
pub struct Service { name: String, replicas: u32, status: Status, updated_at: String }
#[derive(Serialize)] #[serde(rename_all = "lowercase")] pub enum Status { Running, Degraded, Stopped }

serde_json::to_string_pretty(&doc)?   // json
serde_json::to_string(&item)?         // jsonl line
serde_saphyr::to_string(&doc)?        // yaml
```

- one `Serialize` struct = one schema for all formats,
- enums as strings, timestamps as RFC 3339 UTC, numbers as numbers (not `"3"`),
- YAML crates: `serde_yaml` is unmaintained → `serde-saphyr`, `serde_norway`, `serde_yaml_ng`.

### Tables

`comfy-table` (builder, dynamic width, styles) or `tabled` (derive `Tabled` on structs). Keep the
table a pure rendering of the same data; never put information only in the table.

### Design checklist

- every list command supports `-o json`; plus filters/field selection for big objects,
- single resource → object, list → array (in an envelope),
- empty result: valid empty document (`"items": []`), exit 0,
- not found: message on stderr, non-zero exit, no partial document on stdout,
- render functions return `String`/write to `impl Write` → unit tests without capturing stdout.

## Example walkthrough

- `src/model.rs` – `Service` as the serializable schema, enum status as lowercase strings.
- `src/output.rs` – `Format` (`ValueEnum`), `resolve` for `auto`, renderers for all formats, JSON
  envelope with `api_version`; tests for envelope, JSONL, TSV, YAML.
- `src/main.rs` – global `-o/--output`, `--no-headers`, TTY detection, errors on stderr.

## Try it

```bash
cargo test -p ex006-output-formats
cargo build -p ex006-output-formats && P=target/debug/platctl-services
$P list                              # table (terminal)
$P list | cat                        # tsv (pipe)
$P list -o json | jq '.items[] | select(.status != "running") | .name'
$P list -o jsonl | jq -c 'select(.replicas > 2)'
$P list --no-headers | awk -F'\t' '$4 > 2 {print $1}'
$P get billing -o yaml
$P get nope -o json; echo $?          # stderr message, exit 1, empty stdout
```

## Exercises

1. Add `--fields name,version` selecting columns for table/tsv and keys for json.
2. Add `-o wide` showing more columns than the default table.
3. Add `--output template='{{name}}:{{version}}'` with a minimal placeholder substitution.
