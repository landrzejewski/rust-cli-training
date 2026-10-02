pub fn run(service: &str, force: bool, env: &str) -> anyhow::Result<String> {
    let mode = if force {
        "immediately"
    } else {
        "after draining connections"
    };
    Ok(format!("restarted {service} in {env} {mode}"))
}
