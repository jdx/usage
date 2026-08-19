// Command parse-n binds the same command line N times, N coming from the
// environment.
//
// Differencing two runs of *this* program separates the first bind from the ones
// after it: N=1 minus N=0 is what a cold parse costs in a fresh process, and N=2
// minus N=1 is what each costs with the caches warm. A CLI only ever does the
// first, which is why the cold number is the one that matters.
//
// Differencing two runs of the same program rather than two different programs is
// the part worth being careful about. Subtracting a separate do-nothing binary
// looks equivalent and is not: two Go binaries do measurably different amounts of
// work before `main` — the runtime's own startup is ~950,000 instructions and
// varies with what the linker kept — and that difference lands in whatever you
// attribute to parsing. Holding the binary fixed and varying only how many binds
// it does leaves nothing else to explain.
//
// This is the counterpart of `benches/gate/src/bin/parse-n.rs`, which measures the
// Rust side the same way. Same protocol, so the numbers can sit in one table.
package main

import (
	"fmt"
	"os"
	"strconv"

	"github.com/jdx/usage/go/argv"
	"github.com/jdx/usage/go/internal/shadow/mise"
)

func main() {
	n := 1
	if v, err := strconv.Atoi(os.Getenv("PARSE_N")); err == nil {
		n = v
	}

	// Printed at the end, and it is what keeps the measurement honest: a rejected
	// command line is cheap to bind, so a harness that did not check would happily
	// report the cost of failing early. The script that drives this refuses to
	// measure a binary that does not say 1.
	seen := 0
	for i := 0; i < n; i++ {
		p := argv.New(mise.Root, os.Args[1:])
		reached := 0
		for p.Next() {
			if ev := p.Event(); ev.Kind == argv.KindCommand {
				reached = 1
			}
		}
		if p.Err() == nil {
			seen += reached
		}
	}
	// One line, so a shell can read it. Not the count of binds: what matters is
	// whether the last one arrived somewhere, and N is already known to the caller.
	if seen > 0 {
		fmt.Println(1)
		return
	}
	fmt.Println(0)
}
