// Command sweep is wall clock for the four Go parsers, measured so it survives a
// loaded machine.
//
// This is `benches/gate/src/bin/time-sweep.rs` in Go, deliberately: the two cards on
// the landing page should be answering the same question with the same estimator, and
// what a parse costs is a question about parsing rather than about how long a Go
// process takes to start. A whole-process measurement cannot answer it — 0.95 ms of a
// Go process is the runtime coming up, which is three orders of magnitude larger than
// the thing being compared and varies run to run by more than the thing being
// compared costs.
//
// So each parser runs repeatedly in one process and the report is the *fastest*
// per-parse time from many short rounds. Noise from other tenants is additive —
// nothing another process does can make this one faster — so the minimum is the
// estimator to want, and short rounds are the ones an interruption can only spoil
// individually. Each framework gets rounds sized to about the same wall time rather
// than the same iteration count, so a parser 200x slower than another is not asked
// for 200x the work.
//
// Two things this does not measure, both reported elsewhere by `tasks/perf-go.sh`:
// what a whole process costs, which is the number an adopter feels, and the garbage
// collection an allocating parser causes, which a minimum over short rounds mostly
// steps around. The median column is printed beside the minimum because that is where
// collection shows up.
package main

import (
	"flag"
	"fmt"
	"os"
	"runtime"
	"runtime/debug"
	"sort"
	"strings"
	"time"

	"github.com/jdx/usage/benches/go/mise"
	misecobra "github.com/jdx/usage/benches/go/mise-cobra"
	misekong "github.com/jdx/usage/benches/go/mise-kong"
	miseurfave "github.com/jdx/usage/benches/go/mise-urfave"
	"github.com/jdx/usage/go/argv"
)

// The one command line every row is measured against, the same one the Rust shadows
// use, so the two cards describe the same work.
var words = []string{"use", "-g", "node@20"}

// Rounds, and per-round iteration counts chosen so a round is ~0.5-3ms of work.
const rounds = 4000

// sink keeps a parse from being optimized away. Go has no `black_box`, and a compiler
// that can see the result is unused is within its rights to skip producing it.
var sink bool

type stats struct {
	min, p01, p10, median float64
}

// sweep times iters calls of f, rounds times, and describes the distribution per call.
//
// The collector is off for the duration and asked to run between rounds, outside the
// timed interval. Three frameworks here build a model per parse and drop it, so left to
// itself the collector runs inside most rounds and lands unevenly: two runs of this
// program read urfave's minimum as 266 µs and 546 µs, which is not a measurement of
// anything. Off, every round measures the same work.
//
// Excluding it is also the closer answer for a CLI. A process that parses one command
// line and gets on with it usually exits before the collector was ever going to run; what
// the garbage costs shows up in the whole-process table instead, where a real process pays
// for it.
func sweep(rounds, iters int, f func() bool) stats {
	defer debug.SetGCPercent(debug.SetGCPercent(-1))

	// Warm the allocator, the caches, the branch predictors and — for the frameworks
	// that build a model per call — the heap they will keep reusing. Whatever the first
	// call pays for is not what a parse costs on the millionth.
	warm := iters
	if warm < 200 {
		warm = 200
	}
	for i := 0; i < warm; i++ {
		sink = f()
	}

	perCall := make([]float64, 0, rounds)
	for r := 0; r < rounds; r++ {
		// Between rounds, never inside one: with the collector off, whatever the last
		// round allocated is still on the heap, and a round that has to grow it is
		// measuring the allocator's bad day rather than the parser.
		runtime.GC()
		start := time.Now()
		for i := 0; i < iters; i++ {
			sink = f()
		}
		perCall = append(perCall, float64(time.Since(start).Nanoseconds())/float64(iters))
	}
	sort.Float64s(perCall)
	at := func(q float64) float64 {
		return perCall[int(float64(len(perCall)-1)*q)]
	}
	return stats{min: perCall[0], p01: at(0.01), p10: at(0.10), median: at(0.50)}
}

// row is one framework, and how much work to ask it for.
type row struct {
	label  string
	rounds int
	iters  int
	f      func() bool
}

func main() {
	tsv := flag.Bool("tsv", false, "one tab-separated row per parser, for a script to read")
	flag.Parse()

	rows := []row{
		{"usage-go, argv -> struct", rounds, 2_000, func() bool {
			cli, err := mise.Parse(words)
			return err == nil && cli.Use != nil
		}},
		// The binder alone, without the post-binding rules or the structs it fills.
		// Reported because it is the part that is comparable to nothing else here: no
		// other framework has a stage that answers "which token is which" and stops.
		{"usage-go, argv -> events", rounds, 2_000, func() bool {
			p := argv.New(mise.Root, words)
			reached := false
			for p.Next() {
				if ev := p.Event(); ev.Kind == argv.KindCommand {
					reached = true
				}
			}
			return p.Err() == nil && reached
		}},
		{"urfave/cli v3, build tree + run", rounds / 8, 4, func() bool {
			return miseurfave.Resolve(words)
		}},
		{"cobra, build tree + resolve", rounds / 8, 4, func() bool {
			return misecobra.Resolve(words)
		}},
		{"kong, reflect over structs + parse", rounds / 40, 1, func() bool {
			return misekong.Resolve(words)
		}},
	}

	// Every row is checked before any row is timed. A parser that rejected this command
	// line would be cheap for the wrong reason, and a table that reported it anyway
	// would be measuring how fast a framework can fail.
	for _, r := range rows {
		if !r.f() {
			fmt.Fprintf(os.Stderr,
				"sweep: %s did not reach a subcommand on `%s`, so there is nothing worth measuring\n",
				r.label, strings.Join(words, " "))
			os.Exit(1)
		}
	}

	if !*tsv {
		fmt.Printf("%-40s%9s %9s %9s %9s\n", "", "min", "p01", "p10", "median")
	}
	for _, r := range rows {
		s := sweep(r.rounds, r.iters, r.f)
		if *tsv {
			fmt.Printf("%s\t%.0f\t%.0f\t%.0f\t%.0f\n", r.label, s.min, s.p01, s.p10, s.median)
			continue
		}
		fmt.Printf("%-40s%9.0f %9.0f %9.0f %9.0f  ns\n", r.label, s.min, s.p01, s.p10, s.median)
	}
}
