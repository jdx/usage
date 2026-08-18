package argv

import (
	"reflect"
	"testing"
)

func TestMulticallBasename(t *testing.T) {
	if got := MulticallBasename("/usr/bin/ls"); got != "ls" {
		t.Fatalf("path: got %q", got)
	}
	if got := MulticallBasename(`C:\busybox\ls.exe`); got != "ls" {
		t.Fatalf("exe: got %q", got)
	}
	if got := MulticallBasename("LS.EXE"); got != "LS" {
		t.Fatalf("case: got %q", got)
	}
}

func TestRewriteMulticall(t *testing.T) {
	args := []string{"-l"}
	got := RewriteMulticall("/usr/bin/ls", args, "busybox", "busybox")
	if !reflect.DeepEqual(got, []string{"ls", "-l"}) {
		t.Fatalf("applet: got %q", got)
	}
	got = RewriteMulticall("/usr/bin/busybox", []string{"ls", "-l"}, "busybox", "busybox")
	if !reflect.DeepEqual(got, []string{"ls", "-l"}) {
		t.Fatalf("dispatcher: got %q", got)
	}
}
