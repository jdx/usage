package conformance

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// Exercise the generated front door, not just the event parser: synthetic help
// and version flags are events that Parse must turn into requests.
func TestGeneratedHelpAndVersion(t *testing.T) {
	dir := t.TempDir()
	module, err := filepath.Abs("..")
	if err != nil {
		t.Fatal(err)
	}
	files := map[string]string{
		"go.mod": fmt.Sprintf("module example.com/generated\n\ngo 1.26.0\n\nrequire github.com/jdx/usage/go v0.0.0\nreplace github.com/jdx/usage/go => %q\n", filepath.ToSlash(module)),
		"cli.usage.kdl": `name "example"
bin "example"
version "1.0.0"
flag "--required <value>" required=#true
cmd "run" {
    flag "--child-required <value>" required=#true
}
cmd "shadow" {
    flag "-h --help"
    flag "-V --version"
}
cmd "disabled" {
    disable_help_flag #true
    disable_version_flag #true
}
`,
		"tables_test.go": `package generated

import (
    "errors"
    "testing"

    "github.com/jdx/usage/go/argv"
)

func TestRequests(t *testing.T) {
    for _, tc := range []struct {
        name string
        args []string
        code argv.Code
        command string
        long bool
    }{
        {"short help", []string{"-h"}, argv.CodeHelp, "example", false},
        {"long help", []string{"--help"}, argv.CodeHelp, "example", true},
        {"short version", []string{"-V"}, argv.CodeVersion, "example", false},
        {"long version", []string{"--version"}, argv.CodeVersion, "example", true},
        {"child help", []string{"run", "--help"}, argv.CodeHelp, "run", true},
        {"help stops parsing", []string{"--help", "--unknown"}, argv.CodeHelp, "example", true},
        {"version stops parsing", []string{"--version", "--unknown"}, argv.CodeVersion, "example", true},
    } {
        t.Run(tc.name, func(t *testing.T) {
            cli, err := Parse(tc.args)
            var request *argv.Error
            if cli != nil || !errors.As(err, &request) || request.Code != tc.code {
                t.Fatalf("Parse(%q) = %#v, %v; want nil, %v", tc.args, cli, err, tc.code)
            }
            if request.Cmd.Name != tc.command || request.Long != tc.long {
                t.Fatalf("request = %+v; want command %q, long %v", request, tc.command, tc.long)
            }
        })
    }
}

func TestDeclaredFlagsStillBind(t *testing.T) {
    cli, err := Parse([]string{"--required", "yes", "shadow", "-h", "-V"})
    if err != nil || cli == nil || cli.Shadow == nil || !cli.Shadow.Help || !cli.Shadow.Version {
        t.Fatalf("declared flags should bind normally: %#v, %v", cli, err)
    }
}

func TestDisabledFlagsDoNotRequestHelp(t *testing.T) {
    for _, flag := range []string{"--help", "-h", "--version", "-V"} {
        _, err := Parse([]string{"--required", "yes", "disabled", flag})
        var request *argv.Error
        if errors.As(err, &request) && (request.Code == argv.CodeHelp || request.Code == argv.CodeVersion) {
            t.Fatalf("disabled %s produced a request: %v", flag, err)
        }
    }
}
`,
	}
	for name, contents := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	cmd := exec.Command(findUsage(t), "generate", "go", "-f", filepath.Join(dir, "cli.usage.kdl"), "--package", "generated", "-o", filepath.Join(dir, "tables.go"))
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("generate: %v\n%s", err, out)
	}
	cmd = exec.Command("go", "test", "-mod=mod", ".")
	cmd.Dir = dir
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("generated parser: %v\n%s", err, out)
	}
}
