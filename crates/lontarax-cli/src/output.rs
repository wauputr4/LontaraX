use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorOutput {
    pub error: String,
}

pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_section(title: &str) {
    println!("## {title}");
}

pub fn print_bullet(label: &str, value: impl std::fmt::Display) {
    println!("- {label}: {value}");
}
