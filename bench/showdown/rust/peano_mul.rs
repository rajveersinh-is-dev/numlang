enum Peano {
    Zero,
    Succ(Box<Peano>),
}

fn add(x: Peano, y: Peano) -> Peano {
    match x {
        Peano::Zero => y,
        Peano::Succ(p) => Peano::Succ(Box::new(add(*p, y))),
    }
}

fn mul(x: Peano, y: Peano) -> Peano {
    match x {
        Peano::Zero => Peano::Zero,
        Peano::Succ(p) => {
            let y_clone = clone_peano(&y);
            add(y, mul(*p, y_clone))
        }
    }
}

fn clone_peano(x: &Peano) -> Peano {
    match x {
        Peano::Zero => Peano::Zero,
        Peano::Succ(p) => Peano::Succ(Box::new(clone_peano(p))),
    }
}

fn to_int(x: &Peano) -> i64 {
    match x {
        Peano::Zero => 0,
        Peano::Succ(p) => 1 + to_int(p),
    }
}

fn from_int(n: i64) -> Peano {
    if n <= 0 {
        Peano::Zero
    } else {
        Peano::Succ(Box::new(from_int(n - 1)))
    }
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..100 {
        let three = from_int(3);
        let four = from_int(4);
        let prod = mul(three, four);
        sum += to_int(&prod);
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
