//! Progress reporting with `indicatif`: one bar per service + an overall bar, updated from
//! worker threads.

use std::{thread, time::Duration};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub succeeded: Vec<String>,
    pub failed: Vec<String>,
}

/// `draw_target`: `stderr()` normally, `hidden()` for `--quiet` and in tests.
/// Progress goes to STDERR – stdout stays clean for the final machine-readable result.
pub fn rollout(services: &[String], fail: Option<&str>, draw_target: ProgressDrawTarget) -> Report {
    // `MultiProgress` coordinates several bars so they don't overwrite each other.
    let multi = MultiProgress::with_draw_target(draw_target);

    let overall = multi.add(ProgressBar::new(services.len() as u64));
    overall.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} rollout [{elapsed_precise}] {wide_bar:.green} {pos}/{len}",
        )
        .expect("valid template")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    // Animate the spinner even when no progress happens (waiting for the platform).
    overall.enable_steady_tick(Duration::from_millis(100));

    // Template placeholders: {prefix}, {msg}, {bar:WIDTH.COLOR/BG}, {pos}, {len}, {eta}, {per_sec}...
    let style = ProgressStyle::with_template("  {prefix:<14} [{bar:30.cyan/blue}] {pos:>3}% {msg}")
        .expect("valid template")
        .progress_chars("=> ");

    let workers: Vec<_> = services
        .iter()
        .enumerate()
        .map(|(index, service)| {
            let bar = multi.add(ProgressBar::new(100));
            bar.set_style(style.clone());
            bar.set_prefix(service.clone());
            let overall = overall.clone();
            let service = service.clone();
            let should_fail = fail == Some(service.as_str());
            // `ProgressBar` is `Clone + Send` (an `Arc` inside) – update it from any thread.
            thread::spawn(move || {
                for percent in 0..=100 {
                    if should_fail && percent == 60 {
                        // `abandon_with_message` – stop the bar where it is, keep it visible.
                        bar.abandon_with_message("✖ health check failed");
                        overall.inc(1);
                        return (service, false);
                    }
                    bar.set_position(percent);
                    bar.set_message(match percent {
                        0..=29 => "pulling image",
                        30..=79 => "starting replicas",
                        _ => "health checks",
                    });
                    thread::sleep(Duration::from_millis(8 + 4 * index as u64));
                }
                bar.finish_with_message("✔ done");
                overall.inc(1);
                (service, true)
            })
        })
        .collect();

    let mut report = Report {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };
    for worker in workers {
        let (service, ok) = worker.join().expect("worker thread");
        if ok {
            report.succeeded.push(service);
        } else {
            let warning = format!("warning: rollback of {service} scheduled");
            if multi.is_hidden() {
                // No bars (pipe, CI, --quiet): `MultiProgress::println` would be swallowed too.
                eprintln!("{warning}");
            } else {
                // `println` on MultiProgress prints ABOVE the bars without corrupting them.
                let _ = multi.println(warning);
            }
            report.failed.push(service);
        }
    }
    overall.finish_and_clear();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_success_and_failure() {
        let services = vec!["billing".to_string(), "search".to_string()];

        let report = rollout(&services, Some("search"), ProgressDrawTarget::hidden());

        assert_eq!(report.succeeded, ["billing"]);
        assert_eq!(report.failed, ["search"]);
    }
}
