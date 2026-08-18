package conformance

import (
	"encoding/json"
	"os"
	"testing"

	"github.com/expr-lang/expr"
)

func TestGoMatchesThePortableValidationVectors(t *testing.T) {
	type vector struct {
		Expression string `json:"expression"`
		Value      string `json:"value"`
		Valid      bool   `json:"valid"`
	}
	data, err := os.ReadFile("../../conformance/validation.json")
	if err != nil {
		t.Fatal(err)
	}
	var vectors []vector
	if err := json.Unmarshal(data, &vectors); err != nil {
		t.Fatal(err)
	}
	for _, vector := range vectors {
		got, err := expr.Eval(vector.Expression, map[string]any{"value": vector.Value})
		if err != nil {
			t.Fatalf("%s with %q: %v", vector.Expression, vector.Value, err)
		}
		valid, ok := got.(bool)
		if !ok || valid != vector.Valid {
			t.Errorf("%s with %q: want %v, got %v", vector.Expression, vector.Value, vector.Valid, got)
		}
	}
}
