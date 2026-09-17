package argv

import (
	"strconv"
	"testing"
)

// TestNativeInt checks platform bounds and errors without accepting float or whitespace syntax.
func TestNativeInt(t *testing.T) {
	for _, tc := range []struct {
		value string
		want  int
	}{{"0", 0}, {"+12", 12}, {"-12", -12}} {
		got, err := NativeInt("--jobs", tc.value)
		if err != nil || got != tc.want {
			t.Fatalf("NativeInt(%q) = %d, %v", tc.value, got, err)
		}
	}
	max := int(^uint(0) >> 1)
	for _, want := range []int{max, -max - 1} {
		if got, err := NativeInt("--jobs", strconv.FormatInt(int64(want), 10)); err != nil || got != want {
			t.Fatalf("bound %d: %d %v", want, got, err)
		}
	}
	overflow := "9223372036854775808"
	if strconv.IntSize == 32 {
		overflow = "2147483648"
	}
	for _, value := range []string{"", " 1", "1.0", "1_000", overflow} {
		_, err := NativeInt("--jobs", value)
		if err == nil || err.Code != CodeInvalidValue || err.Name != "--jobs" || err.Value != value {
			t.Fatalf("NativeInt(%q): %+v", value, err)
		}
	}
}
