package argv

import (
	"strings"
	"testing"
)

// Does a HelpTemplate lay a page out the way the other two implementations do?
//
// The pages below are the ones `corpus/render/04-help-template.json` pins for
// usage-lib and usage-argv, transcribed. Go does not run the rendering corpus —
// `go/conformance` checks pages against mise, which declares no template — so
// this is where the third implementation is held to the same expectations, and a
// transcription is the price of that. Compare against the JSON when changing
// either.

func templateFixture(template string) (HelpSpec, []string, []*Command, HelpTable) {
	force := &Flag{Key: 2, Name: "force", Longs: []string{"force"}}
	file := &Arg{Key: 3, Name: "file"}
	root := &Command{Name: "ex", Key: 1, Flags: []*Flag{force}, Args: []*Arg{file}}
	help := HelpTable{
		{Key: 1, Short: "An example"},
		{Key: 2, Short: "Do it anyway"},
		{Key: 3, Short: "Which file"},
	}
	spec := HelpSpec{Name: "ex", Bin: "ex", About: "An example", HelpTemplate: template}
	return spec, []string{"ex"}, []*Command{root}, help
}

func TestATemplateReordersTheSections(t *testing.T) {
	// `{{flags}}` above `{{args}}` inverts the order every default page writes.
	spec, path, chain, help := templateFixture(
		"{{about}}\n\n{{usage}}\n\n{{flags}}\n\n{{args}}")
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force] [file]",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
		"",
		"Arguments:",
		"  [file]  Which file",
	}, "\n") + "\n"
	if got := ShortHelp(spec, path, chain, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateOmitsASection(t *testing.T) {
	// A section the template does not name is not on the page.
	spec, path, chain, help := templateFixture("{{about}}\n\n{{usage}}\n\n{{flags}}")
	got := ShortHelp(spec, path, chain, help)
	if strings.Contains(got, "Arguments:") {
		t.Fatalf("an unnamed section should not be rendered:\n%s", got)
	}
	if !strings.Contains(got, "--force") {
		t.Fatalf("a named section should be:\n%s", got)
	}
}

func TestATemplateWrapsTheSectionsInText(t *testing.T) {
	// Text around a placeholder is written as-is, which is what makes a template a
	// layout rather than a permutation.
	spec, path, chain, help := templateFixture(
		"== ex ==\n\n{{usage}}\n\n{{flags}}\n\nSee https://example.com/docs for more.")
	got := ShortHelp(spec, path, chain, help)
	if !strings.HasPrefix(got, "== ex ==\n\n") {
		t.Errorf("the author's heading should open the page:\n%s", got)
	}
	if !strings.HasSuffix(got, "See https://example.com/docs for more.\n") {
		t.Errorf("and their footer should close it:\n%s", got)
	}
}

func TestATemplateClosesTheGapAMissingSectionLeaves(t *testing.T) {
	// The rule that lets one template serve a whole CLI: this command has no
	// subcommands and no trailing text, and the separators around those sections
	// collapse rather than pushing the rest of the page down.
	spec, path, chain, help := templateFixture(
		"{{about}}\n\n{{usage}}\n\n{{commands}}\n\n{{args}}\n\n{{flags}}\n\n{{after_help}}")
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force] [file]",
		"",
		"Arguments:",
		"  [file]  Which file",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
	}, "\n") + "\n"
	if got := ShortHelp(spec, path, chain, help); got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestATemplateGathersALongPagesTrailingSections(t *testing.T) {
	// `{{after_help}}` is the whole tail of a page — examples, the spec's trailing
	// text, and the author and licence a long page ends with — so a template moving
	// it moves all of it at once.
	root := &Command{Name: "ex", Key: 1, Version: true}
	help := HelpTable{{Key: 1, Examples: []Example{{Header: "Force it", Code: "ex --force"}}}}
	spec := HelpSpec{
		Name: "ex", Bin: "ex", About: "An example", Version: "1.2.3",
		Author: "Ex Ample", AfterHelp: "Read the docs.",
		HelpTemplate: "{{after_help}}\n\n{{usage}}\n\n{{flags}}\n\n{{about}}",
	}
	got := LongHelp(spec, []string{"ex"}, []*Command{root}, help)
	want := strings.Join([]string{
		"Examples:",
		"  Force it:",
		"    $ ex --force",
		"",
		"Read the docs.",
		"",
		"Author: Ex Ample",
		"",
		"Usage: ex",
		"",
		"Flags:",
		"  -h, --help     Print help",
		"  -V, --version  Print version",
		"",
		"ex 1.2.3",
		"An example",
	}, "\n") + "\n"
	if got != want {
		t.Fatalf("page differs\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestAPageWithoutATemplateIsUnchanged(t *testing.T) {
	// The default order is what every other test in this package renders, and the
	// point of the whole arrangement is that adding a template did not move it.
	spec, path, chain, help := templateFixture("")
	want := strings.Join([]string{
		"An example",
		"",
		"Usage: ex [--force] [file]",
		"",
		"Arguments:",
		"  [file]  Which file",
		"",
		"Flags:",
		"      --force  Do it anyway",
		"  -h, --help   Print help",
	}, "\n") + "\n"
	if got := ShortHelp(spec, path, chain, help); got != want {
		t.Fatalf("the default page changed\n got:\n%s\nwant:\n%s", got, want)
	}
}

func TestAPlaceholderNamingNoSectionIsLeftAlone(t *testing.T) {
	// The vocabulary is checked where a spec is authored — KDL refuses one at parse,
	// the Rust derive at compile time — so a name reaching this renderer is text an
	// author meant literally rather than an error to discover here.
	spec, path, chain, help := templateFixture("{{usage}}\n\n{{options}}")
	got := ShortHelp(spec, path, chain, help)
	if !strings.Contains(got, "{{options}}") {
		t.Fatalf("an unknown placeholder should survive as written:\n%s", got)
	}
}

func TestTheSectionVocabularyIsTheSameSixWords(t *testing.T) {
	// The list the other implementations hold: usage::help_template::SECTIONS and
	// usage_argv::help::SECTIONS. Nothing mechanical compares them across languages,
	// so this is where Go's copy is written down beside the order they share.
	want := []string{"about", "usage", "commands", "args", "flags", "after_help"}
	if len(HelpSections) != len(want) {
		t.Fatalf("HelpSections = %v, want %v", HelpSections, want)
	}
	for i, name := range want {
		if HelpSections[i] != name {
			t.Fatalf("HelpSections = %v, want %v", HelpSections, want)
		}
	}

	// And every one of them can be placed: a name this renderer does not know would
	// survive substitution as literal braces.
	var template strings.Builder
	for i, name := range HelpSections {
		if i > 0 {
			template.WriteString("\n\n")
		}
		template.WriteString("{{" + name + "}}")
	}
	spec, path, chain, help := templateFixture(template.String())
	if got := ShortHelp(spec, path, chain, help); strings.Contains(got, "{{") {
		t.Fatalf("a section went unfilled:\n%s", got)
	}
}
