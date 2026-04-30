use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug)]
struct TestCase {
    name: String,
    description_lines: Vec<String>,
}

#[derive(Debug)]
struct SpecFile {
    path: PathBuf,
    tests: Vec<TestCase>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "spec" => {
            let check = match args.next().as_deref() {
                None => false,
                Some("--check") => true,
                Some(other) => return Err(format!("unknown argument: {other}\n{}", usage())),
            };

            if let Some(extra) = args.next() {
                return Err(format!("unexpected argument: {extra}\n{}", usage()));
            }

            generate_spec(check)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage:\n  cargo xtask spec\n  cargo xtask spec --check".to_string()
}

fn generate_spec(check: bool) -> Result<(), String> {
    let root = workspace_root()?;
    let tests_dir = root.join("backend/tests");
    let spec_path = root.join("SPEC.md");

    let spec_files = collect_spec_files(&tests_dir)
        .map_err(|err| format!("failed to read {}: {err}", tests_dir.display()))?;
    let rendered = render_spec(&spec_files);

    if check {
        match fs::read_to_string(&spec_path) {
            Ok(existing) if existing == rendered => Ok(()),
            Ok(_) => Err(format!(
                "{} is out of date. Run `cargo xtask spec`.",
                spec_path.display()
            )),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Err(format!(
                "{} is missing. Run `cargo xtask spec`.",
                spec_path.display()
            )),
            Err(err) => Err(format!("failed to read {}: {err}", spec_path.display())),
        }
    } else {
        fs::write(&spec_path, rendered)
            .map_err(|err| format!("failed to write {}: {err}", spec_path.display()))?;
        println!("generated {}", spec_path.display());
        Ok(())
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve workspace root".to_string())
}

fn collect_spec_files(dir: &Path) -> io::Result<Vec<SpecFile>> {
    let mut rust_files = Vec::new();
    walk_rs_files(dir, &mut rust_files)?;
    rust_files.sort();

    rust_files
        .into_iter()
        .map(|path| {
            let contents = fs::read_to_string(&path)?;
            let tests = parse_test_cases(&contents);
            Ok(SpecFile { path, tests })
        })
        .collect()
}

fn walk_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, files)?;
        } else if path.extension() == Some(OsStr::new("rs")) {
            files.push(path);
        }
    }

    Ok(())
}

fn parse_test_cases(contents: &str) -> Vec<TestCase> {
    let lines: Vec<&str> = contents.lines().collect();
    let mut cases = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if is_test_attribute(trimmed) {
            let description_lines = collect_doc_comments(&lines, index);
            if let Some((name, next_index)) = find_test_fn(&lines, index + 1) {
                cases.push(TestCase {
                    name: snake_case_to_words(&name),
                    description_lines,
                });
                index = next_index;
                continue;
            }
        }
        index += 1;
    }

    cases
}

fn is_test_attribute(line: &str) -> bool {
    matches!(line, "#[test]" | "#[tokio::test]")
}

fn collect_doc_comments(lines: &[&str], attr_index: usize) -> Vec<String> {
    let mut comments = Vec::new();
    let mut cursor = attr_index;

    while cursor > 0 {
        let prev = lines[cursor - 1].trim();
        if let Some(comment) = prev.strip_prefix("///") {
            comments.push(comment.trim().to_string());
            cursor -= 1;
        } else {
            break;
        }
    }

    comments.reverse();
    comments
}

fn find_test_fn(lines: &[&str], start: usize) -> Option<(String, usize)> {
    for (offset, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            continue;
        }

        if let Some(name) = parse_fn_name(trimmed) {
            return Some((name, offset + 1));
        }

        if !trimmed.is_empty() {
            return None;
        }
    }

    None
}

fn parse_fn_name(line: &str) -> Option<String> {
    let candidate = line
        .strip_prefix("async fn ")
        .or_else(|| line.strip_prefix("fn "))?;

    let name: String = candidate
        .chars()
        .take_while(|ch| *ch == '_' || ch.is_ascii_alphanumeric())
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn snake_case_to_words(name: &str) -> String {
    name.replace('_', " ")
}

fn render_spec(spec_files: &[SpecFile]) -> String {
    let mut output = String::from("# SPEC\n");

    for spec_file in spec_files {
        output.push('\n');
        output.push_str("## ");
        output.push_str(&display_path(&spec_file.path));
        output.push('\n');

        for test in &spec_file.tests {
            output.push('\n');
            output.push_str("### ");
            output.push_str(&test.name);
            output.push('\n');

            for line in &test.description_lines {
                output.push_str(line);
                output.push('\n');
            }
        }
    }

    output
}

fn display_path(path: &Path) -> String {
    let root = workspace_root().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
