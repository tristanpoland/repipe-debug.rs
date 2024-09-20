use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use regex::Regex;
use tempfile::NamedTempFile;
use walkdir::WalkDir;
use std::collections::HashMap;

fn escape_for_regex(s: &str) -> String {
    regex::escape(s).replace(" ", r"\s+")
}

fn safe_execute(cmd: &str) -> io::Result<(String, i32, String)> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_status = output.status.code().unwrap_or(-1);

    Ok((stdout, exit_status, stderr))
}

fn resolve_placeholders(line: &str, merged_temp_file: &str) -> String {
    let mut resolved_line = line.to_string();

    // Resolve (( param "..." )) with environment variables or default values
    let param_re = Regex::new(r#"\(\(\s*param\s+"([^"]+)"\s+\)\)"#).unwrap();
    while let Some(cap) = param_re.captures(&resolved_line) {
        let param_name = &cap[1];
        let param_value = env::var(param_name).unwrap_or_else(|_| format!("UNDEFINED_PARAM_{}", param_name));
        resolved_line = param_re.replace(&resolved_line, &param_value).into_owned();
    }

    // Resolve (( grab <something> )) by looking it up from the merged YAML or other files
    let grab_re = Regex::new(r#"\(\(\s*grab\s+([^\s]+)\s+\)\)"#).unwrap();
    while let Some(cap) = grab_re.captures(&resolved_line) {
        let grab_target = &cap[1];
        let cmd = format!("grep -oP '{}:\\s*\\K.*' {}", grab_target, merged_temp_file);
        let (grab_value, exit_status, _) = safe_execute(&cmd).unwrap();
        let grab_value = if exit_status == 0 { grab_value.trim().to_string() } else { format!("UNDEFINED_GRAB_{}", grab_target) };
        resolved_line = grab_re.replace(&resolved_line, &grab_value).into_owned();
    }

    // Resolve (( concat ... )) by concatenating the values
    let concat_re = Regex::new(r#"\(\(\s*concat\s+(.*?)\s+\)\)"#).unwrap();
    while let Some(cap) = concat_re.captures(&resolved_line) {
        let concat_targets = &cap[1];
        let mut concat_values = String::new();
        for target in concat_targets.split_whitespace() {
            let cmd = format!("grep -oP '{}:\\s*\\K.*' {}", target, merged_temp_file);
            let (value, exit_status, _) = safe_execute(&cmd).unwrap();
            let value = if exit_status == 0 { value.trim().to_string() } else { target.to_string() };
            concat_values.push_str(&value);
        }
        resolved_line = concat_re.replace(&resolved_line, &concat_values).into_owned();
    }

    resolved_line
}

fn find_hierarchical_line_number(file: &str, key: &str, value: &str) -> io::Result<usize> {
    let file = File::open(file)?;
    let reader = BufReader::new(file);
    let mut current_indent = 0;
    let mut line_number = 0;
    let mut found_key = false;
    let mut found_line = 0;

    for line in reader.lines() {
        line_number += 1;
        let line = line?;
        if let Some((indent, content)) = line.split_once(|c: char| !c.is_whitespace()) {
            let indent_level = indent.len();
            if indent_level <= current_indent {
                found_key = false;
            }
            if content.starts_with(&format!("{}:", key)) {
                found_key = true;
                current_indent = indent_level;
                found_line = line_number;
            } else if found_key && content.contains(value) {
                return Ok(found_line);
            }
        }
    }
    Ok(found_line)
}

fn spruce_merge_with_blame(output_file: &str, files: &[String]) -> io::Result<()> {
    println!("Performing Spruce merge...");
    let spruce_cmd = format!("spruce merge --skip-eval {}", files.join(" "));
    let (spruce_output, spruce_exit, spruce_error) = safe_execute(&spruce_cmd)?;
    if spruce_exit != 0 {
        eprintln!("Warning: Spruce merge reported errors: {}", spruce_error);
        eprintln!("Continuing with the merge result despite errors...");
    }

    println!("Processing merged content and adding blame...");
    let mut out_file = File::create(output_file)?;
    writeln!(out_file, "# Debug: Input files: {}", files.join(" "))?;

    // Create a map to store the content of each input file
    let mut file_contents: HashMap<String, Vec<String>> = HashMap::new();
    for file in files {
        file_contents.insert(file.clone(), fs::read_to_string(file)?.lines().map(String::from).collect());
    }

    for line in spruce_output.lines() {
        if line.trim().starts_with('#') || line.trim().is_empty() {
            writeln!(out_file, "{}", line)?;
            continue;
        }

        let resolved_line = resolve_placeholders(line, &spruce_output);

        if let Some((indent, content)) = resolved_line.split_once(|c: char| !c.is_whitespace()) {
            if let Some((key, value)) = content.split_once(':') {
                let value = value.trim();
                let mut found = false;

                for (file, contents) in &file_contents {
                    if let Some((line_number, _)) = contents.iter().enumerate()
                        .find(|(_, l)| l.contains(key) && l.contains(value)) {
                        writeln!(out_file, "{}# File: {} (Line: {})", indent, file, line_number + 1)?;
                        writeln!(out_file, "{}", resolved_line)?;
                        found = true;
                        break;
                    }
                }

                if !found {
                    writeln!(out_file, "{}", resolved_line)?;
                }
            } else {
                writeln!(out_file, "{}", resolved_line)?;
            }
        } else {
            writeln!(out_file, "{}", resolved_line)?;
        }
    }

    println!("Merge complete. Output written to: {}", output_file);

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <output_merged.yml>", args[0]);
        std::process::exit(1);
    }

    let base_dir = env::current_dir()?;
    env::set_current_dir(&base_dir)?;

    let mut input_files = vec![
        "pipeline/base.yml".to_string(),
        "settings.yml".to_string(),
    ];

    for entry in WalkDir::new("pipeline") {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension() == Some("yml".as_ref()) {
            let path_str = path.to_str().unwrap();
            if !path_str.contains("custom") && !path_str.contains("optional") {
                input_files.push(path_str.to_string());
            }
        }
    }

    let merged_file = &args[1];

    println!("Merging input files using Spruce and tracking blame...");
    spruce_merge_with_blame(merged_file, &input_files)?;

    println!("Merged file with blame information has been saved to: {}", merged_file);

    Ok(())
}