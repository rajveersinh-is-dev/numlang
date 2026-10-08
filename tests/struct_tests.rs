use std::fs;
use std::process::Command;

fn run_numlang_code(code: &str) -> (Option<i32>, String, String) {
    let id = format!(
        "{}_{:?}_{}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
    .replace(['(', ')', ' '], "_");
    let test_dir = std::env::temp_dir().join(format!("numlang_struct_{}", id));
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("test.nl");
    fs::write(&src_file, code).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to run numlang program");

    let _ = fs::remove_dir_all(&test_dir);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.code(), stdout, stderr)
}

#[test]
fn test_basic_struct_literal_and_field_read() {
    let code = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() -> i64 {
    let p: Point = Point { x: 15, y: 25 };
    return p.x + p.y;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(40));
}

#[test]
fn test_struct_field_mutation() {
    let code = r#"
struct Counter {
    val: i64,
    step: i64,
}

fn main() -> i64 {
    let mut c: Counter = Counter { val: 10, step: 5 };
    c.val = c.val + c.step;
    c.val = c.val + c.step;
    return c.val;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(20));
}

#[test]
fn test_struct_pass_by_value() {
    let code = r#"
struct Point {
    x: i64,
    y: i64,
}

fn manhattan_dist(p: Point, q: Point) -> i64 {
    let mut dx: i64 = p.x - q.x;
    if dx < 0 {
        dx = -dx;
    }
    let mut dy: i64 = p.y - q.y;
    if dy < 0 {
        dy = -dy;
    }
    return dx + dy;
}

fn main() -> i64 {
    let p1: Point = Point { x: 3, y: 7 };
    let p2: Point = Point { x: 9, y: 15 };
    return manhattan_dist(p1, p2);
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(14));
}

#[test]
fn test_struct_returned_from_function() {
    let code = r#"
struct Point {
    x: i64,
    y: i64,
}

fn make_point(x: i64, y: i64) -> Point {
    return Point { x: x * 2, y: y * 3 };
}

fn main() -> i64 {
    let p: Point = make_point(5, 7);
    return p.x + p.y;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(31));
}

#[test]
fn test_nested_structs() {
    let code = r#"
struct Vec2 {
    x: i64,
    y: i64,
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    mass: i64,
}

fn step_particle(p: Particle) -> Particle {
    return Particle {
        pos: Vec2 { x: p.pos.x + p.vel.x, y: p.pos.y + p.vel.y },
        vel: p.vel,
        mass: p.mass,
    };
}

fn main() -> i64 {
    let p: Particle = Particle {
        pos: Vec2 { x: 10, y: 20 },
        vel: Vec2 { x: 3, y: 4 },
        mass: 1,
    };
    let updated: Particle = step_particle(p);
    return updated.pos.x + updated.pos.y;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(37));
}

#[test]
fn test_array_of_structs() {
    let code = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() -> i64 {
    let pts: [Point; 3] = [
        Point { x: 10, y: 1 },
        Point { x: 20, y: 2 },
        Point { x: 30, y: 3 },
    ];
    let mut sum: i64 = 0;
    for i in 0..3 {
        let p: Point = pts[i];
        sum = sum + p.x + p.y;
    }
    return sum;
}
"#;
    let (exit_code, _stdout, _stderr) = run_numlang_code(code);
    assert_eq!(exit_code, Some(66));
}

#[test]
fn test_struct_type_errors() {
    let missing_field = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() -> i64 {
    let p: Point = Point { x: 10 };
    return 0;
}
"#;
    let (exit_code, _stdout, stderr) = run_numlang_code(missing_field);
    assert_ne!(exit_code, Some(0));
    assert!(stderr.contains("Missing field") || stderr.contains("missing"));

    let no_such_field = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() -> i64 {
    let p: Point = Point { x: 10, y: 20 };
    return p.z;
}
"#;
    let (exit_code, _stdout, stderr) = run_numlang_code(no_such_field);
    assert_ne!(exit_code, Some(0));
    assert!(
        stderr.contains("No field")
            || stderr.contains("z")
            || stderr.contains("Cannot access field")
    );
}
