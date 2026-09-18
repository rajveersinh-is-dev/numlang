struct Point {
    x: f64,
    y: f64,
}

fn dist_sq(p: Point, q: Point) -> f64 {
    let dx: f64 = p.x - q.x;
    let dy: f64 = p.y - q.y;
    return dx * dx + dy * dy;
}

fn main() -> i64 {
    let p: Point = Point { x: 3.0, y: 4.0 };
    let q: Point = Point { x: 0.0, y: 0.0 };
    let d2: f64 = dist_sq(p, q);
    let d: f64 = sqrt(d2);
    println("Point distance calculation:");
    print("Distance: ");
    println(d);
    if d == 5.0 {
        return 0;
    }
    return 1;
}
