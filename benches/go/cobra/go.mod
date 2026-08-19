// A module of its own, so that cobra is not a dependency of `github.com/jdx/usage/go`.
//
// That module has none, deliberately: an adopter's binary carries the tables and nothing
// else. A benchmark that put cobra in its go.mod would be measuring the thing it is
// comparing against while claiming to have no dependencies.
module github.com/jdx/usage/benches/go/cobra

go 1.24

require github.com/spf13/cobra v1.10.1

require (
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/spf13/pflag v1.0.9 // indirect
)
