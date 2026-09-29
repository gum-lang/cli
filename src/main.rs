use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "gum")]
#[command(about = "Gum markup language CLI", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a Gum document for syntax and semantic errors
    Validate {
        /// Path to the Gum file to validate
        file: PathBuf,
    },
    /// Format a Gum document with standardized formatting
    Format {
        /// Path to the Gum file to format
        file: PathBuf,
        /// Write changes back to the file (default: print to stdout)
        #[arg(short, long)]
        write: bool,
    },
    /// Convert a Gum document to another format
    Convert {
        /// Path to the Gum file to convert
        file: PathBuf,
        /// Target format (json, toml, yaml)
        #[arg(short, long)]
        to: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file } => cmd_validate(&file),
        Commands::Format { file, write } => cmd_format(&file, write),
        Commands::Convert { file, to, output } => cmd_convert(&file, &to, output.as_deref()),
    }
}

fn cmd_validate(file: &PathBuf) -> Result<ExitCode> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    match gum_core::parse(&content) {
        Ok(doc) => {
            let errors = gum_core::validate(&doc);
            if errors.is_empty() {
                println!("✓ Valid: {}", file.display());
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!("✗ Validation errors in {}:", file.display());
                for error in errors {
                    eprintln!("  - {}", error);
                }
                Ok(ExitCode::FAILURE)
            }
        }
        Err(error) => {
            eprintln!("✗ Parse error in {}: {}", file.display(), error);
            Ok(ExitCode::FAILURE)
        }
    }
}

fn cmd_format(file: &PathBuf, write: bool) -> Result<ExitCode> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    let doc = gum_core::parse(&content)
        .with_context(|| format!("Failed to parse file: {}", file.display()))?;

    let formatted = gum_core::format(&doc);

    if write {
        fs::write(file, &formatted)
            .with_context(|| format!("Failed to write file: {}", file.display()))?;
        println!("✓ Formatted: {}", file.display());
    } else {
        print!("{}", formatted);
    }

    Ok(ExitCode::SUCCESS)
}

fn cmd_convert(file: &PathBuf, to: &str, output: Option<&std::path::Path>) -> Result<ExitCode> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    let doc = gum_core::parse(&content)
        .with_context(|| format!("Failed to parse file: {}", file.display()))?;

    let converted = match to.to_lowercase().as_str() {
        "json" => convert_to_json(&doc)?,
        "toml" => convert_to_toml(&doc)?,
        "yaml" => convert_to_yaml(&doc)?,
        _ => {
            eprintln!("✗ Unsupported format: {}", to);
            eprintln!("  Supported formats: json, toml, yaml");
            return Ok(ExitCode::FAILURE);
        }
    };

    if let Some(output_path) = output {
        fs::write(output_path, &converted)
            .with_context(|| format!("Failed to write file: {}", output_path.display()))?;
        println!("✓ Converted to {}: {}", to, output_path.display());
    } else {
        print!("{}", converted);
    }

    Ok(ExitCode::SUCCESS)
}

fn convert_to_json(doc: &gum_core::Document) -> Result<String> {
    let mut map = serde_json::Map::new();

    for element in &doc.elements {
        if let gum_core::DocumentElement::Assignment(assignment) = element {
            let key = assignment.key.as_str();
            let value = gum_value_to_json(&assignment.value);
            map.insert(key, value);
        }
    }

    serde_json::to_string_pretty(&JsonValue::Object(map))
        .with_context(|| "Failed to serialize to JSON")
}

fn convert_to_yaml(doc: &gum_core::Document) -> Result<String> {
    let mut map = serde_json::Map::new();

    for element in &doc.elements {
        if let gum_core::DocumentElement::Assignment(assignment) = element {
            let key = assignment.key.as_str();
            let value = gum_value_to_json(&assignment.value);
            map.insert(key, value);
        }
    }

    serde_yaml::to_string(&JsonValue::Object(map))
        .with_context(|| "Failed to serialize to YAML")
}

fn gum_value_to_json(value: &gum_core::Value) -> JsonValue {
    match value {
        gum_core::Value::String(s) => {
            let content = match s {
                gum_core::StringValue::Basic(c) => c,
                gum_core::StringValue::MultilineBasic(c) => c,
                gum_core::StringValue::Literal(c) => c,
                gum_core::StringValue::MultilineLiteral(c) => c,
            };
            JsonValue::String(content.clone())
        }
        gum_core::Value::Number(n) => match n {
            gum_core::NumberValue::Integer(i) => {
                serde_json::Number::from(*i).into()
            }
            gum_core::NumberValue::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            }
        },
        gum_core::Value::Boolean(b) => JsonValue::Bool(*b),
        gum_core::Value::Null => JsonValue::Null,
        gum_core::Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for entry in &table.entries {
                let key = entry.key.as_str();
                let val = gum_value_to_json(&entry.value);
                map.insert(key, val);
            }
            JsonValue::Object(map)
        }
        gum_core::Value::List(list) => {
            let elements: Vec<JsonValue> = list.elements.iter().map(gum_value_to_json).collect();
            JsonValue::Array(elements)
        }
    }
}

fn convert_to_toml(doc: &gum_core::Document) -> Result<String> {
    let mut toml = String::new();

    for element in &doc.elements {
        if let gum_core::DocumentElement::Assignment(assignment) = element {
            toml.push_str(&format!(
                "{} = {}\n",
                assignment.key.as_str(),
                format_value_toml(&assignment.value)
            ));
        }
    }

    Ok(toml)
}

fn format_value_toml(value: &gum_core::Value) -> String {
    match value {
        gum_core::Value::String(s) => {
            let content = match s {
                gum_core::StringValue::Basic(c) => c,
                gum_core::StringValue::MultilineBasic(c) => c,
                gum_core::StringValue::Literal(c) => c,
                gum_core::StringValue::MultilineLiteral(c) => c,
            };
            format!("\"{}\"", content.replace('\\', "\\\\").replace('"', "\\\""))
        }
        gum_core::Value::Number(n) => match n {
            gum_core::NumberValue::Integer(i) => format!("{}", i),
            gum_core::NumberValue::Float(f) => format!("{}", f),
        },
        gum_core::Value::Boolean(b) => format!("{}", b),
        gum_core::Value::Null => "\"\"".to_string(),
        gum_core::Value::Table(table) => {
            let mut parts = Vec::new();
            for entry in &table.entries {
                parts.push(format!(
                    "{} = {}",
                    entry.key.as_str(),
                    format_value_toml(&entry.value)
                ));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        gum_core::Value::List(list) => {
            let parts: Vec<String> = list.elements.iter().map(format_value_toml).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_gum(input: &str) -> gum_core::Document {
        gum_core::parse(input).expect("Failed to parse test input")
    }

    #[test]
    fn test_json_basic_assignments() {
        let doc = parse_gum(r#"name = "Gum""#);
        let json = convert_to_json(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "Gum");
    }

    #[test]
    fn test_json_number_types() {
        let doc = parse_gum("int_val = 42\nfloat_val = 2.71");
        let json = convert_to_json(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["int_val"], 42);
        assert_eq!(parsed["float_val"], 2.71);
    }

    #[test]
    fn test_json_boolean_and_null() {
        let doc = parse_gum("enabled = true\ndisabled = false\nnothing = null");
        let json = convert_to_json(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["enabled"], true);
        assert_eq!(parsed["disabled"], false);
        assert_eq!(parsed["nothing"], serde_json::Value::Null);
    }

    #[test]
    fn test_json_nested_table() {
        let doc = parse_gum(r#"server = { host = "localhost", port = 8080 }"#);
        let json = convert_to_json(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["server"]["host"], "localhost");
        assert_eq!(parsed["server"]["port"], 8080);
    }

    #[test]
    fn test_json_list() {
        let doc = parse_gum(r#"items = [1, "two", true]"#);
        let json = convert_to_json(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["items"][0], 1);
        assert_eq!(parsed["items"][1], "two");
        assert_eq!(parsed["items"][2], true);
    }

    #[test]
    fn test_yaml_basic() {
        let doc = parse_gum(r#"name = "Gum""#);
        let yaml = convert_to_yaml(&doc).unwrap();
        assert!(yaml.contains("name:"));
        assert!(yaml.contains("Gum"));
    }

    #[test]
    fn test_yaml_nested() {
        let doc = parse_gum(r#"server = { host = "localhost" }"#);
        let yaml = convert_to_yaml(&doc).unwrap();
        assert!(yaml.contains("server:"));
        assert!(yaml.contains("host:"));
    }

    #[test]
    fn test_toml_basic() {
        let doc = parse_gum(r#"name = "Gum""#);
        let toml = convert_to_toml(&doc).unwrap();
        assert_eq!(toml.trim(), r#"name = "Gum""#);
    }

    #[test]
    fn test_toml_table() {
        let doc = parse_gum(r#"server = { host = "localhost", port = 8080 }"#);
        let toml = convert_to_toml(&doc).unwrap();
        assert!(toml.contains("server ="));
        assert!(toml.contains(r#"host = "localhost""#));
        assert!(toml.contains("port = 8080"));
    }
}
