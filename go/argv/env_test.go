package argv

import (
	"reflect"
	"runtime"
	"testing"
)

func TestUnknownEnv(t *testing.T) {
	known := []string{"EX_PORT", "EX_LIBRARY"}
	names := []string{"EX_TSL", "HOME", "EX_PORT", "EX_", "EX", "EX_PROT", "EX_TSL", "EXTRA"}
	got := UnknownEnv("EX_", known, names)
	if want := []string{"EX_", "EX_PROT", "EX_TSL"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("got %q, want %q", got, want)
	}
	if err := RefuseUnknownEnv("EX_", known, []string{"EX_PORT"}); err != nil {
		t.Fatalf("nothing unknown should be a nil error, got %#v", err)
	}
	// Windows names are case-insensitive; elsewhere a different case is a different variable.
	got = UnknownEnv("EX_", known, []string{"EX_port", "Ex_Port", "ex_other"})
	want := []string{"EX_port"}
	if runtime.GOOS == "windows" {
		want = []string{"ex_other"}
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("case: got %q, want %q", got, want)
	}
	// The Windows rules, wherever this runs: one variable set in two cases is
	// reported once, and the Kelvin sign is not K, so EX_K is its own name
	// and stays apart from EX_K even with EX_Z sorted between them.
	got = unknownEnv("EX_", nil, []string{"EX_K", "EX_Z", "EX_K", "ex_k", "EX_ä", "EX_Ä"}, true)
	if want := []string{"EX_K", "EX_Z", "EX_Ä", "EX_K"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("windows: got %q, want %q", got, want)
	}
	// ß has no one-letter uppercase, so usage-lib keeps it as it is; so must this.
	if got := envKey("EX_ß"); got != "EX_ß" {
		t.Fatalf("ß folded to %q", got)
	}
	// Names that are not UTF-8: the prefix is matched on the valid head, so a
	// prefix ending in U+FFFD does not claim an invalid byte; such a name is never
	// a declared one; and two of them are one only when their bytes are.
	for _, fold := range []bool{false, true} {
		if got := unknownEnv("EX_�", nil, []string{"EX_\xff"}, fold); got != nil {
			t.Fatalf("fold=%v: U+FFFD prefix claimed an invalid byte: %q", fold, got)
		}
		got := unknownEnv("EX_", []string{"EX_�"}, []string{"EX_\xfe", "EX_\xff", "EX_\xff"}, fold)
		if want := []string{"EX_\xfe", "EX_\xff"}; !reflect.DeepEqual(got, want) {
			t.Fatalf("fold=%v: non-UTF-8 names: got %q, want %q", fold, got, want)
		}
	}
	if got := UnknownEnv("", known, []string{"PATH"}); got != nil {
		t.Fatalf("an empty prefix owns nothing, got %q", got)
	}
}

func TestUnknownEnvRendering(t *testing.T) {
	err := &Error{Code: CodeUnknownEnv, Name: "EX_", Names: []string{"EX_PROT"}}
	if got, want := err.Error(), "unknown environment variable under EX_: EX_PROT"; got != want {
		t.Fatalf("Error(): %q", got)
	}
	if got, want := Render(err, nil, nil, nil), "error: unknown environment variable under `EX_`: `EX_PROT`\n\nFor more information, try `--help`.\n"; got != want {
		t.Fatalf("Render: %q", got)
	}
}
