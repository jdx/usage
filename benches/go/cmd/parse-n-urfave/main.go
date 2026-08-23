// Command parse-n-urfave resolves the same command line N times, N coming from the
// environment, against urfave's command tree.
//
// The counterpart of `go/internal/bench/parse-n` for one of the frameworks usage-go
// is compared against, and the same protocol: differencing two runs of one binary
// separates what the resolves cost from what the Go runtime costs to start, without
// subtracting a second binary whose startup is not the same size.
//
// What this answers that `cmd/sweep` cannot: instruction counts, which are
// deterministic where wall clock is not, and what a whole process costs — the number
// an adopter feels, most of which is the runtime rather than the parser.
package main

import (
	"fmt"
	"os"
	"strconv"

	miseurfave "github.com/jdx/usage/benches/go/mise-urfave"
)

func main() {
	n := 1
	if v, err := strconv.Atoi(os.Getenv("PARSE_N")); err == nil {
		n = v
	}

	// Printed at the end, and it is what keeps the measurement honest: a rejected
	// command line is cheap to resolve, so a harness that did not check would happily
	// report the cost of failing early.
	seen := 0
	for i := 0; i < n; i++ {
		if miseurfave.Resolve(os.Args[1:]) {
			seen = 1
		}
	}
	fmt.Println(seen)
}
