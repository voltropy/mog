package main

import "fmt"

type Point struct {
	X float64
	Y float64
}

func newPoint(x, y float64) Point {
	return Point{X: x, Y: y}
}

func distance(a, b Point) float64 {
	dx := a.X - b.X
	dy := a.Y - b.Y
	// manual sqrt via Newton's method
	val := dx*dx + dy*dy
	if val == 0 {
		return 0
	}
	guess := val / 2
	for i := 0; i < 20; i++ {
		guess = (guess + val/guess) / 2
	}
	return guess
}

func buildSlice(n int) []int {
	result := make([]int, 0, n)
	for i := 0; i < n; i++ {
		result = append(result, i*i)
	}
	return result
}

func filterEven(nums []int) []int {
	result := make([]int, 0)
	for _, v := range nums {
		if v%2 == 0 {
			result = append(result, v)
		}
	}
	return result
}

func sumSlice(nums []int) int {
	total := 0
	for _, v := range nums {
		total += v
	}
	return total
}

func bubbleSort(nums []int) []int {
	sorted := make([]int, len(nums))
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

func classify(n int) string {
	switch {
	case n < 0:
		return "negative"
	case n == 0:
		return "zero"
	case n < 10:
		return "small"
	case n < 100:
		return "medium"
	default:
		return "large"
	}
}

func makeAdder(x int) func(int) int {
	return func(y int) int {
		return x + y
	}
}

func formatPoint(p Point) string {
	return fmt.Sprintf("(%.2f, %.2f)", p.X, p.Y)
}

func main() {
	p1 := newPoint(3.0, 4.0)
	p2 := newPoint(0.0, 0.0)
	fmt.Println(formatPoint(p1))
	fmt.Println(formatPoint(p2))
	fmt.Printf("distance: %.4f\n", distance(p1, p2))

	nums := buildSlice(20)
	evens := filterEven(nums)
	fmt.Println("sum of squares:", sumSlice(nums))
	fmt.Println("sum of even squares:", sumSlice(evens))

	unsorted := []int{64, 34, 25, 12, 22, 11, 90}
	sorted := bubbleSort(unsorted)
	fmt.Println("sorted:", sorted)

	for _, v := range []int{-5, 0, 3, 42, 999} {
		fmt.Printf("%d is %s\n", v, classify(v))
	}

	add5 := makeAdder(5)
	add10 := makeAdder(10)
	fmt.Println("add5(3):", add5(3))
	fmt.Println("add10(7):", add10(7))
}
