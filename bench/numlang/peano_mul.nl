enum Peano {
    Zero,
    Succ(Box<Peano>),
}

fn add(x: Peano, y: Peano) -> Peano {
    return match x {
        Zero => y,
        Succ(p) => Succ(box(add(deref(p), y))),
    };
}

fn mul(x: Peano, y: Peano) -> Peano {
    return match x {
        Zero => Zero,
        Succ(p) => add(y, mul(deref(p), y)),
    };
}

fn to_int(x: Peano) -> i64 {
    return match x {
        Zero => 0,
        Succ(p) => 1 + to_int(deref(p)),
    };
}

fn from_int(n: i64) -> Peano {
    if n <= 0 {
        return Zero;
    } else {
        return Succ(box(from_int(n - 1)));
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..100 {
        let three: Peano = from_int(3);
        let four: Peano = from_int(4);
        let prod: Peano = mul(three, four);
        sum = sum + to_int(prod);
    }
    println(sum);
    return sum % 256;
}
