//! Rendering the same data in different output formats.
//!
//! Every renderer returns a `String` – pure functions, trivially testable. `main` decides only
//! WHICH renderer to use and writes the result to stdout.

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use serde::Serialize;

use crate::model::Service;

/// `--output` values. `ValueEnum` turns variants into accepted values (`json`, `jsonl`, ...)
/// and lists them in `--help`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Table on a terminal, TSV when piped
    Auto,
    /// Sentences for humans
    Human,
    /// Aligned table
    Table,
    /// JSON document (pretty-printed)
    Json,
    /// JSON Lines: one compact object per line (streaming, `jq -c`, log pipelines)
    Jsonl,
    /// YAML document
    Yaml,
    /// Tab-separated values: stable columns for `cut`, `awk`, `sort`
    Tsv,
}

impl Format {
    /// `auto` resolves to a concrete format: rich output for people, plain for programs.
    /// Same idea as `ls` printing columns on a terminal and one name per line into a pipe.
    pub fn resolve(self, stdout_is_terminal: bool) -> Self {
        match self {
            Self::Auto if stdout_is_terminal => Self::Table,
            Self::Auto => Self::Tsv,
            other => other,
        }
    }
}

/// Machine-readable envelope with a schema version. Scripts depend on the JSON shape – changing
/// it is a breaking change. A version field lets you evolve it deliberately.
#[derive(Serialize)]
struct ListDocument<'a> {
    api_version: &'static str,
    items: &'a [Service],
}

const API_VERSION: &str = "platctl/v1";

pub fn render_list(services: &[Service], format: Format, headers: bool) -> String {
    match format {
        Format::Auto | Format::Table => table(services, headers),
        Format::Human => services
            .iter()
            .map(|s| {
                format!(
                    "{} {} is {} in {} with {} replica(s), updated {}\n",
                    s.name, s.version, s.status, s.env, s.replicas, s.updated_at
                )
            })
            .collect(),
        Format::Json => json(&ListDocument {
            api_version: API_VERSION,
            items: services,
        }),
        // One object per line, no envelope – each line is self-contained.
        Format::Jsonl => services
            .iter()
            .map(|s| serde_json::to_string(s).expect("serializable") + "\n")
            .collect(),
        Format::Yaml => serde_saphyr::to_string(&ListDocument {
            api_version: API_VERSION,
            items: services,
        })
        .expect("serializable"),
        Format::Tsv => tsv(services, headers),
    }
}

pub fn render_one(service: &Service, format: Format) -> String {
    match format {
        Format::Human => format!(
            "Service:  {}\nEnv:      {}\nVersion:  {}\nReplicas: {}\nStatus:   {}\nUpdated:  {}\n",
            service.name,
            service.env,
            service.version,
            service.replicas,
            service.status,
            service.updated_at
        ),
        // A single resource is a single object, not a list with one element.
        Format::Json => json(service),
        Format::Jsonl => serde_json::to_string(service).expect("serializable") + "\n",
        Format::Yaml => serde_saphyr::to_string(service).expect("serializable"),
        Format::Auto | Format::Table | Format::Tsv => {
            render_list(std::slice::from_ref(service), format, true)
        }
    }
}

fn json(value: &impl Serialize) -> String {
    // Serializing our own plain structs can't fail; `expect` documents that assumption.
    serde_json::to_string_pretty(value).expect("serializable") + "\n"
}

fn table(services: &[Service], headers: bool) -> String {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL_CONDENSED)
        // Wrap/shrink columns to the terminal width instead of overflowing.
        .set_content_arrangement(ContentArrangement::Dynamic);
    if headers {
        table.set_header(["NAME", "ENV", "VERSION", "REPLICAS", "STATUS", "UPDATED"]);
    }
    for s in services {
        table.add_row([
            s.name.as_str(),
            s.env.as_str(),
            s.version.as_str(),
            &s.replicas.to_string(),
            &s.status.to_string(),
            s.updated_at.as_str(),
        ]);
    }
    table.to_string() + "\n"
}

/// TSV: no alignment, no decoration, values never contain tabs – trivially parseable.
fn tsv(services: &[Service], headers: bool) -> String {
    let mut out = String::new();
    if headers {
        out.push_str("name\tenv\tversion\treplicas\tstatus\tupdated_at\n");
    }
    for s in services {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            s.name, s.env, s.version, s.replicas, s.status, s.updated_at
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::sample;

    #[test]
    fn auto_depends_on_terminal() {
        assert_eq!(Format::Auto.resolve(true), Format::Table);
        assert_eq!(Format::Auto.resolve(false), Format::Tsv);
        assert_eq!(Format::Json.resolve(false), Format::Json);
    }

    #[test]
    fn json_has_versioned_envelope_and_stable_field_names() {
        let value: serde_json::Value =
            serde_json::from_str(&render_list(&sample()[..1], Format::Json, true)).unwrap();

        assert_eq!(value["api_version"], "platctl/v1");
        assert_eq!(value["items"][0]["name"], "billing");
        assert_eq!(value["items"][0]["status"], "running");
        assert_eq!(value["items"][0]["replicas"], 3);
    }

    #[test]
    fn jsonl_is_one_object_per_line() {
        let output = render_list(&sample(), Format::Jsonl, true);

        assert_eq!(output.lines().count(), sample().len());
        for line in output.lines() {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }

    #[test]
    fn tsv_without_headers() {
        let output = render_list(&sample()[..1], Format::Tsv, false);

        assert_eq!(
            output,
            "billing\tprod\t1.4.2\t3\trunning\t2026-09-30T12:00:00Z\n"
        );
    }

    #[test]
    fn yaml_round_trips() {
        let yaml = render_one(&sample()[0], Format::Yaml);

        assert!(yaml.contains("name: billing"), "{yaml}");
    }
}
