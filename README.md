# Repipe-Debug.rs

## Overview

This Rust script performs a Spruce merge operation on YAML files, adding blame information to track the origin of each line in the merged output. It's designed to work with a specific directory structure and file naming convention.

![image](https://github.com/user-attachments/assets/18c5edea-28dc-4c31-9d4b-ef171b33ed0e)

## Dependencies

The script uses the following external crates:
- `std`: Rust standard library
- `regex`: For regular expression operations
- `tempfile`: For creating temporary files
- `walkdir`: For recursively walking directories

## Main Functions

### `main()`

The entry point of the program. It performs the following steps:
1. Parses command-line arguments
2. Sets the current working directory
3. Collects input files for merging
4. Calls `spruce_merge_with_blame()` to perform the merge operation

### `spruce_merge_with_blame(output_file: &str, files: &[String]) -> io::Result<()>`

This function is responsible for merging the input files using Spruce and adding blame information. It performs the following steps:
1. Executes the Spruce merge command
2. Processes the merged content line by line
3. Resolves placeholders in each line
4. Adds blame information (file and line number) for each line
5. Writes the processed content to the output file

### `resolve_placeholders(line: &str, merged_temp_file: &str) -> String`

This function resolves various types of placeholders in a given line:
1. `(( param "..." ))`: Replaced with environment variables or default values
2. `(( grab <something> ))`: Replaced with values from the merged YAML or other files
3. `(( concat ... ))`: Concatenates multiple values

### `find_hierarchical_line_number(file: &str, key: &str, value: &str) -> io::Result<usize>`

This function finds the line number of a key-value pair in a YAML file, considering the hierarchical structure.

### `safe_execute(cmd: &str) -> io::Result<(String, i32, String)>`

A utility function that safely executes shell commands and returns the stdout, exit status, and stderr.

### `escape_for_regex(s: &str) -> String`

A utility function that escapes special characters in a string for use in regular expressions.

## Detailed Workflow

1. The script starts by collecting input files:
   - `pipeline/base.yml`
   - `settings.yml`
   - All `.yml` files in the `pipeline` directory (excluding those containing "custom" or "optional" in their names)

2. It then calls `spruce_merge_with_blame()` with these input files and the specified output file.

3. `spruce_merge_with_blame()` performs the following:
   - Executes the Spruce merge command
   - Processes each line of the merged output:
     - Skips comments and empty lines
     - Resolves placeholders using `resolve_placeholders()`
     - Attempts to find the origin (file and line number) of each line
     - Writes the processed line with blame information to the output file

4. The `resolve_placeholders()` function handles three types of placeholders:
   - `(( param "..." ))`: Replaced with environment variables or default values
   - `(( grab <something> ))`: Replaced with values from the merged YAML
   - `(( concat ... ))`: Concatenates multiple values

5. The script uses regular expressions extensively to identify and process placeholders.

## Error Handling

- The script uses Rust's `Result` type for error handling
- It prints warnings for non-critical issues (e.g., Spruce merge errors) but continues execution
- Critical errors (e.g., file I/O errors) will cause the program to terminate

## Limitations and Considerations

- The script assumes a specific directory structure and naming convention for input files
- It may not handle all possible YAML structures or Spruce merge scenarios
- The blame information might not be 100% accurate in all cases, especially for complex merges or heavily modified content

## Usage

Run the script with the desired output file as an argument:

```
cargo run -- <output_merged.yml>
```

The script will merge the input files, add blame information, and write the result to the specified output file.
