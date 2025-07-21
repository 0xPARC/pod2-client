use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentFormat {
    Markdown,
    Latex,
    Typst,
}

impl DocumentFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Some(Self::Markdown),
            "latex" | "tex" => Some(Self::Latex),
            "typst" | "typ" => Some(Self::Typst),
            _ => None,
        }
    }
}

/// Convert document content to Markdown
pub fn convert_to_markdown(
    content: &str,
    format: &DocumentFormat,
) -> Result<String, Box<dyn std::error::Error>> {
    match format {
        DocumentFormat::Markdown => Ok(content.to_string()),
        DocumentFormat::Latex => latex_to_markdown(content),
        DocumentFormat::Typst => typst_to_markdown(content),
    }
}

/// Convert LaTeX to Markdown using pandoc
fn latex_to_markdown(latex_content: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Check if pandoc is available
    let pandoc_check = Command::new("pandoc").arg("--version").output();

    if pandoc_check.is_err() {
        return Err(
            "pandoc is not installed. Please install pandoc to convert LaTeX files.".into(),
        );
    }

    // Create temporary file for LaTeX content
    let mut temp_file = NamedTempFile::new()?;
    temp_file.write_all(latex_content.as_bytes())?;
    let temp_path = temp_file.path();

    // Run pandoc conversion
    let output = Command::new("pandoc")
        .arg("-f")
        .arg("latex")
        .arg("-t")
        .arg("markdown")
        .arg("--wrap=none") // Don't wrap lines
        .arg("--standalone") // Include necessary headers
        .arg(temp_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pandoc conversion failed: {stderr}").into());
    }

    let markdown = String::from_utf8(output.stdout)?;
    Ok(markdown)
}

/// Convert Typst to Markdown (placeholder for future LLM integration)
fn typst_to_markdown(typst_content: &str) -> Result<String, Box<dyn std::error::Error>> {
    // For now, return a placeholder indicating Typst conversion isn't fully supported
    let markdown = format!(
        "# Document converted from Typst\n\n> **Note**: This document was originally written in Typst. Full conversion to Markdown is not yet supported.\n\n```typst\n{typst_content}\n```"
    );
    Ok(markdown)
}
