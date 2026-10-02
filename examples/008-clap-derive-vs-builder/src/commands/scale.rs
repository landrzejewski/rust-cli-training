/// Business rules belong here, not in the parser: `0` is a valid `u32` but not a valid
/// replica count in production.
pub fn run(service: &str, replicas: u32, env: &str) -> anyhow::Result<String> {
    if env == "prod" && replicas == 0 {
        anyhow::bail!("refusing to scale {service} to zero in prod");
    }
    Ok(format!(
        "scaled {service} in {env} to {replicas} replica(s)"
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn prod_cannot_be_scaled_to_zero() {
        assert!(super::run("billing", 0, "prod").is_err());
        assert!(super::run("billing", 0, "dev").is_ok());
    }
}
