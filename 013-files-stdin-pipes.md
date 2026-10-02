# 013 – Files, standard input and pipes; working with Unix tools

## Goal

Write a CLI that composes with the Unix toolbox: reads files or stdin, streams with constant
memory, buffers output efficiently, survives closed pipes, and produces output that `grep`, `jq`,
`xargs -0`, `wc` can consume.

Example: `examples/013-files-stdin-pipes` – `platctl-logq`, a grep-like filter for logfmt logs.

## Key concepts

### The filter contract

```
producer | platctl-logq --level error | consumer
```

- input: files given as arguments, otherwise stdin; `-` = stdin explicitly (`cat a.log | logq b.log - c.log`),
- output: data on stdout only; messages on stderr,
- streaming: process line by line – works on 10 GB files and endless streams (`tail -f | logq`),
- exit codes like grep: 0 = matched, 1 = no match, 2 = error – usable in `if`/`&&`,
- missing file: report, continue with the rest, still exit correctly.

### Reading

```rust
let reader: Box<dyn BufRead> = match path {
    None => Box::new(io::stdin().lock()),
    Some(p) => Box::new(BufReader::new(File::open(p)?)),
};
let mut buffer = Vec::new();
while reader.read_until(b'\n', &mut buffer)? > 0 {      // bytes, reused buffer
    let line = String::from_utf8_lossy(&buffer);        // invalid UTF-8 doesn't abort
    ...; buffer.clear();
}
```

- `lines()` fails on the first invalid UTF-8 byte and allocates per line – fine for config, not for logs,
- `stdin().is_terminal()` with no input → hint on stderr ("reading from standard input, Ctrl+D to end").

### Writing – buffering

Rust's `stdout()` is **line-buffered** even into pipes and files: one syscall per line.

```rust
let mut out = BufWriter::new(io::stdout().lock());   // 8 KiB batches; lock once
writeln!(out, "{line}")?;
out.flush()?;                                        // report errors (disk full!), don't drop silently
```

For interactive tools that print progress in real time, flush explicitly or keep line buffering.

### Broken pipe (SIGPIPE / EPIPE)

`platctl-logq big.log | head -1` – `head` exits after one line, the pipe closes.

- C programs get SIGPIPE and die silently (the desired behavior),
- Rust **ignores SIGPIPE**: the next write returns `ErrorKind::BrokenPipe`; `println!` would even **panic**,
- correct handling: propagate write errors with `?`, treat `BrokenPipe` as success, print nothing:

```rust
Err(error) if is_broken_pipe(&error) => ExitCode::SUCCESS,
```

(Alternative: restore default SIGPIPE via `libc::signal(SIGPIPE, SIG_DFL)` – requires `unsafe`.)

### Interoperability details

| Need | Solution |
|------|----------|
| file names with spaces/newlines | `-0/--null` output, `xargs -0`, `find -print0` |
| structured processing | JSON Lines (`--json`) → `jq -c 'select(...)'` |
| counting | `--count`, or `| wc -l` |
| line endings | trim `\n` and `\r\n` |
| huge inputs | streaming + bounded memory (step 001 used a bounded ring buffer) |

### Testing pipes

Run the binary (`env!("CARGO_BIN_EXE_...")`) with piped stdin/stdout; simulate `| head` by closing
the read end early and assert exit code 0 and empty stderr.

## Example walkthrough

- `src/logfmt.rs` – borrowing logfmt parser + test.
- `src/main.rs` – inputs (files/stdin/`-`), TTY hint, `BufWriter`, byte-based reading with lossy
  decoding, filters, `--count`, `--json` (JSON Lines), `-l -0`, grep exit codes, broken pipe handling;
  `debug_assert` test (it caught a duplicated `-l` while writing the example).
- `tests/pipes.rs` – stdin filtering, exit codes, NUL output, closed pipe.
- `logs/*.log` – sample data.

## Try it

```bash
cargo test -p ex013-files-stdin-pipes
cargo build -p ex013-files-stdin-pipes && cd examples/013-files-stdin-pipes && P=../../target/debug/platctl-logq
cat logs/*.log | $P --level error
$P --json --level error logs/*.log | jq -r .service
$P -l -0 --level error logs/*.log | xargs -0 wc -l
$P -c logs/*.log
seq 1 1000000 | sed 's/^/level=info service=x i=/' | $P | head -2; echo ${PIPESTATUS[2]}   # 0, no error
$P --service none logs/*.log; echo $?                                                    # 1
if $P --level error -c logs/*.log >/dev/null; then echo "errors found"; fi
$P                                                                                        # hint on stderr
```

## Exercises

1. Add `-f/--follow`: keep reading a file as it grows (poll with `seek` + sleep, stop on Ctrl+C – step 004).
2. Support gzip-compressed inputs (`flate2::read::GzDecoder`) transparently by file extension.
3. Add `--since 10m` using the `ts=` field and the duration parser from step 009.
