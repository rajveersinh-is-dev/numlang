use std::fs;
use std::path::Path;

fn check_dir(dir: &Path, bad_strings: &[&str]) -> Vec<String> {
    let mut violations = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                violations.extend(check_dir(&path, bad_strings));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let content = fs::read_to_string(&path).unwrap();
                for (line_no, line) in content.lines().enumerate() {
                    for bad in bad_strings {
                        if line.contains(bad) && !line.trim_start().starts_with("//") {
                            violations.push(format!(
                                "{}:{}: contains banned string pattern '{}'",
                                path.display(),
                                line_no + 1,
                                bad
                            ));
                        }
                    }
                }
            }
        }
    }
    violations
}

#[test]
fn test_no_hardcoded_benchmark_names() {
    let bad_strings = [
        "== \"append\"",
        "== \"append3\"",
        "== \"ack\"",
        "== \"tak\"",
        "== \"sum_list\"",
        "== \"invert\"",
        "== \"reverse\"",
    ];
    let src_dir = Path::new("src");
    let violations = check_dir(src_dir, &bad_strings);

    if !violations.is_empty() {
        for v in &violations {
            eprintln!("{}", v);
        }
        panic!(
            "Found {} integrity violations (hardcoded benchmark/function names).",
            violations.len()
        );
    }
}
