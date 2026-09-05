//! Gum CLI - Command-line interface for the Gum markup language
//!
//! Provides validation, formatting, and conversion tools for Gum documents.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
        /// Target format (json, html, toml)
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

/// Validate a Gum document
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

/// Format a Gum document
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

/// Convert a Gum document to another format
fn cmd_convert(file: &PathBuf, to: &str, output: Option<&std::path::Path>) -> Result<ExitCode> {
    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    let doc = gum_core::parse(&content)
        .with_context(|| format!("Failed to parse file: {}", file.display()))?;

    let converted = match to.to_lowercase().as_str() {
        "json" => convert_to_json(&doc)?,
        "html" => convert_to_html(&doc)?,
        "toml" => convert_to_toml(&doc)?,
        _ => {
            eprintln!("✗ Unsupported format: {}", to);
            eprintln!("  Supported formats: json, html, toml");
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

/// Convert Gum AST to JSON
fn convert_to_json(doc: &gum_core::Document) -> Result<String> {
    serde_json::to_string_pretty(doc).with_context(|| "Failed to serialize to JSON")
}

/// Convert Gum AST to HTML
fn convert_to_html(doc: &gum_core::Document) -> Result<String> {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n  <meta charset=\"UTF-8\">\n  <title>Gum Document</title>\n</head>\n<body>\n");

    for element in &doc.elements {
        match element {
            gum_core::DocumentElement::Assignment(assignment) => {
                html.push_str("  <div class=\"assignment\">\n");
                html.push_str(&format!(
                    "    <span class=\"key\">{}</span>",
                    escape_html(&assignment.key.as_str())
                ));
                html.push_str(&format!(
                    " = <span class=\"value\">{}</span>\n",
                    escape_html(&format_value(&assignment.value))
                ));
                html.push_str("  </div>\n");
            }
            gum_core::DocumentElement::NonAssignment(na) => {
                // Skip whitespace-only content
                if !na.content.trim().is_empty() {
                    html.push_str(&format!("  <!-- {} -->\n", escape_html(&na.content)));
                }
            }
        }
    }

    html.push_str("</body>\n</html>\n");
    Ok(html)
}

/// Convert Gum AST to TOML
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

/// Format a value for display
fn format_value(value: &gum_core::Value) -> String {
    match value {
        gum_core::Value::String(s) => {
            let content = match s {
                gum_core::StringValue::Basic(c) => c,
                gum_core::StringValue::MultilineBasic(c) => c,
                gum_core::StringValue::Literal(c) => c,
                gum_core::StringValue::MultilineLiteral(c) => c,
            };
            format!("\"{}\"", content)
        }
        gum_core::Value::Number(n) => match n {
            gum_core::NumberValue::Integer(i) => format!("{}", i),
            gum_core::NumberValue::Float(f) => format!("{}", f),
        },
        gum_core::Value::Boolean(b) => format!("{}", b),
        gum_core::Value::Null => "null".to_string(),
        gum_core::Value::Table(_) => "{...}".to_string(),
        gum_core::Value::List(_) => "[...]".to_string(),
    }
}

/// Format a value for TOML output
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
        gum_core::Value::Null => "\"\"".to_string(), // TOML doesn't have null, use empty string
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

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
