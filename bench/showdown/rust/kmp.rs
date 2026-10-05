enum List {
    Nil,
    Cons(i64, Box<List>),
}

fn match_state0(text: &List) -> i64 {
    match text {
        List::Nil => 0,
        List::Cons(c, rest) => {
            if *c == 1 {
                match_state1(rest)
            } else {
                match_state0(rest)
            }
        }
    }
}

fn match_state1(text: &List) -> i64 {
    match text {
        List::Nil => 0,
        List::Cons(c, rest) => {
            if *c == 0 {
                match_state2(rest)
            } else {
                match_state1(rest)
            }
        }
    }
}

fn match_state2(text: &List) -> i64 {
    match text {
        List::Nil => 0,
        List::Cons(c, rest) => {
            if *c == 1 {
                1 + match_state1(rest)
            } else {
                match_state0(rest)
            }
        }
    }
}

fn make_text(n: i64) -> List {
    if n <= 0 {
        List::Nil
    } else {
        let bit = (n * 73 + 19) % 2;
        List::Cons(bit, Box::new(make_text(n - 1)))
    }
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..100 {
        let t = make_text(15);
        sum += match_state0(&t);
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
