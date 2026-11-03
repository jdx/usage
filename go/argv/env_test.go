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
