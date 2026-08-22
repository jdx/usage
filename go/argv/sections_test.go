package argv

import (
	"strings"
	"testing"
)

func helpKeyed(entries ...Help) HelpTable {
	var max uint64
	for _, e := range entries {
		if e.Key > max {
			max = e.Key
		}
	}
	table := make(HelpTable, max)
	for i := range table {
		table[i].Key = uint64(i + 1)
	}
	for _, e := range entries {
		table[e.Key-1] = e
	}
	return table
}

// Does a HelpTemplate lay a page out the way the other two implementations do?
//
// The pages below are the ones `corpus/render/04-help-template.json` pins for
// usage-lib and usage-argv. Go does not run the rendering corpus —
// `go/conformance` checks pages against mise, which declares no template — so
// this is where the third implementation is held to those same full pages.

func TestATemplateReordersTheSections(t *testing.T) {
	// corpus/render/04-help-template.json#template-reorders-the-sections
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	file := &Arg{Key: 3, Name: "file", Required: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}, Args: []*Arg{file}}
	help := helpKeyed(
		Help{Key: 1, Short: "An example"},
		Help{Key: 2, Short: "Do it anyway"},
		Help{Key: 3, Short: "Which file", Demanded: true},
	)
	spec := HelpSpec{
		Name: "ex", Bin: "ex", About: "An example",
		HelpTemplate: "{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}",
	}
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force] <file>",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
		"",
		"Arguments:",
		"  <file>  Which file",
	}, "\n") + "\n"
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateOmitsASection(t *testing.T) {
	// corpus/render/04-help-template.json#template-omits-a-section
	install := &Command{Name: "install", Key: 4}
	remove := &Command{Name: "remove", Key: 5}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{install, remove}}
	help := helpKeyed(
		Help{Key: 1, Short: "An example"},
		Help{Key: 4, Short: "Install a tool"},
		Help{Key: 5, Short: "Remove a tool"},
	)
	spec := HelpSpec{
		Name: "ex", Bin: "ex", About: "An example",
		HelpTemplate: "{{about}}\n\n{{usage}}\n\n{{flags}}",
	}
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex <SUBCOMMAND>",
		"",
		"Flags:",
		"  -h, --help  Print help",
	}, "\n") + "\n"
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateWrapsTheSectionsInText(t *testing.T) {
	// corpus/render/04-help-template.json#template-wraps-the-sections-in-text
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}}
	help := helpKeyed(Help{Key: 2, Short: "Do it anyway"})
	spec := HelpSpec{
		Name: "ex", Bin: "ex",
		HelpTemplate: "== ex ==\n\n{{usage}}\n\n{{flags}}\n\nSee https://example.com/docs for more.",
	}
	want := strings.Join([]string{
		"== ex ==",
		"",
		"Usage: ex [--force]",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
		"",
		"See https://example.com/docs for more.",
	}, "\n") + "\n"
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateClosesTheGapAMissingSectionLeaves(t *testing.T) {
	// corpus/render/04-help-template.json#template-closes-the-gap-a-missing-section-leaves
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}}
	help := helpKeyed(
		Help{Key: 1, Short: "An example"},
		Help{Key: 2, Short: "Do it anyway"},
	)
	spec := HelpSpec{
		Name: "ex", Bin: "ex", About: "An example",
		HelpTemplate: "{{about}}\n\n{{usage}}\n\n{{commands}}\n\n{{args}}\n\n{{flags}}\n\n{{after_help}}",
	}
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force]",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
	}, "\n") + "\n"
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateGathersALongPagesTrailingSections(t *testing.T) {
	// corpus/render/04-help-template.json#template-gathers-a-long-pages-trailing-sections
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	root := &Command{Name: "ex", Key: 1, Version: true, Flags: []*Flag{force}}
	help := helpKeyed(
		Help{Key: 1, Examples: []Example{{Header: "Force it", Code: "ex --force"}}},
		Help{Key: 2, Short: "Do it anyway"},
	)
	spec := HelpSpec{
		Name: "ex", Bin: "ex", About: "An example", Version: "1.2.3",
		Author: "Ex Ample", AfterHelp: "Read the docs.",
		HelpTemplate: "{{after_help}}\n\n{{usage}}\n\n{{flags}}\n\n{{about}}",
	}
	want := strings.Join([]string{
		"Examples:",
		"  Force it:",
		"    $ ex --force",
		"",
		"Read the docs.",
		"",
		"Author: Ex Ample",
		"",
		"Usage: ex [--force]",
		"",
		"Flags:",
		"      --force    Do it anyway",
		"  -h, --help     Print help",
		"  -V, --version  Print version",
		"",
		"ex 1.2.3",
		"An example",
	}, "\n") + "\n"
	if got := LongHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestAPageWithoutATemplateIsUnchanged(t *testing.T) {
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	file := &Arg{Key: 3, Name: "file", Required: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}, Args: []*Arg{file}}
	help := helpKeyed(
		Help{Key: 1, Short: "An example"},
		Help{Key: 2, Short: "Do it anyway"},
		Help{Key: 3, Short: "Which file", Demanded: true},
	)
	spec := HelpSpec{Name: "ex", Bin: "ex", About: "An example"}
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force] <file>",
		"",
		"Arguments:",
		"  <file>  Which file",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
	}, "\n") + "\n"
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
		t.Fatalf("the default page changed\n got:\n%s\nwant:\n%s", got, want)
	}

	// An empty or whitespace-only template is the same unset: Go used to treat
	// "" as default and Rust used to render "\n".
	for _, template := range []string{"", "  ", "\n\t"} {
		spec.HelpTemplate = template
		if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); got != want {
			t.Fatalf("template %q should be the default page\n got:\n%s\nwant:\n%s", template, got, want)
		}
	}
}

func TestAPlaceholderNamingNoSectionIsLeftAlone(t *testing.T) {
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}}
	spec := HelpSpec{Name: "ex", Bin: "ex", HelpTemplate: "{{usage}}\n\n{{options}}"}
	got := ShortHelp(spec, []string{"ex"}, []*Command{root}, HelpTable{})
	if !strings.Contains(got, "{{options}}") {
		t.Fatalf("an unknown placeholder should survive as written:\n%s", got)
	}
}

func TestTheSectionVocabularyIsTheSameSixWords(t *testing.T) {
	want := []string{"about", "usage", "commands", "args", "flags", "after_help"}
	if len(HelpSections) != len(want) {
		t.Fatalf("HelpSections = %v, want %v", HelpSections, want)
	}
	for i, name := range want {
		if HelpSections[i] != name {
			t.Fatalf("HelpSections = %v, want %v", HelpSections, want)
		}
	}

	var template strings.Builder
	for i, name := range HelpSections {
		if i > 0 {
			template.WriteString("\n\n")
		}
		template.WriteString("{{" + name + "}}")
	}
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	file := &Arg{Key: 3, Name: "file", Required: true}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}, Args: []*Arg{file}}
	help := helpKeyed(
		Help{Key: 1, Short: "An example"},
		Help{Key: 2, Short: "Do it anyway"},
		Help{Key: 3, Short: "Which file", Demanded: true},
	)
	spec := HelpSpec{Name: "ex", Bin: "ex", About: "An example", HelpTemplate: template.String()}
	if got := ShortHelp(spec, []string{"ex"}, []*Command{root}, help); strings.Contains(got, "{{") {
		t.Fatalf("a section went unfilled:\n%s", got)
	}
}
