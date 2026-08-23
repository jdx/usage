// One module for every mise-scale Go shadow, and the only place in the repository
// where another CLI framework is a dependency.
//
// `github.com/jdx/usage/go` has none, deliberately: an adopter's binary carries the
// tables and nothing else. A benchmark that put cobra, urfave/cli or kong in *that*
// go.mod would be measuring the thing it is comparing against while claiming to have
// no dependencies. So the four shadows live here, together, and the sweep that times
// them links all four into one binary — the Go counterpart of `benches/gate`, which
// does the same for the Rust four.
module github.com/jdx/usage/benches/go

go 1.24

require (
	github.com/alecthomas/kong v1.16.1
	github.com/jdx/usage/go v0.0.0
	github.com/spf13/cobra v1.10.2
	github.com/urfave/cli/v3 v3.11.0
)

require (
	github.com/expr-lang/expr v1.17.8 // indirect
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/spf13/pflag v1.0.9 // indirect
)

// The Go module is unreleased, and what is being measured is this checkout of it
// rather than a published version.
replace github.com/jdx/usage/go => ../../go
