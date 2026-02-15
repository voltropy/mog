// large.rs — Mini statistics / data processor benchmark (~500 lines)

// ── Structs ──────────────────────────────────────────────────────────────────

struct Stats {
    count: usize,
    sum: f64,
    min: f64,
    max: f64,
    mean: f64,
    variance: f64,
    stddev: f64,
}

struct DataPoint {
    label: String,
    value: f64,
    category: String,
}

struct Bucket {
    low: f64,
    high: f64,
    count: usize,
}

struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

struct RunningStats {
    n: usize,
    mean: f64,
    m2: f64,
}

// ── Stats helpers ────────────────────────────────────────────────────────────

fn compute_stats(values: &[f64]) -> Stats {
    if values.is_empty() {
        return Stats {
            count: 0,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            variance: 0.0,
            stddev: 0.0,
        };
    }
    let mut s = 0.0;
    let mut mn = values[0];
    let mut mx = values[0];
    for &v in values {
        s += v;
        if v < mn {
            mn = v;
        }
        if v > mx {
            mx = v;
        }
    }
    let mean = s / values.len() as f64;
    let mut var_sum = 0.0;
    for &v in values {
        let d = v - mean;
        var_sum += d * d;
    }
    let variance = var_sum / values.len() as f64;
    Stats {
        count: values.len(),
        sum: s,
        min: mn,
        max: mx,
        mean,
        variance,
        stddev: variance.sqrt(),
    }
}

fn format_stats(st: &Stats) -> String {
    format!(
        "n={} sum={:.4} min={:.4} max={:.4} mean={:.4} std={:.4}",
        st.count, st.sum, st.min, st.max, st.mean, st.stddev
    )
}

fn running_stats_new() -> RunningStats {
    RunningStats {
        n: 0,
        mean: 0.0,
        m2: 0.0,
    }
}

fn running_stats_push(rs: &mut RunningStats, x: f64) {
    rs.n += 1;
    let delta = x - rs.mean;
    rs.mean += delta / rs.n as f64;
    let delta2 = x - rs.mean;
    rs.m2 += delta * delta2;
}

fn running_stats_variance(rs: &RunningStats) -> f64 {
    if rs.n < 2 {
        return 0.0;
    }
    rs.m2 / rs.n as f64
}

// ── Histogram ────────────────────────────────────────────────────────────────

fn build_histogram(values: &[f64], num_buckets: usize) -> Vec<Bucket> {
    if values.is_empty() || num_buckets == 0 {
        return Vec::new();
    }
    let mut mn = values[0];
    let mut mx = values[0];
    for &v in values {
        if v < mn {
            mn = v;
        }
        if v > mx {
            mx = v;
        }
    }
    let range = mx - mn;
    let width = if range == 0.0 {
        1.0
    } else {
        range / num_buckets as f64
    };
    let mut buckets = Vec::new();
    for i in 0..num_buckets {
        buckets.push(Bucket {
            low: mn + i as f64 * width,
            high: mn + (i + 1) as f64 * width,
            count: 0,
        });
    }
    for &v in values {
        let mut idx = ((v - mn) / width) as usize;
        if idx >= num_buckets {
            idx = num_buckets - 1;
        }
        buckets[idx].count += 1;
    }
    buckets
}

fn print_histogram(buckets: &[Bucket]) {
    for b in buckets {
        let bar: String = std::iter::repeat('#').take(b.count).collect();
        println!("  [{:>8.2}, {:>8.2}) | {}", b.low, b.high, bar);
    }
}

// ── Matrix operations ────────────────────────────────────────────────────────

fn matrix_new(rows: usize, cols: usize) -> Matrix {
    Matrix {
        rows,
        cols,
        data: vec![0.0; rows * cols],
    }
}

fn matrix_get(m: &Matrix, r: usize, c: usize) -> f64 {
    m.data[r * m.cols + c]
}

fn matrix_set(m: &mut Matrix, r: usize, c: usize, v: f64) {
    m.data[r * m.cols + c] = v;
}

fn matrix_multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut result = matrix_new(a.rows, b.cols);
    for i in 0..a.rows {
        for j in 0..b.cols {
            let mut sum = 0.0;
            for k in 0..a.cols {
                sum += matrix_get(a, i, k) * matrix_get(b, k, j);
            }
            matrix_set(&mut result, i, j, sum);
        }
    }
    result
}

fn matrix_transpose(m: &Matrix) -> Matrix {
    let mut result = matrix_new(m.cols, m.rows);
    for i in 0..m.rows {
        for j in 0..m.cols {
            matrix_set(&mut result, j, i, matrix_get(m, i, j));
        }
    }
    result
}

fn matrix_row_sums(m: &Matrix) -> Vec<f64> {
    let mut sums = Vec::new();
    for i in 0..m.rows {
        let mut s = 0.0;
        for j in 0..m.cols {
            s += matrix_get(m, i, j);
        }
        sums.push(s);
    }
    sums
}

fn matrix_col_sums(m: &Matrix) -> Vec<f64> {
    let mut sums = vec![0.0; m.cols];
    for i in 0..m.rows {
        for j in 0..m.cols {
            sums[j] += matrix_get(m, i, j);
        }
    }
    sums
}

fn matrix_format(m: &Matrix) -> String {
    let mut s = String::new();
    for i in 0..m.rows {
        for j in 0..m.cols {
            if j > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("{:>8.2}", matrix_get(m, i, j)));
        }
        s.push('\n');
    }
    s
}

// ── Data generation & categorisation ─────────────────────────────────────────

fn generate_data(n: usize) -> Vec<DataPoint> {
    let mut data = Vec::new();
    for i in 0..n {
        let x = i as f64;
        let value = (x * 0.7).sin() * 100.0 + (x * 0.3).cos() * 50.0;
        let category = match i % 5 {
            0 => "alpha",
            1 => "beta",
            2 => "gamma",
            3 => "delta",
            _ => "epsilon",
        };
        data.push(DataPoint {
            label: format!("point_{:04}", i),
            value,
            category: category.to_string(),
        });
    }
    data
}

fn filter_by_category(data: &[DataPoint], cat: &str) -> Vec<f64> {
    let mut result = Vec::new();
    for dp in data {
        if dp.category == cat {
            result.push(dp.value);
        }
    }
    result
}

fn extract_values(data: &[DataPoint]) -> Vec<f64> {
    let mut vals = Vec::new();
    for dp in data {
        vals.push(dp.value);
    }
    vals
}

fn classify_value(v: f64) -> &'static str {
    if v < -75.0 {
        "very_low"
    } else if v < -25.0 {
        "low"
    } else if v < 25.0 {
        "medium"
    } else if v < 75.0 {
        "high"
    } else {
        "very_high"
    }
}

fn count_classifications(values: &[f64]) -> Vec<(String, usize)> {
    let labels = ["very_low", "low", "medium", "high", "very_high"];
    let mut counts = vec![0usize; labels.len()];
    for &v in values {
        let cls = classify_value(v);
        for (i, &lbl) in labels.iter().enumerate() {
            if cls == lbl {
                counts[i] += 1;
                break;
            }
        }
    }
    let mut result = Vec::new();
    for (i, &lbl) in labels.iter().enumerate() {
        if counts[i] > 0 {
            result.push((lbl.to_string(), counts[i]));
        }
    }
    result
}

// ── Sorting ──────────────────────────────────────────────────────────────────

fn insertion_sort(v: &mut Vec<f64>) {
    let n = v.len();
    for i in 1..n {
        let key = v[i];
        let mut j = i;
        while j > 0 && v[j - 1] > key {
            v[j] = v[j - 1];
            j -= 1;
        }
        v[j] = key;
    }
}

fn selection_sort(v: &mut Vec<f64>) {
    let n = v.len();
    for i in 0..n {
        let mut min_idx = i;
        for j in (i + 1)..n {
            if v[j] < v[min_idx] {
                min_idx = j;
            }
        }
        v.swap(i, min_idx);
    }
}

fn is_sorted(v: &[f64]) -> bool {
    for i in 1..v.len() {
        if v[i] < v[i - 1] {
            return false;
        }
    }
    true
}

// ── String operations ────────────────────────────────────────────────────────

fn repeat_string(s: &str, n: usize) -> String {
    let mut result = String::new();
    for _ in 0..n {
        result.push_str(s);
    }
    result
}

fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}

fn count_chars(s: &str) -> Vec<(char, usize)> {
    let mut counts: Vec<(char, usize)> = Vec::new();
    for c in s.chars() {
        let mut found = false;
        for entry in counts.iter_mut() {
            if entry.0 == c {
                entry.1 += 1;
                found = true;
                break;
            }
        }
        if !found {
            counts.push((c, 1));
        }
    }
    counts
}

fn caesar_cipher(s: &str, shift: u8) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c.is_ascii_lowercase() {
            let shifted = ((c as u8 - b'a' + shift) % 26 + b'a') as char;
            result.push(shifted);
        } else if c.is_ascii_uppercase() {
            let shifted = ((c as u8 - b'A' + shift) % 26 + b'A') as char;
            result.push(shifted);
        } else {
            result.push(c);
        }
    }
    result
}

fn run_length_encode(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let mut count = 1;
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            count += 1;
        } else {
            if count > 1 {
                result.push_str(&format!("{}{}", count, chars[i - 1]));
            } else {
                result.push(chars[i - 1]);
            }
            count = 1;
        }
    }
    if count > 1 {
        result.push_str(&format!("{}{}", count, chars[chars.len() - 1]));
    } else {
        result.push(chars[chars.len() - 1]);
    }
    result
}

// ── Math utilities ───────────────────────────────────────────────────────────

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}

fn lcm(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    (a / gcd(a, b) * b).abs()
}

fn is_prime(n: i64) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i: i64 = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

fn primes_up_to(limit: i64) -> Vec<i64> {
    let mut result = Vec::new();
    for n in 2..=limit {
        if is_prime(n) {
            result.push(n);
        }
    }
    result
}

fn fibonacci_iterative(n: usize) -> Vec<i64> {
    let mut fibs = Vec::new();
    if n == 0 {
        return fibs;
    }
    fibs.push(0);
    if n == 1 {
        return fibs;
    }
    fibs.push(1);
    for i in 2..n {
        let next = fibs[i - 1] + fibs[i - 2];
        fibs.push(next);
    }
    fibs
}

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0;
    let len = if a.len() < b.len() { a.len() } else { b.len() };
    for i in 0..len {
        sum += a[i] * b[i];
    }
    sum
}

fn normalize(v: &[f64]) -> Vec<f64> {
    let mag = dot_product(v, v).sqrt();
    if mag == 0.0 {
        return vec![0.0; v.len()];
    }
    let mut result = Vec::new();
    for &x in v {
        result.push(x / mag);
    }
    result
}

fn linspace(start: f64, end: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![start];
    }
    let step = (end - start) / (n - 1) as f64;
    let mut result = Vec::new();
    for i in 0..n {
        result.push(start + i as f64 * step);
    }
    result
}

fn polynomial_eval(coeffs: &[f64], x: f64) -> f64 {
    let mut result = 0.0;
    let mut power = 1.0;
    for &c in coeffs {
        result += c * power;
        power *= x;
    }
    result
}

// ── Percentile / median ─────────────────────────────────────────────────────

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

fn median(values: &mut Vec<f64>) -> f64 {
    insertion_sort(values);
    percentile(values, 50.0)
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    println!("=== Large Rust Benchmark: Data Processor ===\n");

    // Generate data
    let data = generate_data(200);
    let all_values = extract_values(&data);
    println!("Generated {} data points", data.len());

    // Overall stats
    let st = compute_stats(&all_values);
    println!("Overall: {}", format_stats(&st));

    // Running stats comparison
    let mut rs = running_stats_new();
    for &v in &all_values {
        running_stats_push(&mut rs, v);
    }
    println!(
        "Running: mean={:.4} var={:.4}",
        rs.mean,
        running_stats_variance(&rs)
    );

    // Per-category breakdown
    println!("\n--- Per-Category Stats ---");
    let categories = ["alpha", "beta", "gamma", "delta", "epsilon"];
    for &cat in &categories {
        let vals = filter_by_category(&data, cat);
        let cst = compute_stats(&vals);
        println!("  {:<8} {}", cat, format_stats(&cst));
    }

    // Classification counts
    println!("\n--- Value Classifications ---");
    let class_counts = count_classifications(&all_values);
    for (label, count) in &class_counts {
        println!("  {:<10} {}", label, count);
    }

    // Histogram
    println!("\n--- Histogram (10 buckets) ---");
    let hist = build_histogram(&all_values, 10);
    print_histogram(&hist);

    // Sorting
    println!("\n--- Sorting ---");
    let mut v1 = all_values.clone();
    insertion_sort(&mut v1);
    println!("Insertion sort: sorted={}", is_sorted(&v1));

    let mut v2 = all_values.clone();
    selection_sort(&mut v2);
    println!("Selection sort: sorted={}", is_sorted(&v2));

    // Percentiles
    let mut pvals = all_values.clone();
    insertion_sort(&mut pvals);
    println!(
        "Percentiles: p25={:.2} p50={:.2} p75={:.2} p90={:.2} p99={:.2}",
        percentile(&pvals, 25.0),
        percentile(&pvals, 50.0),
        percentile(&pvals, 75.0),
        percentile(&pvals, 90.0),
        percentile(&pvals, 99.0),
    );

    // Median
    let mut med_vals = all_values.clone();
    println!("Median: {:.4}", median(&mut med_vals));

    // Matrix operations
    println!("\n--- Matrix Operations ---");
    let size = 5;
    let mut a = matrix_new(size, size);
    let mut b = matrix_new(size, size);
    for i in 0..size {
        for j in 0..size {
            matrix_set(&mut a, i, j, (i * size + j + 1) as f64);
            matrix_set(&mut b, i, j, if i == j { 1.0 } else { 0.5 });
        }
    }
    println!("Matrix A:\n{}", matrix_format(&a));
    println!("Matrix B:\n{}", matrix_format(&b));

    let c = matrix_multiply(&a, &b);
    println!("A x B:\n{}", matrix_format(&c));

    let at = matrix_transpose(&a);
    println!("A^T:\n{}", matrix_format(&at));

    let row_sums = matrix_row_sums(&c);
    let col_sums = matrix_col_sums(&c);
    println!("Row sums of A*B: {:?}", row_sums);
    println!("Col sums of A*B: {:?}", col_sums);

    // String operations
    println!("\n--- String Operations ---");
    let test_str = "Hello, Benchmark World!";
    println!("Original:  {}", test_str);
    println!("Reversed:  {}", reverse_string(test_str));
    println!("Repeated:  {}", repeat_string("ab", 5));

    let cipher_text = caesar_cipher(test_str, 3);
    let deciphered = caesar_cipher(&cipher_text, 23);
    println!("Caesar +3: {}", cipher_text);
    println!("Caesar -3: {}", deciphered);

    let rle_input = "aaabbbccdddddeef";
    println!("RLE input:  {}", rle_input);
    println!("RLE output: {}", run_length_encode(rle_input));

    let char_counts = count_chars("abracadabra");
    println!("Char counts of 'abracadabra':");
    for (ch, cnt) in &char_counts {
        println!("  '{}': {}", ch, cnt);
    }

    // Math utilities
    println!("\n--- Math ---");
    println!("gcd(48, 18) = {}", gcd(48, 18));
    println!("lcm(12, 8) = {}", lcm(12, 8));

    let primes = primes_up_to(50);
    println!("Primes up to 50: {:?}", primes);

    let fibs = fibonacci_iterative(20);
    println!("First 20 Fibonacci: {:?}", fibs);

    let a_vec = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let b_vec = vec![5.0, 4.0, 3.0, 2.0, 1.0];
    println!(
        "dot({:?}, {:?}) = {:.2}",
        a_vec,
        b_vec,
        dot_product(&a_vec, &b_vec)
    );

    let norm = normalize(&a_vec);
    println!("normalize({:?}) = {:?}", a_vec, norm);
    println!(
        "magnitude after normalize = {:.6}",
        dot_product(&norm, &norm).sqrt()
    );

    let ls = linspace(0.0, 10.0, 11);
    println!("linspace(0,10,11): {:?}", ls);

    let coeffs = vec![1.0, -2.0, 3.0, -1.0]; // 1 - 2x + 3x^2 - x^3
    println!(
        "poly [1,-2,3,-1] at x=2: {:.2}",
        polynomial_eval(&coeffs, 2.0)
    );
    println!(
        "poly [1,-2,3,-1] at x=0: {:.2}",
        polynomial_eval(&coeffs, 0.0)
    );
    println!(
        "poly [1,-2,3,-1] at x=1: {:.2}",
        polynomial_eval(&coeffs, 1.0)
    );

    // Complex nested processing
    println!("\n--- Nested Processing ---");
    let mut grand_total = 0.0;
    for cat in &categories {
        let vals = filter_by_category(&data, cat);
        for &v in &vals {
            let cls = classify_value(v);
            let weight = match cls {
                "very_low" => 0.5,
                "low" => 0.75,
                "medium" => 1.0,
                "high" => 1.25,
                "very_high" => 1.5,
                _ => 1.0,
            };
            grand_total += v * weight;
        }
    }
    println!("Weighted grand total: {:.4}", grand_total);

    // Summary
    println!("\n=== Benchmark Complete ===");
}
