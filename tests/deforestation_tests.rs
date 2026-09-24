use numlang::mir::lower::lower_program;
use numlang::mir::supercompiler::supercompile_mir_program;
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
fn test_deforestation_struct_construction_and_field_access() {
    let code = r#"
    struct Point {
        x: i64,
        y: i64,
    }

    fn distance_squared(a: i64, b: i64) -> i64 {
        let p: Point = Point { x: a, y: b };
        return p.x * p.x + p.y * p.y;
    }
    "#;

    let mut mir_program = get_mir(code);
    let stats = supercompile_mir_program(&mut mir_program);

    assert_eq!(mir_program.functions.len(), 1);
    let func = &mir_program.functions[0];
    assert_eq!(func.name, "distance_squared");
    // Verify that the function was explored and retained clean blocks
    assert!(!func.blocks.is_empty());
    assert!(stats.nodes_explored >= 1);
}

#[test]
fn test_deforestation_chained_function_passing() {
    let code = r#"
    struct Pair {
        first: i64,
        second: i64,
    }

    fn make_pair(x: i64, y: i64) -> Pair {
        return Pair { first: x + 1, second: y * 2 };
    }

    fn consume_pair(p: Pair) -> i64 {
        return p.first + p.second;
    }

    fn pipeline(x: i64, y: i64) -> i64 {
        let p: Pair = make_pair(x, y);
        return consume_pair(p);
    }
    "#;

    let mut mir_program = get_mir(code);
    let stats = supercompile_mir_program(&mut mir_program);

    assert_eq!(mir_program.functions.len(), 3);
    assert!(stats.nodes_explored >= 3);
}
