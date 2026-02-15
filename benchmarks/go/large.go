package main

import "fmt"

// --- Math helpers ---

func abs(x float64) float64 {
	if x < 0 {
		return -x
	}
	return x
}

func sqrt(val float64) float64 {
	if val <= 0 {
		return 0
	}
	guess := val / 2
	for i := 0; i < 30; i++ {
		guess = (guess + val/guess) / 2
	}
	return guess
}

func pow(base float64, exp int) float64 {
	result := 1.0
	neg := false
	if exp < 0 {
		neg = true
		exp = -exp
	}
	for i := 0; i < exp; i++ {
		result *= base
	}
	if neg {
		return 1.0 / result
	}
	return result
}

func minFloat(a, b float64) float64 {
	if a < b {
		return a
	}
	return b
}

func maxFloat(a, b float64) float64 {
	if a > b {
		return a
	}
	return b
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func clamp(val, lo, hi float64) float64 {
	return maxFloat(lo, minFloat(val, hi))
}

// --- Data structures ---

type Vec2 struct {
	X float64
	Y float64
}

func newVec2(x, y float64) Vec2 {
	return Vec2{X: x, Y: y}
}

func addVec2(a, b Vec2) Vec2 {
	return Vec2{X: a.X + b.X, Y: a.Y + b.Y}
}

func subVec2(a, b Vec2) Vec2 {
	return Vec2{X: a.X - b.X, Y: a.Y - b.Y}
}

func scaleVec2(v Vec2, s float64) Vec2 {
	return Vec2{X: v.X * s, Y: v.Y * s}
}

func magVec2(v Vec2) float64 {
	return sqrt(v.X*v.X + v.Y*v.Y)
}

func distVec2(a, b Vec2) float64 {
	return magVec2(subVec2(a, b))
}

func dotVec2(a, b Vec2) float64 {
	return a.X*b.X + a.Y*b.Y
}

func formatVec2(v Vec2) string {
	return fmt.Sprintf("(%.3f, %.3f)", v.X, v.Y)
}

type Stats struct {
	Count    int
	Mean     float64
	Median   float64
	StdDev   float64
	Min      float64
	Max      float64
	Variance float64
	Range    float64
}

type DataPoint struct {
	Label string
	Value float64
	Tag   int
}

type DataSet struct {
	Name   string
	Points []DataPoint
}

// --- Slice utilities ---

func makeRange(start, stop, step int) []int {
	result := make([]int, 0)
	for i := start; i < stop; i += step {
		result = append(result, i)
	}
	return result
}

func mapInts(nums []int, f func(int) int) []int {
	result := make([]int, len(nums))
	for i, v := range nums {
		result[i] = f(v)
	}
	return result
}

func filterInts(nums []int, pred func(int) bool) []int {
	result := make([]int, 0)
	for _, v := range nums {
		if pred(v) {
			result = append(result, v)
		}
	}
	return result
}

func reduceInts(nums []int, init int, f func(int, int) int) int {
	acc := init
	for _, v := range nums {
		acc = f(acc, v)
	}
	return acc
}

func mapFloats(nums []float64, f func(float64) float64) []float64 {
	result := make([]float64, len(nums))
	for i, v := range nums {
		result[i] = f(v)
	}
	return result
}

func filterFloats(nums []float64, pred func(float64) bool) []float64 {
	result := make([]float64, 0)
	for _, v := range nums {
		if pred(v) {
			result = append(result, v)
		}
	}
	return result
}

func sumFloats(nums []float64) float64 {
	total := 0.0
	for _, v := range nums {
		total += v
	}
	return total
}

func sumInts(nums []int) int {
	total := 0
	for _, v := range nums {
		total += v
	}
	return total
}

// --- Sorting ---

func bubbleSortFloats(nums []float64) []float64 {
	sorted := make([]float64, len(nums))
	copy(sorted, nums)
	n := len(sorted)
	for i := 0; i < n; i++ {
		for j := 0; j < n-i-1; j++ {
			if sorted[j] > sorted[j+1] {
				sorted[j], sorted[j+1] = sorted[j+1], sorted[j]
			}
		}
	}
	return sorted
}

func insertionSortInts(nums []int) []int {
	sorted := make([]int, len(nums))
	copy(sorted, nums)
	for i := 1; i < len(sorted); i++ {
		key := sorted[i]
		j := i - 1
		for j >= 0 && sorted[j] > key {
			sorted[j+1] = sorted[j]
			j--
		}
		sorted[j+1] = key
	}
	return sorted
}

func selectionSortFloats(nums []float64) []float64 {
	sorted := make([]float64, len(nums))
	copy(sorted, nums)
	n := len(sorted)
	for i := 0; i < n-1; i++ {
		minIdx := i
		for j := i + 1; j < n; j++ {
			if sorted[j] < sorted[minIdx] {
				minIdx = j
			}
		}
		sorted[i], sorted[minIdx] = sorted[minIdx], sorted[i]
	}
	return sorted
}

// --- Statistics ---

func computeStats(data []float64) Stats {
	n := len(data)
	if n == 0 {
		return Stats{}
	}

	sum := sumFloats(data)
	mean := sum / float64(n)

	sorted := bubbleSortFloats(data)
	minVal := sorted[0]
	maxVal := sorted[n-1]

	var median float64
	if n%2 == 0 {
		median = (sorted[n/2-1] + sorted[n/2]) / 2
	} else {
		median = sorted[n/2]
	}

	varianceSum := 0.0
	for _, v := range data {
		diff := v - mean
		varianceSum += diff * diff
	}
	variance := varianceSum / float64(n)
	stddev := sqrt(variance)

	return Stats{
		Count:    n,
		Mean:     mean,
		Median:   median,
		StdDev:   stddev,
		Min:      minVal,
		Max:      maxVal,
		Variance: variance,
		Range:    maxVal - minVal,
	}
}

func formatStats(s Stats) string {
	return fmt.Sprintf(
		"count=%d mean=%.4f median=%.4f stddev=%.4f min=%.4f max=%.4f var=%.4f range=%.4f",
		s.Count, s.Mean, s.Median, s.StdDev, s.Min, s.Max, s.Variance, s.Range,
	)
}

// --- Data generation ---

func generateData(n int, seed int) []float64 {
	data := make([]float64, n)
	x := seed
	for i := 0; i < n; i++ {
		// simple LCG pseudo-random
		x = (x*1103515245 + 12345) & 0x7fffffff
		data[i] = float64(x%10000) / 100.0
	}
	return data
}

func generateDataPoints(n int, seed int) []DataPoint {
	points := make([]DataPoint, n)
	x := seed
	for i := 0; i < n; i++ {
		x = (x*1103515245 + 12345) & 0x7fffffff
		val := float64(x%10000) / 100.0
		tag := x % 5
		label := classifyValue(val)
		points[i] = DataPoint{Label: label, Value: val, Tag: tag}
	}
	return points
}

func classifyValue(v float64) string {
	switch {
	case v < 10:
		return "tiny"
	case v < 25:
		return "small"
	case v < 50:
		return "medium"
	case v < 75:
		return "large"
	default:
		return "huge"
	}
}

func classifyTag(t int) string {
	switch t {
	case 0:
		return "alpha"
	case 1:
		return "beta"
	case 2:
		return "gamma"
	case 3:
		return "delta"
	case 4:
		return "epsilon"
	default:
		return "unknown"
	}
}

// --- Data processing ---

func extractValues(points []DataPoint) []float64 {
	vals := make([]float64, len(points))
	for i, p := range points {
		vals[i] = p.Value
	}
	return vals
}

func filterByTag(points []DataPoint, tag int) []DataPoint {
	result := make([]DataPoint, 0)
	for _, p := range points {
		if p.Tag == tag {
			result = append(result, p)
		}
	}
	return result
}

func filterByLabel(points []DataPoint, label string) []DataPoint {
	result := make([]DataPoint, 0)
	for _, p := range points {
		if p.Label == label {
			result = append(result, p)
		}
	}
	return result
}

func countByLabel(points []DataPoint) map[string]int {
	counts := make(map[string]int)
	for _, p := range points {
		counts[p.Label]++
	}
	return counts
}

func averageByTag(points []DataPoint) map[int]float64 {
	sums := make(map[int]float64)
	counts := make(map[int]int)
	for _, p := range points {
		sums[p.Tag] += p.Value
		counts[p.Tag]++
	}
	avgs := make(map[int]float64)
	for tag, sum := range sums {
		avgs[tag] = sum / float64(counts[tag])
	}
	return avgs
}

// --- Matrix operations (flat 2D) ---

type Matrix struct {
	Rows int
	Cols int
	Data []float64
}

func newMatrix(rows, cols int) Matrix {
	return Matrix{Rows: rows, Cols: cols, Data: make([]float64, rows*cols)}
}

func matGet(m Matrix, r, c int) float64 {
	return m.Data[r*m.Cols+c]
}

func matSet(m *Matrix, r, c int, val float64) {
	m.Data[r*m.Cols+c] = val
}

func matMul(a, b Matrix) Matrix {
	if a.Cols != b.Rows {
		return newMatrix(0, 0)
	}
	result := newMatrix(a.Rows, b.Cols)
	for i := 0; i < a.Rows; i++ {
		for j := 0; j < b.Cols; j++ {
			sum := 0.0
			for k := 0; k < a.Cols; k++ {
				sum += matGet(a, i, k) * matGet(b, k, j)
			}
			matSet(&result, i, j, sum)
		}
	}
	return result
}

func matAdd(a, b Matrix) Matrix {
	if a.Rows != b.Rows || a.Cols != b.Cols {
		return newMatrix(0, 0)
	}
	result := newMatrix(a.Rows, a.Cols)
	for i := 0; i < a.Rows*a.Cols; i++ {
		result.Data[i] = a.Data[i] + b.Data[i]
	}
	return result
}

func matScale(m Matrix, s float64) Matrix {
	result := newMatrix(m.Rows, m.Cols)
	for i := 0; i < m.Rows*m.Cols; i++ {
		result.Data[i] = m.Data[i] * s
	}
	return result
}

func matTranspose(m Matrix) Matrix {
	result := newMatrix(m.Cols, m.Rows)
	for i := 0; i < m.Rows; i++ {
		for j := 0; j < m.Cols; j++ {
			matSet(&result, j, i, matGet(m, i, j))
		}
	}
	return result
}

func formatMatrix(m Matrix) string {
	s := ""
	for i := 0; i < m.Rows; i++ {
		for j := 0; j < m.Cols; j++ {
			if j > 0 {
				s += " "
			}
			s += fmt.Sprintf("%.2f", matGet(m, i, j))
		}
		s += "\n"
	}
	return s
}

func identityMatrix(n int) Matrix {
	m := newMatrix(n, n)
	for i := 0; i < n; i++ {
		matSet(&m, i, i, 1.0)
	}
	return m
}

// --- String operations ---

func repeatStr(s string, n int) string {
	result := ""
	for i := 0; i < n; i++ {
		result += s
	}
	return result
}

func reverseStr(s string) string {
	runes := []rune(s)
	for i, j := 0, len(runes)-1; i < j; i, j = i+1, j-1 {
		runes[i], runes[j] = runes[j], runes[i]
	}
	return string(runes)
}

func isPalindrome(s string) bool {
	return s == reverseStr(s)
}

func countChars(s string) map[rune]int {
	counts := make(map[rune]int)
	for _, c := range s {
		counts[c]++
	}
	return counts
}

func caesarCipher(s string, shift int) string {
	result := make([]rune, len(s))
	for i, c := range s {
		if c >= 'a' && c <= 'z' {
			result[i] = rune('a' + (int(c-'a')+shift)%26)
		} else if c >= 'A' && c <= 'Z' {
			result[i] = rune('A' + (int(c-'A')+shift)%26)
		} else {
			result[i] = c
		}
	}
	return string(result)
}

// --- Fibonacci and combinatorics ---

func fibIterative(n int) int {
	if n <= 1 {
		return n
	}
	a, b := 0, 1
	for i := 2; i <= n; i++ {
		a, b = b, a+b
	}
	return b
}

func factorial(n int) int {
	result := 1
	for i := 2; i <= n; i++ {
		result *= i
	}
	return result
}

func combination(n, k int) int {
	if k > n {
		return 0
	}
	if k == 0 || k == n {
		return 1
	}
	k = minInt(k, n-k)
	result := 1
	for i := 0; i < k; i++ {
		result = result * (n - i) / (i + 1)
	}
	return result
}

func permutation(n, k int) int {
	if k > n {
		return 0
	}
	result := 1
	for i := 0; i < k; i++ {
		result *= (n - i)
	}
	return result
}

func gcd(a, b int) int {
	for b != 0 {
		a, b = b, a%b
	}
	return a
}

func lcm(a, b int) int {
	return a / gcd(a, b) * b
}

func isPrime(n int) bool {
	if n < 2 {
		return false
	}
	for i := 2; i*i <= n; i++ {
		if n%i == 0 {
			return false
		}
	}
	return true
}

func primesUpTo(n int) []int {
	primes := make([]int, 0)
	for i := 2; i <= n; i++ {
		if isPrime(i) {
			primes = append(primes, i)
		}
	}
	return primes
}

// --- Histogram ---

func histogram(data []float64, bins int) []int {
	if len(data) == 0 || bins <= 0 {
		return nil
	}
	sorted := bubbleSortFloats(data)
	minVal := sorted[0]
	maxVal := sorted[len(sorted)-1]
	rangeVal := maxVal - minVal
	if rangeVal == 0 {
		counts := make([]int, bins)
		counts[0] = len(data)
		return counts
	}
	binWidth := rangeVal / float64(bins)
	counts := make([]int, bins)
	for _, v := range data {
		idx := int((v - minVal) / binWidth)
		if idx >= bins {
			idx = bins - 1
		}
		counts[idx]++
	}
	return counts
}

func printHistogram(counts []int, width int) {
	maxCount := 0
	for _, c := range counts {
		if c > maxCount {
			maxCount = c
		}
	}
	for i, c := range counts {
		barLen := 0
		if maxCount > 0 {
			barLen = c * width / maxCount
		}
		bar := repeatStr("#", barLen)
		fmt.Printf("bin %2d [%3d]: %s\n", i, c, bar)
	}
}

// --- Closures and higher-order functions ---

func makeMultiplier(factor float64) func(float64) float64 {
	return func(x float64) float64 {
		return x * factor
	}
}

func makeAccumulator(initial float64) func(float64) float64 {
	sum := initial
	return func(x float64) float64 {
		sum += x
		return sum
	}
}

func compose(f, g func(float64) float64) func(float64) float64 {
	return func(x float64) float64 {
		return f(g(x))
	}
}

func applyN(f func(float64) float64, x float64, n int) float64 {
	result := x
	for i := 0; i < n; i++ {
		result = f(result)
	}
	return result
}

// --- Running analysis ---

func runVectorTests() {
	fmt.Println("=== Vector Tests ===")
	v1 := newVec2(3, 4)
	v2 := newVec2(1, 2)
	fmt.Println("v1:", formatVec2(v1))
	fmt.Println("v2:", formatVec2(v2))
	fmt.Println("v1+v2:", formatVec2(addVec2(v1, v2)))
	fmt.Println("v1-v2:", formatVec2(subVec2(v1, v2)))
	fmt.Printf("|v1|: %.4f\n", magVec2(v1))
	fmt.Printf("dist: %.4f\n", distVec2(v1, v2))
	fmt.Printf("dot: %.4f\n", dotVec2(v1, v2))
	fmt.Println("scaled:", formatVec2(scaleVec2(v1, 2.5)))

	points := []Vec2{
		newVec2(0, 0), newVec2(1, 1), newVec2(2, 4),
		newVec2(3, 9), newVec2(4, 16),
	}
	totalDist := 0.0
	for i := 1; i < len(points); i++ {
		totalDist += distVec2(points[i-1], points[i])
	}
	fmt.Printf("path length: %.4f\n", totalDist)
	fmt.Println()
}

func runStatsTests() {
	fmt.Println("=== Statistics Tests ===")
	seeds := []int{42, 137, 256}
	sizes := []int{50, 100, 200}

	for i, seed := range seeds {
		data := generateData(sizes[i], seed)
		stats := computeStats(data)
		fmt.Printf("dataset %d (n=%d): %s\n", i, sizes[i], formatStats(stats))

		hist := histogram(data, 10)
		fmt.Printf("histogram (seed=%d):\n", seed)
		printHistogram(hist, 40)
		fmt.Println()
	}
}

func runDataProcessing() {
	fmt.Println("=== Data Processing ===")
	points := generateDataPoints(100, 99)

	labelCounts := countByLabel(points)
	labels := []string{"tiny", "small", "medium", "large", "huge"}
	for _, label := range labels {
		fmt.Printf("  %s: %d\n", label, labelCounts[label])
	}

	tagAvgs := averageByTag(points)
	for tag := 0; tag < 5; tag++ {
		fmt.Printf("  tag %d (%s): avg=%.4f\n", tag, classifyTag(tag), tagAvgs[tag])
	}

	for tag := 0; tag < 3; tag++ {
		filtered := filterByTag(points, tag)
		vals := extractValues(filtered)
		stats := computeStats(vals)
		fmt.Printf("  tag %d stats: %s\n", tag, formatStats(stats))
	}

	for _, label := range []string{"small", "large"} {
		filtered := filterByLabel(points, label)
		vals := extractValues(filtered)
		if len(vals) > 0 {
			stats := computeStats(vals)
			fmt.Printf("  label '%s' stats: %s\n", label, formatStats(stats))
		}
	}
	fmt.Println()
}

func runMatrixTests() {
	fmt.Println("=== Matrix Tests ===")
	a := newMatrix(3, 3)
	b := newMatrix(3, 3)
	val := 1.0
	for i := 0; i < 3; i++ {
		for j := 0; j < 3; j++ {
			matSet(&a, i, j, val)
			matSet(&b, i, j, val*2)
			val++
		}
	}
	fmt.Println("A:")
	fmt.Print(formatMatrix(a))
	fmt.Println("B:")
	fmt.Print(formatMatrix(b))
	fmt.Println("A*B:")
	fmt.Print(formatMatrix(matMul(a, b)))
	fmt.Println("A+B:")
	fmt.Print(formatMatrix(matAdd(a, b)))
	fmt.Println("A^T:")
	fmt.Print(formatMatrix(matTranspose(a)))
	fmt.Println("A*2.5:")
	fmt.Print(formatMatrix(matScale(a, 2.5)))
	fmt.Println("I(3):")
	fmt.Print(formatMatrix(identityMatrix(3)))
	fmt.Println()
}

func runStringTests() {
	fmt.Println("=== String Tests ===")
	words := []string{"hello", "racecar", "world", "madam", "openai", "level"}
	for _, w := range words {
		fmt.Printf("  %s reversed=%s palindrome=%t\n", w, reverseStr(w), isPalindrome(w))
	}

	msg := "The Quick Brown Fox"
	for _, shift := range []int{1, 3, 13, 25} {
		encrypted := caesarCipher(msg, shift)
		decrypted := caesarCipher(encrypted, 26-shift)
		fmt.Printf("  shift=%2d: '%s' -> '%s' (matches=%t)\n", shift, encrypted, decrypted, decrypted == msg)
	}

	charCounts := countChars("abracadabra")
	fmt.Print("  char counts for 'abracadabra': ")
	for _, c := range "abcdr" {
		fmt.Printf("%c=%d ", c, charCounts[c])
	}
	fmt.Println()
	fmt.Println()
}

func runNumberTheory() {
	fmt.Println("=== Number Theory ===")
	fmt.Println("fibonacci:")
	for i := 0; i <= 20; i++ {
		fmt.Printf("  fib(%d) = %d\n", i, fibIterative(i))
	}

	fmt.Println("factorials:")
	for i := 0; i <= 12; i++ {
		fmt.Printf("  %d! = %d\n", i, factorial(i))
	}

	fmt.Println("combinations and permutations:")
	for n := 5; n <= 10; n += 5 {
		for k := 0; k <= n; k += 2 {
			fmt.Printf("  C(%d,%d)=%d P(%d,%d)=%d\n", n, k, combination(n, k), n, k, permutation(n, k))
		}
	}

	fmt.Println("gcd and lcm:")
	pairs := [][2]int{{12, 8}, {15, 25}, {100, 75}, {17, 13}}
	for _, p := range pairs {
		fmt.Printf("  gcd(%d,%d)=%d lcm(%d,%d)=%d\n", p[0], p[1], gcd(p[0], p[1]), p[0], p[1], lcm(p[0], p[1]))
	}

	primes := primesUpTo(100)
	fmt.Printf("primes up to 100 (%d total): ", len(primes))
	for i, p := range primes {
		if i > 0 {
			fmt.Print(", ")
		}
		fmt.Print(p)
	}
	fmt.Println()
	fmt.Println()
}

func runClosureTests() {
	fmt.Println("=== Closure Tests ===")
	double := makeMultiplier(2.0)
	triple := makeMultiplier(3.0)
	half := makeMultiplier(0.5)

	for _, x := range []float64{1, 5, 10, 100} {
		fmt.Printf("  x=%.0f: double=%.1f triple=%.1f half=%.1f\n", x, double(x), triple(x), half(x))
	}

	acc := makeAccumulator(0)
	for _, v := range []float64{10, 20, 30, 40, 50} {
		fmt.Printf("  accumulate %.0f -> %.1f\n", v, acc(v))
	}

	doubleAndAdd1 := compose(func(x float64) float64 { return x + 1 }, double)
	fmt.Printf("  compose(x+1, double)(5) = %.1f\n", doubleAndAdd1(5))

	fmt.Printf("  applyN(double, 1, 10) = %.1f\n", applyN(double, 1, 10))
	fmt.Println()
}

func runSortingTests() {
	fmt.Println("=== Sorting Tests ===")
	intData := []int{64, 25, 12, 22, 11, 90, 34, 55, 77, 1, 99, 42}
	floatData := []float64{3.14, 2.71, 1.41, 1.73, 0.58, 2.23, 0.69}

	fmt.Println("insertion sort:", insertionSortInts(intData))
	fmt.Printf("bubble sort: %v\n", bubbleSortFloats(floatData))
	fmt.Printf("selection sort: %v\n", selectionSortFloats(floatData))

	// functional operations
	nums := makeRange(1, 21, 1)
	squared := mapInts(nums, func(x int) int { return x * x })
	evens := filterInts(nums, func(x int) bool { return x%2 == 0 })
	sumAll := reduceInts(nums, 0, func(a, b int) int { return a + b })
	product := reduceInts(makeRange(1, 11, 1), 1, func(a, b int) int { return a * b })

	fmt.Println("  range 1..20:", nums)
	fmt.Println("  squared:", squared)
	fmt.Println("  evens:", evens)
	fmt.Println("  sum:", sumAll)
	fmt.Println("  product 1..10:", product)
	fmt.Println()
}

func runMathTests() {
	fmt.Println("=== Math Tests ===")
	for _, v := range []float64{0, 1, 2, 4, 9, 16, 25, 100, 144} {
		fmt.Printf("  sqrt(%.0f) = %.6f\n", v, sqrt(v))
	}
	for _, base := range []float64{2, 3, 10} {
		for _, exp := range []int{0, 1, 2, 5, 10} {
			fmt.Printf("  %.0f^%d = %.2f\n", base, exp, pow(base, exp))
		}
	}
	for _, v := range []float64{-5.5, 0, 3.7, 100.1, -0.01} {
		fmt.Printf("  clamp(%.2f, 0, 10) = %.2f\n", v, clamp(v, 0, 10))
	}
	fmt.Println()
}

func main() {
	runVectorTests()
	runStatsTests()
	runDataProcessing()
	runMatrixTests()
	runStringTests()
	runNumberTheory()
	runClosureTests()
	runSortingTests()
	runMathTests()
	fmt.Println("=== All benchmarks complete ===")
}
