package conformance

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jdx/usage/go/argv"
)

// Does this renderer put a logo where the other two put it?
//
// The rendering corpus is the agreement between usage-lib and usage-argv, and the
// logo file is the part of it this renderer can be held to directly: every vector
// is a whole page at eighty columns, which is the width this renderer lays every
// page out at. A logo beside the page narrows the page by the art's width and a
// gutter, so a renderer that placed the art without narrowing the page first would
// fail these on the wrapped text rather than on the picture.
//
// Only this file of the corpus, deliberately. The rest of it covers sections this
// renderer is not yet held to page-for-page; see page_test.go, which compares all
// 211 of mise's pages against the reference instead.

type renderVector struct {
	ID     string `json:"id"`
	Doc    string `json:"doc"`
	Spec   string `json:"spec"`
	Expect struct {
		Usage     string   `json:"usage"`
		ShortHelp []string `json:"short_help"`
		LongHelp  []string `json:"long_help"`
	} `json:"expect"`
}

func TestLogoPlacementMatchesTheRenderingCorpus(t *testing.T) {
	path := filepath.Join("..", "..", "corpus", "render", "06-logo.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("reading %s: %v", path, err)
	}
	var file struct {
		Vectors []renderVector `json:"vectors"`
	}
	if err := json.Unmarshal(raw, &file); err != nil {
		t.Fatalf("decoding %s: %v", path, err)
	}
	if len(file.Vectors) == 0 {
		t.Fatalf("%s has no vectors", path)
	}

	usageBin := findUsage(t)
	for _, v := range file.Vectors {
		t.Run(v.ID, func(t *testing.T) {
			lowered := lower(t, usageBin, v.Spec)
			root, _, help := lowered.BuildAll()
			spec := lowered.HelpSpec()
			path := []string{spec.Bin}

			for _, page := range []struct {
				name string
				got  string
				want []string
			}{
				{"short", argv.ShortHelp(spec, path, []*argv.Command{root}, help), v.Expect.ShortHelp},
				{"long", argv.LongHelp(spec, path, []*argv.Command{root}, help), v.Expect.LongHelp},
			} {
				if len(page.want) == 0 {
					continue
				}
				want := strings.Join(page.want, "\n") + "\n"
				if page.got != want {
					t.Errorf("%s page differs (%s)\n--- want ---\n%s\n--- got ---\n%s",
						page.name, v.Doc, want, page.got)
				}
			}
		})
	}
}
