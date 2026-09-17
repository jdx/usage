package argv

import (
	"reflect"
	"strings"
	"testing"
)

// TestRequestPath checks invoked aliases, implicit routes, and early action exits.
func TestRequestPath(t *testing.T) {
	leaf := &Command{Name: "leaf", Aliases: []string{"l"}, Key: 4}
	left := &Command{Name: "left", Key: 2, Subcommands: []*Command{leaf}}
	right := &Command{Name: "right", Aliases: []string{"r"}, Key: 3, Subcommands: []*Command{leaf}}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{left, right}}
	for _, tt := range []struct {
		words, path []string
		chain       []*Command
	}{
		{[]string{"r", "l", "--help"}, []string{"ex", "r", "l"}, []*Command{root, right, leaf}},
		{[]string{"help", "r", "l"}, []string{"ex", "r", "l"}, []*Command{root, right, leaf}},
		{[]string{"r", "help", "l"}, []string{"ex", "r", "l"}, []*Command{root, right, leaf}},
		{[]string{"help", "r", "unknown"}, []string{"ex", "r"}, []*Command{root, right}},
		{[]string{"--help", "right"}, []string{"ex"}, []*Command{root}},
		{[]string{"r", "--help", "leaf"}, []string{"ex", "r"}, []*Command{root, right}},
	} {
		t.Run(strings.Join(tt.words, " "), func(t *testing.T) {
			path, chain := requestPath(root, tt.words, "ex")
			if !reflect.DeepEqual(path, tt.path) || !reflect.DeepEqual(chain, tt.chain) {
				t.Fatalf("words %q: path %q, chain %v; want %q, %v", tt.words, path, chain, tt.path, tt.chain)
			}
		})
	}
	run := &Command{Name: "run", Key: 5, Flags: []*Flag{{Longs: []string{"jobs"}, TakesValue: true}}}
	root.DefaultSubcommand = run
	root.DefaultSubcommandFlags = true
	root.Subcommands = append(root.Subcommands, run)
	path, chain := requestPath(root, []string{"--jobs", "2", "--help"}, "ex")
	if !reflect.DeepEqual(path, []string{"ex", "run"}) || !reflect.DeepEqual(chain, []*Command{root, run}) {
		t.Fatalf("implicit path %q, chain %v", path, chain)
	}
}

// TestRenderRequest checks page selection, runtime metadata, and unhandled errors.
func TestRenderRequest(t *testing.T) {
	root := &Command{Name: "ex", Key: 1}
	spec := HelpSpec{Name: "ex", Bin: "custom", Version: "dev", LongVersion: "dev (revision abc)", About: "short", LongAbout: "long"}
	path := []string{"custom"}
	chain := []*Command{root}
	for _, tt := range []struct {
		request *Error
		want    string
		handled bool
	}{
		{nil, "", false},
		{&Error{Code: CodeUnknownFlag}, "", false},
		{&Error{Code: CodeHelp}, ShortHelp(spec, path, chain, nil), true},
		{&Error{Code: CodeHelp, Long: true}, LongHelp(spec, path, chain, nil), true},
		{&Error{Code: CodeHelp, All: true}, AllHelp(spec, path, chain, nil), true},
		{&Error{Code: CodeVersion}, "dev\n", true},
		{&Error{Code: CodeVersion, Long: true}, "dev (revision abc)\n", true},
	} {
		got, handled := RenderRequest(tt.request, spec, root, nil, nil)
		if got != tt.want || handled != tt.handled {
			t.Errorf("request %v: (%q,%v), want (%q,%v)", tt.request, got, handled, tt.want, tt.handled)
		}
	}
	spec.LongVersion = ""
	if got, _ := RenderRequest(&Error{Code: CodeVersion, Long: true}, spec, root, nil, nil); got != "dev\n" {
		t.Fatalf("long version fallback: %q", got)
	}
}

// TestRenderRequestSharedRoute keeps inherited flags from the invoked parent.
func TestRenderRequestSharedRoute(t *testing.T) {
	leaf := &Command{Name: "leaf", Key: 4}
	left := &Command{Name: "left", Key: 2, Subcommands: []*Command{leaf}, Flags: []*Flag{{Key: 5, Longs: []string{"left-global"}, Global: true}}}
	right := &Command{Name: "right", Aliases: []string{"r"}, Key: 3, Subcommands: []*Command{leaf}, Flags: []*Flag{{Key: 6, Longs: []string{"right-global"}, Global: true}}}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{left, right}}
	got, handled := RenderRequest(&Error{Code: CodeHelp, Cmd: leaf, Long: true}, HelpSpec{Bin: "ex"}, root, []string{"help", "r", "leaf"}, nil)
	if !handled || !strings.Contains(got, "ex r leaf") || !strings.Contains(got, "--right-global") || strings.Contains(got, "--left-global") {
		t.Fatalf("wrong shared-route page: %s", got)
	}
}

// TestRenderRequestVariadicTerminator uses a real parser request. Consuming a
// variadic terminator does not emit an event, so the help topic must not be
// reconstructed from the previous event position.
func TestRenderRequestVariadicTerminator(t *testing.T) {
	config := &Command{Name: "config", Key: 3}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: ">>"}},
		Subcommands: []*Command{config},
	}
	words := []string{"--tools", "x", ">>", "help", "config"}
	p := New(root, words)
	for p.Next() {
	}
	request, ok := p.Err().(*Error)
	if !ok || request.Code != CodeHelp || request.Cmd != config {
		t.Fatalf("parse request: %v", p.Err())
	}
	got, handled := RenderRequest(request, HelpSpec{Bin: "ex"}, root, words, nil)
	if !handled || !strings.Contains(got, "ex config") {
		t.Fatalf("rendered wrong topic: handled=%v output=%s", handled, got)
	}
}

// TestRenderRequestHelpTerminator handles a terminator colliding with help.
func TestRenderRequestHelpTerminator(t *testing.T) {
	// The terminator deliberately collides with the help subcommand spelling.
	config := &Command{Name: "config", Key: 3}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: "help"}},
		Subcommands: []*Command{config},
	}
	words := []string{"--tools", "x", "help", "help", "config"}
	p := New(root, words)
	for p.Next() {
	}
	request, ok := p.Err().(*Error)
	if !ok || request.Code != CodeHelp || request.Cmd != config {
		t.Fatalf("parse request: %v", p.Err())
	}
	got, handled := RenderRequest(request, HelpSpec{Bin: "ex"}, root, words, nil)
	if !handled || !strings.Contains(got, "ex config") {
		t.Fatalf("rendered wrong topic: handled=%v output=%s", handled, got)
	}
}

// TestRequestPathVariadicTerminatorAliasAndPartialTopic preserves aliases on shared routes.
func TestRequestPathVariadicTerminatorAliasAndPartialTopic(t *testing.T) {
	leaf := &Command{Name: "leaf", Aliases: []string{"l"}, Key: 4}
	right := &Command{Name: "right", Aliases: []string{"r"}, Key: 3, Subcommands: []*Command{leaf}}
	left := &Command{Name: "left", Subcommands: []*Command{leaf}}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: ">>"}},
		Subcommands: []*Command{left, right},
	}
	for _, tt := range []struct {
		words []string
		path  []string
	}{
		{[]string{"--tools", "x", ">>", "help", "r", "l"}, []string{"ex", "r", "l"}},
		{[]string{"--tools", "x", ">>", "help", "r", "unknown"}, []string{"ex", "r"}},
	} {
		p := New(root, tt.words)
		for p.Next() {
		}
		request, ok := p.Err().(*Error)
		if !ok || request.Code != CodeHelp {
			t.Fatalf("words %q: parse request: %v", tt.words, p.Err())
		}
		path, chain := requestPath(root, tt.words, "ex")
		if !reflect.DeepEqual(path, tt.path) || chain[len(chain)-1] != request.Cmd {
			t.Fatalf("words %q: path=%q chain=%v, want path=%q target=%v", tt.words, path, chain, tt.path, request.Cmd)
		}
	}
}

// TestRequestPathNestedDeclaredHelpTopic handles a declared help child in a topic route.
func TestRequestPathNestedDeclaredHelpTopic(t *testing.T) {
	nestedHelp := &Command{Name: "help", Key: 4}
	config := &Command{Name: "config", Key: 3, Subcommands: []*Command{nestedHelp}}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: ">>"}},
		Subcommands: []*Command{config},
	}
	words := []string{"--tools", "x", ">>", "help", "config", "help"}
	p := New(root, words)
	for p.Next() {
	}
	request, ok := p.Err().(*Error)
	if !ok || request.Code != CodeHelp || request.Cmd != nestedHelp {
		t.Fatalf("parse request: %v", p.Err())
	}
	path, chain := requestPath(root, words, "ex")
	want := []string{"ex", "config", "help"}
	if !reflect.DeepEqual(path, want) || chain[len(chain)-1] != nestedHelp {
		t.Fatalf("path=%q chain=%v, want path=%q target=%v", path, chain, want, nestedHelp)
	}
}

// TestRequestPathTerminatorBeforeImplicitDefault keeps implicit defaults canonical.
func TestRequestPathTerminatorBeforeImplicitDefault(t *testing.T) {
	run := &Command{Name: "run", Key: 2, Args: []*Arg{{Name: "task"}}}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:             []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: ">>"}},
		DefaultSubcommand: run, DefaultSubcommandFlags: true,
		Subcommands: []*Command{run},
	}
	words := []string{"--tools", "x", ">>", "task", "--help"}
	p := New(root, words)
	for p.Next() {
	}
	// Synthetic help is an ordinary flag event; model the request the caller
	// creates after observing that event.
	path, chain := requestPath(root, words, "ex")
	want := []string{"ex", "run"}
	if !reflect.DeepEqual(path, want) || chain[len(chain)-1] != run {
		t.Fatalf("path=%q chain=%v, want path=%q target=%v", path, chain, want, run)
	}
}

// TestRequestPathTerminatorMatchesImplicitAlias does not mistake a terminator for an alias.
func TestRequestPathTerminatorMatchesImplicitAlias(t *testing.T) {
	run := &Command{Name: "run", Aliases: []string{"r"}, Key: 2, Args: []*Arg{{Name: "task"}}}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:             []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: "r"}},
		DefaultSubcommand: run, DefaultSubcommandFlags: true,
		Subcommands: []*Command{run},
	}
	words := []string{"--tools", "x", "r", "task"}
	path, chain := requestPath(root, words, "ex")
	want := []string{"ex", "run"}
	if !reflect.DeepEqual(path, want) || chain[len(chain)-1] != run {
		t.Fatalf("path=%q chain=%v, want path=%q target=%v", path, chain, want, run)
	}
}

// TestRequestPathTerminatorBeforeExplicitAlias preserves an alias after a terminator.
func TestRequestPathTerminatorBeforeExplicitAlias(t *testing.T) {
	run := &Command{Name: "run", Aliases: []string{"r"}, Key: 2}
	root := &Command{
		Name: "ex", Key: 1,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: ">>"}},
		Subcommands: []*Command{run},
	}
	words := []string{"--tools", "x", ">>", "r"}
	path, chain := requestPath(root, words, "ex")
	want := []string{"ex", "r"}
	if !reflect.DeepEqual(path, want) || chain[len(chain)-1] != run {
		t.Fatalf("path=%q chain=%v, want path=%q target=%v", path, chain, want, run)
	}
}

// TestRequestPathTerminatorExplicitChildPrecedence lets an explicit child win under precedence.
func TestRequestPathTerminatorExplicitChildPrecedence(t *testing.T) {
	run := &Command{Name: "run", Aliases: []string{"r"}, Key: 2}
	root := &Command{
		Name: "ex", Key: 1, SubcommandPrecedenceOverArg: true,
		Flags:       []*Flag{{Name: "tools", Longs: []string{"tools"}, TakesValue: true, Variadic: true, ValueTerminator: "r"}},
		Subcommands: []*Command{run},
	}
	words := []string{"--tools", "x", "r"}
	path, chain := requestPath(root, words, "ex")
	want := []string{"ex", "r"}
	if !reflect.DeepEqual(path, want) || chain[len(chain)-1] != run {
		t.Fatalf("path=%q chain=%v, want path=%q target=%v", path, chain, want, run)
	}
}

// TestRequestPathDeclaredAction stops at user-declared help actions too.
func TestRequestPathDeclaredAction(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}, Flags: []*Flag{{Longs: []string{"assist"}, Action: ActionHelpAll}}}
	path, chain := requestPath(root, []string{"--assist", "run"}, "ex")
	if !reflect.DeepEqual(path, []string{"ex"}) || !reflect.DeepEqual(chain, []*Command{root}) {
		t.Fatalf("action walked past request: %q", path)
	}
}
