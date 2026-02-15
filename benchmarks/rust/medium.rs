struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn format(&self) -> String {
        format!("({:.2}, {:.2})", self.x, self.y)
    }
}

fn build_vec(n: usize) -> Vec<i64> {
    let mut v = Vec::new();
    for i in 0..n {
        v.push(i as i64);
    }
    v
}

fn filter_even(v: &[i64]) -> Vec<i64> {
    let mut result = Vec::new();
    for &x in v {
        if x % 2 == 0 {
            result.push(x);
        }
    }
    result
}

fn sum(v: &[i64]) -> i64 {
    let mut total: i64 = 0;
    for &x in v {
        total += x;
    }
    total
}

fn bubble_sort(v: &mut Vec<i64>) {
    let n = v.len();
    for i in 0..n {
        for j in 0..n - 1 - i {
            if v[j] > v[j + 1] {
                v.swap(j, j + 1);
            }
        }
    }
}

fn describe_number(n: i64) -> &'static str {
    match n % 4 {
        0 => "divisible by 4",
        1 => "remainder 1",
        2 => "remainder 2",
        3 => "remainder 3",
        _ => "unknown",
    }
}

fn make_adder(base: i64) -> Box<dyn Fn(i64) -> i64> {
    Box::new(move |x| base + x)
}

fn format_stats(name: &str, count: usize, total: i64) -> String {
    let avg = if count > 0 {
        total as f64 / count as f64
    } else {
        0.0
    };
    format!("{}: count={}, sum={}, avg={:.2}", name, count, total, avg)
}

fn process_range(start: i64, end: i64) -> Vec<i64> {
    let mut result = Vec::new();
    let mut i = start;
    while i < end {
        if i % 3 != 0 {
            result.push(i * i);
        }
        i += 1;
    }
    result
}

fn main() {
    let p1 = Point::new(0.0, 0.0);
    let p2 = Point::new(3.0, 4.0);
    println!(
        "Distance from {} to {}: {:.2}",
        p1.format(),
        p2.format(),
        p1.distance(&p2)
    );

    let v = build_vec(20);
    let evens = filter_even(&v);
    let total = sum(&evens);
    println!("{}", format_stats("evens", evens.len(), total));

    let mut to_sort = vec![9, 3, 7, 1, 8, 2, 6, 4, 5, 0];
    bubble_sort(&mut to_sort);
    println!("Sorted: {:?}", to_sort);

    for i in 0..8 {
        println!("{}: {}", i, describe_number(i));
    }

    let add5 = make_adder(5);
    let add10 = make_adder(10);
    println!("add5(3) = {}, add10(3) = {}", add5(3), add10(3));

    let squares = process_range(1, 15);
    println!("Squares (non-mult-3): {:?}", squares);
    println!("Sum of squares: {}", sum(&squares));
}
