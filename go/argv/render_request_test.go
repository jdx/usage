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

// TestRequestPathDeclaredAction stops at user-declared help actions too.
func TestRequestPathDeclaredAction(t *testing.T) {
	sub := &Command{Name: "run", Key: 2}
	root := &Command{Name: "ex", Key: 1, Subcommands: []*Command{sub}, Flags: []*Flag{{Longs: []string{"assist"}, Action: ActionHelpAll}}}
	path, chain := requestPath(root, []string{"--assist", "run"}, "ex")
	if !reflect.DeepEqual(path, []string{"ex"}) || !reflect.DeepEqual(chain, []*Command{root}) {
		t.Fatalf("action walked past request: %q", path)
	}
}
