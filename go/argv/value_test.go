package argv

import (
	"strings"
	"testing"
	"time"
)

func TestConversions(t *testing.T) {
	if n, err := Int("jobs", "8"); err != nil || n != 8 {
		t.Errorf("want 8, got %v %v", n, err)
	}
	if n, err := Uint("jobs", "8"); err != nil || n != 8 {
		t.Errorf("want 8, got %v %v", n, err)
	}
	if f, err := Float("ratio", "1.5"); err != nil || f != 1.5 {
		t.Errorf("want 1.5, got %v %v", f, err)
	}
	if b, err := Bool("force", "true"); err != nil || !b {
		t.Errorf("want true, got %v %v", b, err)
	}
	if d, err := Duration("wait", "1h30m"); err != nil || d != 90*time.Minute {
		t.Errorf("want 1h30m, got %v %v", d, err)
	}
	// And nothing is trimmed: `" 8 "` is a value the user quoted, and the Rust
	// sibling refuses it too. Verified against `" 8 ".parse::<i64>()`, which is
	// false.
	if _, err := Int("jobs", "  8 "); err == nil {
		t.Error("padded text should be refused, as it is in Rust")
	}
}

// A failure carries the text that would not convert and the type it was going
// to: a message saying only "invalid value" makes the user guess which of their
// words was wrong.
func TestAFailureNamesTheValueAndTheType(t *testing.T) {
	cases := []struct {
		what func() *Error
		want string
	}{
		{func() *Error { _, e := Int("jobs", "lots"); return e }, "whole number"},
		{func() *Error { _, e := Uint("jobs", "-1"); return e }, "not negative"},
		{func() *Error { _, e := Float("ratio", "half"); return e }, "a number"},
		{func() *Error { _, e := Bool("force", "yes"); return e }, "true or false"},
		{func() *Error { _, e := Duration("wait", "soon"); return e }, "duration"},
	}
	for _, c := range cases {
		err := c.what()
		if err == nil {
			t.Fatalf("want a failure for %q", c.want)
		}
		if err.Code != CodeInvalidValue {
			t.Errorf("want invalid_value, got %q", err.Code)
		}
		if err.Value == "" || err.Name == "" {
			t.Errorf("the failure should name both the entry and the value: %+v", err)
		}
		if !strings.Contains(err.Want, strings.Fields(c.want)[0]) {
			t.Errorf("want %q described, got %q", c.want, err.Want)
		}
		// And it renders as something a person can act on.
		if msg := explain(err, nil); !strings.Contains(msg, err.Value) {
			t.Errorf("the rendered message should show the value: %q", msg)
		}
	}
}

// `yes` is a bool to some CLIs and not to Go. The two truthiness rules in this
// package answer different questions and are deliberately different widths.
func TestBoolIsWiderThanEnvTruth(t *testing.T) {
	if _, err := Bool("force", "yes"); err == nil {
		t.Error("Go's spellings do not include `yes`")
	}
	if b, err := Bool("force", "T"); err != nil || !b {
		t.Errorf("Go's spellings do include `T`: %v %v", b, err)
	}
	// EnvTruth is an allow-list matching usage-lib, and narrower.
	if EnvTruth("T") {
		t.Error("EnvTruth takes only 1, true, True and TRUE")
	}
}

func TestEachStopsAtTheFirstFailure(t *testing.T) {
	got, err := Each("jobs", []string{"1", "2", "3"}, Int)
	if err != nil || len(got) != 3 || got[2] != 3 {
		t.Errorf("want [1 2 3], got %v %v", got, err)
	}
	_, err = Each("jobs", []string{"1", "two", "three"}, Int)
	if err == nil {
		t.Fatal("want a failure")
	}
	// The first, not the last: reporting `three` would send the user to the wrong
	// word.
	if err.Value != "two" {
		t.Errorf("want the first failure, got %q", err.Value)
	}
}

// The rejected value is quoted back, so it goes through the same escaping as the
// other messages that echo what the user typed — and it is the likeliest of them
// to carry something strange, since it exists because the text was unexpected.
func TestARejectedValueIsEscapedBeforeItIsShown(t *testing.T) {
	_, err := Int("jobs", "\x1b[31m8\r\nerror: forged")
	if err == nil {
		t.Fatal("want a failure")
	}
	msg := explain(err, nil)
	for _, forbidden := range []string{"\x1b", "\r", "\n"} {
		if strings.Contains(msg, forbidden) {
			t.Errorf("a control character survived into %q", msg)
		}
	}
	if !strings.Contains(msg, `\x1b`) || !strings.Contains(msg, "forged") {
		t.Errorf("the value should still be legible: %q", msg)
	}
}
