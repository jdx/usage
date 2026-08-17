package argv

import (
	"strconv"
	"time"
)

// Turning bound text into the type a field wants.
//
// Binding collects text, deliberately: the grammar decides which token becomes
// which flag or argument, not what it means, so `"8"` stays a string until
// something that knows the target type asks. This is where that asking happens.
//
// The functions are separate rather than one generic `Convert`, because the set
// of types a CLI wants is small and closed, and each one has its own idea of what
// it accepts — `1h30m` is a duration and not a number, `yes` is neither. A
// generated struct calls the one matching its field.
//
// Every failure carries the text that would not convert and the type it was being
// converted to. A message that says only "invalid value" makes the user guess
// which of their words was wrong.
//
// Nothing is trimmed on the way in. `" 8 "` is refused, as `" 8 ".parse::<i64>()`
// is on the Rust side — checked rather than assumed. The parser goes out of its
// way to hand over the bytes the operating system gave it, and a converter
// quietly tidying them would mean a quoted argument means one thing in Go and
// another in Rust from the same spec.

// Int converts a bound value, naming the entry in any failure.
func Int(name, value string) (int64, *Error) {
	n, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0, invalid(name, value, "a whole number")
	}
	return n, nil
}

// Uint is [Int] for a value that may not be negative.
func Uint(name, value string) (uint64, *Error) {
	n, err := strconv.ParseUint(value, 10, 64)
	if err != nil {
		return 0, invalid(name, value, "a whole number, not negative")
	}
	return n, nil
}

// Float converts a bound value to a float.
func Float(name, value string) (float64, *Error) {
	n, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return 0, invalid(name, value, "a number")
	}
	return n, nil
}

// Bool converts a bound value to a bool.
//
// The spellings are Go's own, which are also the ones `strconv.ParseBool` takes:
// `1`, `t`, `T`, `true`, `TRUE`, `True` and their false counterparts. Note this
// is *wider* than [EnvTruth], which an environment variable setting a value-less
// flag goes through — that one is an allow-list matching usage-lib, and the two
// answer different questions: this converts a value somebody typed, that one
// decides whether a variable counts as setting a flag at all.
func Bool(name, value string) (bool, *Error) {
	b, err := strconv.ParseBool(value)
	if err != nil {
		return false, invalid(name, value, "true or false")
	}
	return b, nil
}

// Duration converts a bound value to a duration, in Go's notation: `1h30m`,
// `250ms`, `2s`.
func Duration(name, value string) (time.Duration, *Error) {
	d, err := time.ParseDuration(value)
	if err != nil {
		return 0, invalid(name, value, "a duration such as 30s or 1h30m")
	}
	return d, nil
}

// Each maps a conversion over the values a variadic or repeatable entry
// collected, stopping at the first that will not convert.
//
// Written out because the alternative is every caller writing the same loop, and
// getting the early return wrong in a way that reports the last failure instead
// of the first.
func Each[T any](name string, values []string, convert func(string, string) (T, *Error)) ([]T, *Error) {
	out := make([]T, 0, len(values))
	for _, v := range values {
		converted, err := convert(name, v)
		if err != nil {
			return nil, err
		}
		out = append(out, converted)
	}
	return out, nil
}

func invalid(name, value, want string) *Error {
	return &Error{Code: CodeInvalidValue, Name: name, Value: value, Want: want}
}
