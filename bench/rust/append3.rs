enum List {
    Nil,
    Cons(i64, Box<List>),
}

fn append(xs: List, ys: List) -> List {
    match xs {
        List::Nil => ys,
        List::Cons(h, t) => List::Cons(h, Box::new(append(*t, ys))),
    }
}

fn sum_list(xs: &List) -> i64 {
    match xs {
        List::Nil => 0,
        List::Cons(h, t) => *h + sum_list(t),
    }
}

fn make_list(start: i64, len: i64) -> List {
    if len <= 0 {
        List::Nil
    } else {
        List::Cons(start, Box::new(make_list(start + 1, len - 1)))
    }
}

fn main() {
    let mut sum: i64 = 0;
    for _ in 0..100 {
        let xs = make_list(1, 10);
        let ys = make_list(11, 10);
        let zs = make_list(21, 10);
        let app = append(append(xs, ys), zs);
        sum += sum_list(&app);
    }
    println!("{}", sum);
    std::process::exit((sum % 256) as i32);
}
