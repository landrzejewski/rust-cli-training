//! File-based CLI snapshot tests: `tests/cmd/*.md` (or `*.toml`) describe commands and the
//! expected output. `TRYCMD=overwrite cargo test` updates the files after intended changes.

#[test]
fn cli_reference() {
    trycmd::TestCases::new().case("tests/cmd/*.md");
}
