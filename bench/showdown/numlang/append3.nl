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

fn sum_list(xs: List) -> i64 {
    return match xs {
        Nil => 0,
        Cons(h, t) => h + sum_list(deref(t)),
    };
}

fn make_list(start: i64, len: i64) -> List {
    if len <= 0 {
        return Nil;
    } else {
        return Cons(start, box(make_list(start + 1, len - 1)));
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..100 {
        let xs: List = make_list(1, 10);
        let ys: List = make_list(11, 10);
        let zs: List = make_list(21, 10);
        let app: List = append(append(xs, ys), zs);
        sum = sum + sum_list(app);
    }
    println(sum);
    return sum % 256;
}
