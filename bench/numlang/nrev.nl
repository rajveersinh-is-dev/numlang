enum List {
    Nil,
    Cons(i64, Box<List>),
}

fn append(xs: List, ys: List) -> List {
    return match xs {
        Nil => ys,
        Cons(h, t) => Cons(h, box(append(deref(t), ys))),
    };
}

fn nrev(xs: List) -> List {
    return match xs {
        Nil => Nil,
        Cons(h, t) => append(nrev(deref(t)), Cons(h, box(Nil))),
    };
}

fn sum_list(xs: List) -> i64 {
    return match xs {
        Nil => 0,
        Cons(h, t) => h + sum_list(deref(t)),
    };
}

fn make_list(n: i64) -> List {
    if n <= 0 {
        return Nil;
    } else {
        return Cons(n, box(make_list(n - 1)));
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..100 {
        let xs: List = make_list(15);
        let rev: List = nrev(xs);
        sum = sum + sum_list(rev);
    }
    println(sum);
    return sum % 256;
}
