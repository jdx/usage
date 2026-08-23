// Command bind-n binds the same command line N times, N coming from the environment,
// stopping at the events rather than going on to fill a struct.
//
// The same protocol as `parse-n` and the same reasons for it; what differs is where it
// stops. This is the half of usage-go that has no counterpart in the frameworks it is
// compared against — cobra, urfave and kong each have one entry point that resolves and
// converts together — so it is reported as a row about usage-go rather than as a row in the
// comparison, and its binary is what a CLI that only binds actually carries.
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
	if seen > 0 {
		fmt.Println(1)
		return
	}
	fmt.Println(0)
}
