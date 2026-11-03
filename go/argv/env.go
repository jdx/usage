package argv

import (
	"os"
	"runtime"
	"slices"
	"strings"
)

// UnknownEnv is the variables in names that start with prefix and are not in
// known, sorted and each once.
//
// A spec's `unknown_env` asks for this: a misspelled or stale variable under the
// CLI's own prefix is read by nobody, and the program runs on its defaults. known
// is every name the spec declares anywhere in its command tree, and its allow
// list; the generator collects it, since the whole tree counts and not only the
// command that ran — a variable meant for `serve` is not a typo when the user runs
// `publish`.
//
// names is passed in rather than read from the process for the reason [Fill]
// takes its lookup: a test of a parse should not be a test of the machine it runs
// on. [EnvNames] reads the process. On Windows, where variable names are
// case-insensitive, so is the comparison.
func UnknownEnv(prefix string, known, names []string) []string {
	var unknown []string
	for _, name := range names {
		if len(name) < len(prefix) || !sameEnv(name[:len(prefix)], prefix) {
			continue
		}
		if slices.ContainsFunc(known, func(k string) bool { return sameEnv(k, name) }) {
			continue
		}
		unknown = append(unknown, name)
	}
	slices.Sort(unknown)
	return slices.Compact(unknown)
}

// RefuseUnknownEnv is [UnknownEnv] as the error a parse returns, with every
// offending variable named at once: a deployment with two typos should not need
// two restarts to find them. It returns nil when there are none.
func RefuseUnknownEnv(prefix string, known, names []string) error {
	unknown := UnknownEnv(prefix, known, names)
	if len(unknown) == 0 {
		return nil
	}
	return &Error{Code: CodeUnknownEnv, Name: prefix, Names: unknown}
}

// EnvNames is the names of the process environment, for callers that want it.
func EnvNames() []string {
	environ := os.Environ()
	names := make([]string, 0, len(environ))
	for _, entry := range environ {
		// Windows keeps per-drive working directories as `=C:=C:\dir`, so the
		// separator is the first `=` after the name's first byte.
		if i := strings.IndexByte(entry[min(1, len(entry)):], '='); i >= 0 {
			entry = entry[:i+min(1, len(entry))]
		}
		names = append(names, entry)
	}
	return names
}

func sameEnv(a, b string) bool {
	if runtime.GOOS == "windows" {
		return asciiEqualFold(a, b)
	}
	return a == b
}

// asciiEqualFold folds ASCII only, as Windows and usage-lib do: strings.EqualFold
// would also match the Kelvin sign to K.
func asciiEqualFold(a, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range len(a) {
		x, y := a[i], b[i]
		if 'A' <= x && x <= 'Z' {
			x += 'a' - 'A'
		}
		if 'A' <= y && y <= 'Z' {
			y += 'a' - 'A'
		}
		if x != y {
			return false
		}
	}
	return true
}

// pluralWord is the noun alone, without [plural]'s count.
func pluralWord(n int, noun string) string {
	if n == 1 {
		return noun
	}
	return noun + "s"
}
