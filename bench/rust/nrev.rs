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

fn nrev(xs: List) -> List {
    match xs {
        List::Nil => List::Nil,
        List::Cons(h, t) => append(nrev(*t), List::Cons(h, Box::new(List::Nil))),
    }
}

fn sum_list(xs: &List) -> i64 {
    match xs {
        List::Nil => 0,
        List::Cons(h, t) => *h + sum_list(t),
    }
}

fn make_list(n: i64) -> List {
    if n <= 0 {
        List::Nil
    } else {
        List::Cons(n, Box::new(make_list(n - 1)))
    }
}

fn main() {
    let mut sum: i64 = 0;
    for _ in 0..100 {
        let xs = make_list(15);
        let rev = nrev(xs);
        sum += sum_list(&rev);
    }
    println!("{}", sum);
    std::process::exit((sum % 256) as i32);
}
