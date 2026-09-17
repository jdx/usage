package conformance

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// TestGeneratedTypes compiles real generated code and checks resolution before conversion.
func TestGeneratedTypes(t *testing.T) {
	dir := t.TempDir()
	module, err := filepath.Abs("..")
	if err != nil {
		t.Fatal(err)
	}
	files := map[string]string{
		"go.mod": fmt.Sprintf("module example.com/typed\n\ngo 1.26.0\n\nrequire github.com/jdx/usage/go v0.0.0\nreplace github.com/jdx/usage/go => %q\n", filepath.ToSlash(module)),
		"cli.usage.kdl": `name "typed"
bin "typed"
args_override_self #true
flag "--timeout <value>" env="USAGE_TYPED_TIMEOUT" default="2s"
flag "--jobs <value>" env="USAGE_TYPED_JOBS" default="3"
flag "--enabled <value>" default="true"
flag "--ratio <value>" default="1.5"
flag "--unsigned <value>" default="12"
flag "--quick"
flag "-n <number>"
flag "--conditional <value>" {
    default_if "--quick" "1s"
    default "3s"
}
flag "--delays <value>" var=#true
arg "[values]" var=#true
cmd "run" {
    flag "--wait <value>" default="4s"
}
cmd "broken" {
    flag "--wait <value>" default="invalid"
}
`,
		"tables_test.go": `package typed
import (
 "errors"
 "reflect"
 "testing"
 "time"
 "github.com/jdx/usage/go/argv"
)
// TestSources verifies argv, environment, and default precedence before conversion.
func TestSources(t *testing.T) {
 cli,err:=Parse(nil)
 if err!=nil{t.Fatal(err)}
 var _ time.Duration=cli.Timeout
 var _ int=cli.Jobs
 var _ bool=cli.Enabled
 var _ float64=cli.Ratio
 var _ uint64=cli.Unsigned
 var _ []time.Duration=cli.Delays
 var _ []int64=cli.Values
 if cli.Timeout!=2*time.Second||cli.Jobs!=3||!cli.Enabled||cli.Ratio!=1.5||cli.Unsigned!=12||cli.Run!=nil{t.Fatalf("defaults: %+v",cli)}
 t.Setenv("USAGE_TYPED_TIMEOUT","5s")
 t.Setenv("USAGE_TYPED_JOBS","8")
 cli,err=Parse(nil)
 if err!=nil||cli.Timeout!=5*time.Second||cli.Jobs!=8{t.Fatalf("env: %+v %v",cli,err)}
 cli,err=Parse([]string{"--timeout","6s","--jobs","9","--enabled","false","--delays","1s","--delays","2s","10","20"})
 if err!=nil{t.Fatal(err)}
 if cli.Timeout!=6*time.Second||cli.Jobs!=9||cli.Enabled||!reflect.DeepEqual(cli.Delays,[]time.Duration{time.Second,2*time.Second})||!reflect.DeepEqual(cli.Values,[]int64{10,20}){t.Fatalf("argv: %+v",cli)}
 cli,err=Parse([]string{"run"})
 if err!=nil||cli.Run==nil||cli.Run.Wait!=4*time.Second{t.Fatalf("selected command: %+v %v",cli,err)}
}
// TestShortFlagDiagnostic verifies conversion errors retain the short flag name.
func TestShortFlagDiagnostic(t *testing.T) {
 _,err:=Parse([]string{"-n","bad"})
 var e *argv.Error
 if !errors.As(err,&e)||e.Name!="-n"{t.Fatalf("short flag error: %v",err)}
}
// TestInvalidAndConditionalDefaults verifies typed errors and conditional defaults.
func TestInvalidAndConditionalDefaults(t *testing.T) {
 cli,err:=Parse([]string{"--quick"})
 if err!=nil||cli.Conditional!=time.Second{t.Fatalf("conditional default: %+v %v",cli,err)}
 cli,err=Parse(nil)
 if err!=nil||cli.Conditional!=3*time.Second{t.Fatalf("default: %+v %v",cli,err)}
 for _,args:=range [][]string{{"broken"},{"--unsigned=-1"},{"--jobs","999999999999999999999999"},{"--delays","bad"},{"oops"}} {
  cli,err:=Parse(args)
  var e *argv.Error
  if cli!=nil||!errors.As(err,&e)||e.Code!=argv.CodeInvalidValue{t.Fatalf("invalid typed values %q: %+v %v",args,cli,err)}
 }
}
// TestOnlyResolvedValuesConvert verifies overridden values are not converted.
func TestOnlyResolvedValuesConvert(t *testing.T) {
 t.Setenv("USAGE_TYPED_TIMEOUT","invalid")
 cli,err:=Parse([]string{"--timeout","bad","--timeout","3s"})
 if err!=nil||cli.Timeout!=3*time.Second{t.Fatalf("overridden values must not convert: %+v %v",cli,err)}
 for _,args:=range [][]string{nil,{"--timeout","bad"},{"--timeout="}}{
  cli,err:=Parse(args)
  var e *argv.Error
  if cli!=nil||!errors.As(err,&e)||e.Code!=argv.CodeInvalidValue||e.Name!="--timeout"{t.Fatalf("Parse(%q): %+v %v",args,cli,err)}
 }
 _,err=Parse([]string{"--help"})
 var e *argv.Error
 if !errors.As(err,&e)||e.Code!=argv.CodeHelp{t.Fatalf("help must bypass conversion: %v",err)}
}
`,
	}
	for name, contents := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	args := []string{"generate", "go", "-f", filepath.Join(dir, "cli.usage.kdl"), "--package", "typed", "-o", filepath.Join(dir, "tables.go")}
	for _, binding := range []string{"FlagTimeout=duration", "FlagJobs=int", "FlagEnabled=bool", "FlagRatio=float64", "FlagUnsigned=uint64", "FlagDelays=duration", "ArgValues=int64", "FlagRunWait=duration", "FlagBrokenWait=duration", "FlagConditional=duration", "FlagN=int"} {
		args = append(args, "--field-type", binding)
	}
	cmd := exec.Command(findUsage(t), args...)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("generate: %v\n%s", err, out)
	}
	if out, err := exec.Command("gofmt", "-l", filepath.Join(dir, "tables.go")).CombinedOutput(); err != nil || len(out) != 0 {
		t.Fatalf("generated source is not gofmt-clean: %v\n%s", err, out)
	}
	for _, binding := range []string{"FlagMissing=int", "FlagTimeout=wat", "FlagTimeout=int", "FlagQuick=int", "not-a-binding"} {
		rejectedArgs := append(append([]string{}, args...), "--field-type", binding)
		rejected := exec.Command(findUsage(t), rejectedArgs...)
		if out, err := rejected.CombinedOutput(); err == nil {
			t.Fatalf("invalid binding %q accepted: %s", binding, out)
		}
	}
	cmd = exec.Command("go", "test", "-mod=mod", ".")
	cmd.Dir = dir
	// Prevent the caller's environment from influencing the default-value assertions.
	for _, entry := range os.Environ() {
		if !strings.HasPrefix(entry, "USAGE_TYPED_TIMEOUT=") && !strings.HasPrefix(entry, "USAGE_TYPED_JOBS=") {
			cmd.Env = append(cmd.Env, entry)
		}
	}
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("generated tests: %v\n%s", err, out)
	}
}
