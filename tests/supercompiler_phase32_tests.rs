use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::drive::SupercompilerDriver;
use numlang::mir::supercompiler::recurrence::detect_nway_linear_system;
use numlang::mir::supercompiler::residualize::residualize_process_tree;
use numlang::parser::parse;
use numlang::token::tokenize;
use numlang::typecheck::typecheck;

fn get_mir(source: &str) -> numlang::mir::lower::MirProgram {
    let tokens = tokenize(source).unwrap();
    let program = parse(&tokens).unwrap();
    let typed = typecheck(&program).unwrap();
    lower_program(&typed)
}

#[test]
fn test_hofstadter_male_female_recurrence() {
    // 1. Direct trajectory unit verification
    // M: 0, 1, 1, 2, 3, 5, 8, 13, 21, 34
    // F: 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
    let traj_m = vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34];
    let traj_f = vec![1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    let sys_opt = detect_nway_linear_system(&[traj_m, traj_f]);
    assert!(
        sys_opt.is_some(),
        "Expected detect_nway_linear_system to return Some for Hofstadter trajectories"
    );
    let sys = sys_opt.unwrap();
    assert_eq!(sys.n, 2);

    // 2. Supercompiler driving test
    let src = r#"
fn hofstadter(n: i64) -> i64 {
    let mut m: i64 = 0;
    let mut f: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let new_m: i64 = f;
        let new_f: i64 = m + f;
        m = new_m;
        f = new_f;
        i = i + 1;
    }
    return m;
}

fn main() -> i64 {
    return hofstadter(10);
}
"#;
    let mir = get_mir(src);
    let hof_func = mir.functions.iter().find(|f| f.name == "hofstadter").unwrap();
    let driver = SupercompilerDriver::new(hof_func);
    let tree = driver.run();

    assert!(
        tree.stats.loops_collapsed >= 1,
        "Expected loops_collapsed >= 1 for Hofstadter, got: {}",
        tree.stats.loops_collapsed
    );
}

#[test]
fn test_tribonacci_closed_form() {
    let src = r#"
fn trib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 0;
    let mut c: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next: i64 = a + b + c;
        a = b;
        b = c;
        c = next;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    return trib(10);
}
"#;
    let mir = get_mir(src);
    let trib_func = mir.functions.iter().find(|f| f.name == "trib").unwrap();
    let driver = SupercompilerDriver::new(trib_func);
    let tree = driver.run();

    assert!(
        tree.stats.loops_collapsed >= 1,
        "Expected loops_collapsed >= 1 for Tribonacci, got: {}",
        tree.stats.loops_collapsed
    );

    let residualized = residualize_process_tree(&tree, trib_func);
    let mir_str = format!("{:#?}", residualized);
    let has_closed_form =
        mir_str.contains("__nway_recurrence_0") || mir_str.contains("__order3_recurrence");
    assert!(
        has_closed_form,
        "Residualized MIR must contain __nway_recurrence_0 or __order3_recurrence. MIR:\n{}",
        mir_str
    );
}

#[test]
fn test_3way_linear_recurrence_closed_form() {
    let src = r#"
fn triple_rec(n: i64) -> i64 {
    let mut x: i64 = 1;
    let mut y: i64 = 2;
    let mut z: i64 = 3;
    let mut i: i64 = 0;
    while i < n {
        let nx: i64 = y + z;
        let ny: i64 = x + z;
        let nz: i64 = x + y;
        x = nx;
        y = ny;
        z = nz;
        i = i + 1;
    }
    return x;
}

fn main() -> i64 {
    return triple_rec(8);
}
"#;
    let mir = get_mir(src);
    let triple_func = mir.functions.iter().find(|f| f.name == "triple_rec").unwrap();
    let driver = SupercompilerDriver::new(triple_func);
    let tree = driver.run();

    assert!(
        tree.stats.loops_collapsed >= 1,
        "Expected loops_collapsed >= 1 for 3-way linear recurrence, got: {}",
        tree.stats.loops_collapsed
    );
}

#[test]
fn test_detect_nway_system_unit() {
    // Tribonacci: T(n) = T(n-1) + T(n-2) + T(n-3)
    // T: 0, 0, 1, 1, 2, 4, 7, 13, 24, 44, 81, 149
    // In companion matrix form with state vector [T(k), T(k-1), T(k-2)]:
    // T(k)   = 1*T(k-1) + 1*T(k-2) + 1*T(k-3)
    // T(k-1) = 1*T(k-1) + 0*T(k-2) + 0*T(k-3)
    // T(k-2) = 0*T(k-1) + 1*T(k-2) + 0*T(k-3)
    // Companion matrix A = [[1, 1, 1], [1, 0, 0], [0, 1, 0]]
    let c_traj = vec![1, 1, 2, 4, 7, 13, 24, 44, 81, 149];
    let b_traj = vec![0, 1, 1, 2, 4, 7, 13, 24, 44, 81];
    let a_traj = vec![0, 0, 1, 1, 2, 4, 7, 13, 24, 44];

    let sys_opt = detect_nway_linear_system(&[c_traj, b_traj, a_traj]);
    assert!(
        sys_opt.is_some(),
        "detect_nway_linear_system must return Some for Tribonacci"
    );
    let sys = sys_opt.unwrap();
    assert_eq!(sys.n, 3, "Expected sys.n == 3");
    let expected_companion = vec![
        vec![1, 1, 1],
        vec![1, 0, 0],
        vec![0, 1, 0],
    ];
    assert_eq!(
        sys.a, expected_companion,
        "sys.a must match Tribonacci companion matrix [[1,1,1],[1,0,0],[0,1,0]]"
    );
}

#[test]
fn test_existing_coupled_still_works() {
    let src = r#"
fn coupled(n: i64) -> i64 {
    let mut a: i64 = 1;
    let mut b: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        let next_a: i64 = 2 * a + b;
        let next_b: i64 = a + 2 * b;
        a = next_a;
        b = next_b;
        i = i + 1;
    }
    return a + b;
}

fn main() -> i64 {
    return coupled(4);
}
"#;
    let mir = get_mir(src);
    let coupled_func = mir.functions.iter().find(|f| f.name == "coupled").unwrap();
    let driver = SupercompilerDriver::new(coupled_func);
    let tree = driver.run();

    assert!(
        tree.stats.loops_collapsed >= 1,
        "Expected loops_collapsed >= 1 for coupled 2-var recurrence, got: {}",
        tree.stats.loops_collapsed
    );
}
