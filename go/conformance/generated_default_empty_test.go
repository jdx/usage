package conformance

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// TestGeneratedDefaultSubcommandOnEmpty exercises generated Go code rather than
// only the corpus table path. Each fixture is generated into a fresh module so
// stale checked-in tables cannot hide generator/runtime drift.
func TestGeneratedDefaultSubcommandOnEmpty(t *testing.T) {
	runGeneratedFixture(t, `name "example"
bin "example"
default_subcommand "run"
default_subcommand_on_empty #true
version "1.0"
flag "--verbose" global=#true
flag "--mode <mode>" default="root"
flag "--value <value>" allow_hyphen_values=#true
cmd "run" {
  alias "r"
  flag "--mode <mode>" default="child"
  flag "--child <child>" env="EX_CHILD"
}
`, `package generated
import (
  "errors"
  "testing"
  "github.com/jdx/usage/go/argv"
)
func TestDefaultEmptyFields(t *testing.T) {
  t.Setenv("EX_CHILD", "from-env")
  got, err := Parse([]string{})
  if err != nil || got == nil || got.Run == nil { t.Fatalf("empty: %#v %v", got, err) }
  if got.Run.Mode != "child" || got.Run.Child != "from-env" { t.Fatalf("child defaults/env: %#v", got.Run) }
  got, err = Parse([]string{"--verbose"})
  if err != nil || got == nil || !got.Verbose || got.Run == nil { t.Fatalf("parent global ownership: %#v %v", got, err) }
  if got.Mode != "root" || got.Run.Mode != "child" { t.Fatalf("same-name ownership: %#v %#v", got.Mode, got.Run.Mode) }
  got, err = Parse([]string{"--mode", "custom"})
  if err != nil || got == nil || got.Run == nil || got.Mode != "custom" || got.Run.Mode != "child" { t.Fatalf("parent local ownership: %#v %v", got, err) }
  got, err = Parse([]string{"r"})
  if err != nil || got == nil || got.Run == nil { t.Fatalf("alias: %#v %v", got, err) }
  _, err = Parse([]string{"--help"}); var req *argv.Error
  if !errors.As(err, &req) || req.Code != argv.CodeHelp || req.Cmd.Name != "example" { t.Fatalf("root help: %v", err) }
  _, err = Parse([]string{"--version"}); if !errors.As(err, &req) || req.Code != argv.CodeVersion || req.Cmd.Name != "example" { t.Fatalf("root version: %v", err) }
  _, err = Parse([]string{"--mode"}); if !errors.As(err, &req) || req.Code != argv.CodeMissingFlagValue { t.Fatalf("missing value precedence: %v", err) }
  got, err = Parse([]string{"--value", "--"}); if err != nil || got == nil || got.Run == nil || got.Value != "--" { t.Fatalf("literal separator value: %#v %v", got, err) }
  got, err = Parse([]string{"--"}); if err != nil || got == nil || got.Run != nil { t.Fatalf("bare separator should remain root: %#v %v", got, err) }
  _, err = Parse([]string{"--", "--help"}); if !errors.As(err, &req) || req.Code == argv.CodeHelp { t.Fatalf("syntactic separator unexpectedly requested help: %v", err) }
}
`, "primary")

	runGeneratedFixture(t, `name "example"
bin "example"
default_subcommand "run"
default_subcommand_on_empty #true
flag "--verbose" global=#true
flag "--value <value>"
cmd "run" arg_required_else_help=#true {}
`, `package generated
import (
  "errors"
  "testing"
  "github.com/jdx/usage/go/argv"
)
func TestImplicitHelpPrecedence(t *testing.T) {
  _, err := Parse([]string{"--verbose"}); var req *argv.Error
  if !errors.As(err, &req) || req.Code != argv.CodeHelp || req.Cmd.Name != "run" { t.Fatalf("child help after parent flag: %v", err) }
  _, err = Parse([]string{"--value"}); if !errors.As(err, &req) || req.Code != argv.CodeMissingFlagValue { t.Fatalf("missing value must precede child help: %v", err) }
  _, err = Parse([]string{"--help"}); if !errors.As(err, &req) || req.Code != argv.CodeHelp || req.Cmd.Name != "example" { t.Fatalf("root help bypass: %v", err) }
}
`, "help")

	runGeneratedFixture(t, `name "example"
bin "example"
default_subcommand "run"
default_subcommand_on_empty #true
flag "--needed <needed>" required=#true
arg "<root>"
cmd "run" {}
`, `package generated
import (
  "errors"
  "testing"
  "github.com/jdx/usage/go/argv"
)
func TestRequiredRootParity(t *testing.T) {
  for _, args := range [][]string{nil, {"run"}} {
    _, err := Parse(args); var e *argv.Error
    if !errors.As(err, &e) || e.Code != argv.CodeMissingRequiredFlag { t.Fatalf("required root flag Parse(%q): %v", args, err) }
  }
  _, err := Parse([]string{"--needed", "yes"}); var e *argv.Error
  if !errors.As(err, &e) || e.Code != argv.CodeMissingRequiredArg { t.Fatalf("required root arg: %v", err) }
}
`, "required-false")

	runGeneratedFixture(t, `name "example"
bin "example"
default_subcommand "run"
default_subcommand_on_empty #true
subcommand_negates_reqs #true
flag "--needed <needed>" required=#true
arg "<root>"
cmd "run" {}
`, `package generated
import "testing"
func TestRequiredRootNegated(t *testing.T) {
  for _, args := range [][]string{nil, {"run"}} {
    got, err := Parse(args)
    if err != nil || got == nil || got.Run == nil { t.Fatalf("negated root requirements Parse(%q): %#v %v", args, got, err) }
  }
  got, err := Parse([]string{"--needed", "yes"})
  if err != nil || got == nil || got.Run == nil { t.Fatalf("negated root arg/flag: %#v %v", got, err) }
}
`, "required-true")

	runGeneratedFixture(t, `name "example"
bin "example"
default_subcommand "run"
default_subcommand_on_empty #true
arg_required_else_help #true
cmd "run" {}
`, `package generated
import "testing"
func TestRootImplicitHelpBypass(t *testing.T) {
  got, err := Parse(nil)
  if err != nil || got == nil || got.Run == nil { t.Fatalf("valid default child must bypass root help: %#v %v", got, err) }
}
`, "root-help")
}

func runGeneratedFixture(t *testing.T, spec, tests, name string) {
	t.Helper()
	dir := t.TempDir()
	module, err := filepath.Abs("..")
	if err != nil {
		t.Fatal(err)
	}
	goMod := fmt.Sprintf("module example.com/generated-%s\n\ngo 1.26.0\n\nrequire github.com/jdx/usage/go v0.0.0\nreplace github.com/jdx/usage/go => %q\n", name, filepath.ToSlash(module))
	files := map[string]string{"go.mod": goMod, "cli.usage.kdl": spec, "generated_test.go": tests}
	for file, contents := range files {
		if err := os.WriteFile(filepath.Join(dir, file), []byte(contents), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	cmd := exec.Command(findUsage(t), "generate", "go", "-f", filepath.Join(dir, "cli.usage.kdl"), "--package", "generated", "-o", filepath.Join(dir, "tables.go"))
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("%s generate: %v\n%s", name, err, out)
	}
	cmd = exec.Command("go", "test", "-count=1", "-mod=mod", ".")
	cmd.Dir = dir
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("%s generated parser: %v\n%s", name, err, out)
	}
}
