package cobra_usage

import (
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

func TestSimpleCommand(t *testing.T) {
	cmd := &cobra.Command{
		Use:     "mycli",
		Short:   "A simple CLI",
		Version: "1.0.0",
	}
	cmd.Flags().BoolP("verbose", "v", false, "Enable verbose output")
	cmd.Flags().StringP("config", "c", "", "Config file path")

	got := Generate(cmd)

	assertContains(t, got, `name mycli`)
	assertContains(t, got, `bin mycli`)
	assertContains(t, got, `version "1.0.0"`)
	assertContains(t, got, `about "A simple CLI"`)
	assertContains(t, got, `flag "-v --verbose" help="Enable verbose output"`)
	assertContains(t, got, `flag "-c --config" help="Config file path"`)
	assertContains(t, got, `arg <CONFIG>`)
}

func TestNestedSubcommands(t *testing.T) {
	root := &cobra.Command{Use: "app", Short: "An app"}
	sub := &cobra.Command{Use: "sub", Short: "A subcommand"}
	nested := &cobra.Command{Use: "nested", Short: "A nested command"}
	sub.AddCommand(nested)
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `cmd sub`)
	assertContains(t, got, `help="A subcommand"`)
	assertContains(t, got, `cmd nested help="A nested command"`)
}

func TestPersistentFlags(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	root.PersistentFlags().BoolP("debug", "d", false, "Enable debug mode")

	sub := &cobra.Command{Use: "run", Short: "Run something"}
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `flag "-d --debug" help="Enable debug mode" global=#true`)
}

func TestRequiredFlags(t *testing.T) {
	cmd := &cobra.Command{Use: "deploy"}
	cmd.Flags().String("env", "", "Target environment")
	cmd.MarkFlagRequired("env")

	got := Generate(cmd)

	assertContains(t, got, `flag --env help="Target environment" required=#true`)
}

func TestHiddenAndDeprecated(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	hidden := &cobra.Command{Use: "internal", Short: "Internal command", Hidden: true}
	deprecated := &cobra.Command{Use: "old", Short: "Old command", Deprecated: "use new instead"}
	root.AddCommand(hidden, deprecated)

	root.Flags().String("secret", "", "Secret flag")
	root.Flags().MarkHidden("secret")

	got := Generate(root)

	assertContains(t, got, `cmd internal hide=#true help="Internal command"`)
	assertContains(t, got, `cmd old help="Old command" deprecated="use new instead"`)
	assertContains(t, got, `flag --secret help="Secret flag" hide=#true`)
}

func TestArgInference(t *testing.T) {
	tests := []struct {
		name     string
		use      string
		expected []string
	}{
		{
			name:     "required arg",
			use:      "cmd <file>",
			expected: []string{"arg <file>"},
		},
		{
			name:     "optional arg",
			use:      "cmd [name]",
			expected: []string{`arg "[name]" required=#false`},
		},
		{
			name:     "variadic arg",
			use:      "cmd <files>...",
			expected: []string{"arg <files>\u2026 var=#true"},
		},
		{
			name:     "mixed args",
			use:      "cmd <source> [dest]",
			expected: []string{"arg <source>", `arg "[dest]" required=#false`},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cmd := &cobra.Command{Use: tt.use}
			got := Generate(cmd)
			for _, exp := range tt.expected {
				assertContains(t, got, exp)
			}
		})
	}
}

func TestValidArgsChoices(t *testing.T) {
	cmd := &cobra.Command{
		Use:       "deploy <env>",
		ValidArgs: []string{"dev", "staging", "prod"},
	}

	got := Generate(cmd)

	assertContains(t, got, `arg <env>`)
	assertContains(t, got, `choices {`)
	assertContains(t, got, `"dev"`)
	assertContains(t, got, `"staging"`)
	assertContains(t, got, `"prod"`)
}

func TestCountFlag(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().CountP("verbose", "v", "Increase verbosity")

	got := Generate(cmd)

	assertContains(t, got, `flag "-v --verbose" help="Increase verbosity" var=#true count=#true`)
}

func TestDefaultValues(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().String("output", "json", "Output format")
	cmd.Flags().Int("retries", 3, "Number of retries")

	got := Generate(cmd)

	assertContains(t, got, `flag --output help="Output format" default=json`)
	assertContains(t, got, `flag --retries help="Number of retries" default="3"`)
}

func TestBoolFlagNoArg(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().Bool("force", false, "Force the operation")

	got := Generate(cmd)

	line := findLine(got, "flag --force")
	if line == "" {
		t.Fatal("expected flag --force in output")
	}
	if strings.Contains(line, "arg") {
		t.Errorf("bool flag should not have arg child, got: %s", line)
	}
}

func TestSkipsBuiltinCommands(t *testing.T) {
	root := &cobra.Command{Use: "app", Version: "1.0.0"}
	root.AddCommand(&cobra.Command{Use: "run", Short: "Run"})
	// Cobra auto-adds "help" and "completion" commands

	got := Generate(root)

	assertNotContains(t, got, `cmd help`)
	assertNotContains(t, got, `cmd completion`)
	assertContains(t, got, `cmd run`)
}

func TestSkipsBuiltinFlags(t *testing.T) {
	cmd := &cobra.Command{Use: "app", Version: "1.0.0"}
	cmd.Flags().String("custom", "", "A custom flag")

	got := Generate(cmd)

	assertNotContains(t, got, `flag --help`)
	assertNotContains(t, got, `flag --version`)
	assertContains(t, got, `flag --custom`)
}

func TestSubcommandRequired(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	sub := &cobra.Command{Use: "config", Short: "Manage config"}
	sub.AddCommand(&cobra.Command{Use: "get", Short: "Get a value"})
	sub.AddCommand(&cobra.Command{Use: "set", Short: "Set a value"})
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `subcommand_required=#true`)
}

func TestAliases(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	sub := &cobra.Command{
		Use:     "install",
		Short:   "Install packages",
		Aliases: []string{"i", "add"},
	}
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `alias i add`)
}

func TestGenerateJSON(t *testing.T) {
	cmd := &cobra.Command{
		Use:     "mycli",
		Short:   "A CLI tool",
		Version: "2.0.0",
	}
	sub := &cobra.Command{Use: "run", Short: "Run something"}
	cmd.AddCommand(sub)

	data, err := GenerateJSON(cmd)
	if err != nil {
		t.Fatalf("GenerateJSON failed: %v", err)
	}

	jsonStr := string(data)
	assertContains(t, jsonStr, `"name": "mycli"`)
	assertContains(t, jsonStr, `"version": "2.0.0"`)
	assertContains(t, jsonStr, `"about": "A CLI tool"`)
	// JSON uses root "cmd" object with map-based subcommands
	assertContains(t, jsonStr, `"cmd"`)
	assertContains(t, jsonStr, `"subcommands"`)
	assertContains(t, jsonStr, `"run"`)
}

func TestGenerateJSONChoices(t *testing.T) {
	cmd := &cobra.Command{
		Use:       "deploy <env>",
		ValidArgs: []string{"dev", "prod"},
	}

	data, err := GenerateJSON(cmd)
	if err != nil {
		t.Fatalf("GenerateJSON failed: %v", err)
	}

	jsonStr := string(data)
	// JSON uses "choices" key inside choices object, not "values"
	assertContains(t, jsonStr, `"choices"`)
	assertContains(t, jsonStr, `"dev"`)
	assertContains(t, jsonStr, `"prod"`)
	assertNotContains(t, jsonStr, `"values"`)
}

func TestLongHelp(t *testing.T) {
	root := &cobra.Command{
		Use:   "app",
		Short: "Short help",
		Long:  "This is a much longer description of the app.",
	}

	got := Generate(root)

	assertContains(t, got, `about "Short help"`)
	assertContains(t, got, `long_about "This is a much longer description of the app."`)
}

func TestRunnableCommandWithSubcommands(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	sub := &cobra.Command{
		Use:   "task",
		Short: "Run a task",
		Run:   func(cmd *cobra.Command, args []string) {},
	}
	sub.AddCommand(&cobra.Command{Use: "list", Short: "List tasks"})
	root.AddCommand(sub)

	got := Generate(root)

	// "task" has a Run handler, so subcommand_required should NOT be set on it
	taskLine := findLine(got, "cmd task")
	if strings.Contains(taskLine, "subcommand_required") {
		t.Errorf("runnable command should not have subcommand_required, got: %s", taskLine)
	}
}

func TestCommandPlaceholderSkipped(t *testing.T) {
	root := &cobra.Command{Use: "app [command]"}
	root.AddCommand(&cobra.Command{Use: "sub", Short: "A sub"})

	got := Generate(root)

	// [command] is a Cobra placeholder, not a real arg
	assertNotContains(t, got, `arg "[command]"`)
	assertContains(t, got, `cmd sub`)
}

func TestStringDefaultZero(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().String("port", "0", "Port number")

	got := Generate(cmd)

	// "0" is a valid string default and should be preserved
	assertContains(t, got, `default="0"`)
}

// --- helpers ---

func assertContains(t *testing.T, got, want string) {
	t.Helper()
	if !strings.Contains(got, want) {
		t.Errorf("output does not contain %q\ngot:\n%s", want, got)
	}
}

func assertNotContains(t *testing.T, got, unwanted string) {
	t.Helper()
	if strings.Contains(got, unwanted) {
		t.Errorf("output should not contain %q\ngot:\n%s", unwanted, got)
	}
}

func findLine(output, prefix string) string {
	for _, line := range strings.Split(output, "\n") {
		if strings.Contains(line, prefix) {
			return line
		}
	}
	return ""
}
