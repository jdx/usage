package conformance

import "testing"

// TestGeneratedUnknownEnv runs a generated Parse against the real process
// environment, which is what `unknown_env` judges.
func TestGeneratedUnknownEnv(t *testing.T) {
	runGeneratedFixture(t, `name "example"
bin "example"
version "1.0"
unknown_env prefix="EX_" {
  allow "EX_LIBRARY"
}
flag "--port <port>" env="EX_PORT"
cmd "serve" {
  flag "--tls" deprecated_env="EX_SSL"
}
cmd "publish" {
  flag "--name <name>" required=#true env="EX_NAME"
}
config {
  prop "jobs" env="EX_JOBS"
}
`, `package generated
import (
  "errors"
  "os"
  "reflect"
  "strings"
  "testing"
  "github.com/jdx/usage/go/argv"
)
func clearEx(t *testing.T) {
  for _, entry := range os.Environ() {
    if name, _, _ := strings.Cut(entry, "="); strings.HasPrefix(name, "EX_") {
      t.Setenv(name, "")
      os.Unsetenv(name)
    }
  }
}
func TestUnknownEnv(t *testing.T) {
  clearEx(t)
  // Declared by another command, deprecated, read by config, allowed, or outside the prefix.
  for _, name := range []string{"EX_PORT", "EX_SSL", "EX_JOBS", "EX_LIBRARY", "EXTRA"} { t.Setenv(name, "1") }
  t.Setenv("EX_NAME", "n")
  if _, err := Parse([]string{"publish"}); err != nil { t.Fatalf("declared names: %v", err) }

  t.Setenv("EX_TSL", "1")
  t.Setenv("EX_PROT", "1")
  _, err := Parse([]string{"serve"}); var req *argv.Error
  if !errors.As(err, &req) || req.Code != argv.CodeUnknownEnv { t.Fatalf("typos: %v", err) }
  if req.Name != "EX_" || !reflect.DeepEqual(req.Names, []string{"EX_PROT", "EX_TSL"}) { t.Fatalf("every offender, sorted: %#v", req) }
  if got, want := err.Error(), "unknown environment variables under EX_: EX_PROT, EX_TSL"; got != want { t.Fatalf("message: %q", got) }

  // Reported ahead of the requirement it may be the cause of.
  os.Unsetenv("EX_NAME")
  _, err = Parse([]string{"publish"})
  if !errors.As(err, &req) || req.Code != argv.CodeUnknownEnv { t.Fatalf("before requirements: %v", err) }

  // Help and version still answer.
  for _, args := range [][]string{{"--help"}, {"help", "serve"}, {"--version"}} {
    _, err = Parse(args)
    if !errors.As(err, &req) || (req.Code != argv.CodeHelp && req.Code != argv.CodeVersion) { t.Fatalf("%v: %v", args, err) }
  }
}
`, "unknown-env")
}
