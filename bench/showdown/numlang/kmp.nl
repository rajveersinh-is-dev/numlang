enum List {
    Nil,
    Cons(i64, Box<List>),
}

fn match_state0(text: List) -> i64 {
    return match text {
        Nil => 0,
        Cons(c, rest) => match c {
            1 => match_state1(deref(rest)),
            _ => match_state0(deref(rest)),
        },
    };
}

fn match_state1(text: List) -> i64 {
    return match text {
        Nil => 0,
        Cons(c, rest) => match c {
            0 => match_state2(deref(rest)),
            _ => match_state1(deref(rest)),
        },
    };
}

fn match_state2(text: List) -> i64 {
    return match text {
        Nil => 0,
        Cons(c, rest) => match c {
            1 => 1 + match_state1(deref(rest)),
            _ => match_state0(deref(rest)),
        },
    };
}

fn make_text(n: i64) -> List {
    if n <= 0 {
        return Nil;
    } else {
        let bit: i64 = (n * 73 + 19) % 2;
        return Cons(bit, box(make_text(n - 1)));
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..100 {
        let t: List = make_text(15);
        sum = sum + match_state0(t);
    }
    println(sum);
    return sum % 256;
}
