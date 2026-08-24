// Command parse-n parses the same command line N times, N coming from the environment,
// through the typed front door the generator emits.
//
// `mise.Parse` rather than the binder alone, because this is the harness whose numbers sit
// in a table beside cobra's, urfave's and kong's, and those three have no stage that stops
// after deciding which token is which. Comparing our cheapest half against their whole is
// how a benchmark flatters its author. `bind-n` is the binder alone, for the rows that are
// about usage-go rather than about the comparison.
//
// Differencing two runs of *this* program separates the parses from the runtime that starts
// before them: N=1000 minus N=0, over a thousand, is what one parse costs. Differencing two
// runs of the same program rather than two different programs is the part worth being
// careful about — two Go binaries do measurably different amounts of work before `main`, and
// that difference would land in whatever you attributed to parsing.
//
// Why amortized rather than N=1 minus N=0, which is what `benches/gate/src/bin/parse-n.rs`
// does for Rust: Go's startup is not deterministic enough. The runtime creates threads,
// starts the collector and varies with what the linker kept, and repeated N=0 runs differ by
// ±50,000 instructions, which is twenty-five times a whole parse here.
package main

import (
	"fmt"
	"os"
	"strconv"

	"github.com/jdx/usage/go/internal/shadow/mise"
)

func main() {
	n := 1
	if v, err := strconv.Atoi(os.Getenv("PARSE_N")); err == nil {
		n = v
	}

	// Printed at the end, and it is what keeps the measurement honest: a rejected
	// command line is cheap to parse, so a harness that did not check would happily
	// report the cost of failing early. The script that drives this refuses to measure a
	// binary that does not say 1.
	seen := 0
	for i := 0; i < n; i++ {
		cli, err := mise.Parse(os.Args[1:])
		if err == nil && cli.Use != nil {
			seen = 1
		}
	}
	// One line, so a shell can read it. Not the count of parses: what matters is whether
	// the last one arrived somewhere, and N is already known to the caller.
	fmt.Println(seen)
}
