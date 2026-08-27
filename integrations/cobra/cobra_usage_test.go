package cobra_usage

import (
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

// TestSimpleCommand checks that a flat command's name, bin, version, about, and
// flags all reach the spec.
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

// TestNestedSubcommands checks that subcommands are converted recursively.
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

// TestPersistentFlags checks that a root persistent flag is marked global.
func TestPersistentFlags(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	root.PersistentFlags().BoolP("debug", "d", false, "Enable debug mode")

	sub := &cobra.Command{Use: "run", Short: "Run something"}
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `flag "-d --debug" help="Enable debug mode" global=#true`)
}

// TestRequiredFlags checks that a flag marked required in Cobra is required in the
// spec.
func TestRequiredFlags(t *testing.T) {
	cmd := &cobra.Command{Use: "deploy"}
	cmd.Flags().String("env", "", "Target environment")
	cmd.MarkFlagRequired("env")

	got := Generate(cmd)

	assertContains(t, got, `flag --env help="Target environment" required=#true`)
}

// TestHiddenAndDeprecated checks that hidden commands, deprecated commands, and
// hidden flags carry their markers through.
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

// TestArgInference checks that required, optional, variadic, and mixed positional
// args are inferred from a command's Use string.
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

// TestValidArgsChoices checks that Cobra's ValidArgs become a choices block on the
// first positional arg.
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

// TestCountFlag checks that a pflag count flag is marked var and count.
func TestCountFlag(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().CountP("verbose", "v", "Increase verbosity")

	got := Generate(cmd)

	assertContains(t, got, `flag "-v --verbose" help="Increase verbosity" var=#true count=#true`)
}

// TestDefaultValues checks that string and int flag defaults are rendered.
func TestDefaultValues(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().String("output", "json", "Output format")
	cmd.Flags().Int("retries", 3, "Number of retries")

	got := Generate(cmd)

	assertContains(t, got, `flag --output help="Output format" default=json`)
	assertContains(t, got, `flag --retries help="Number of retries" default="3"`)
}

// TestBoolFlagNoArg checks that a bool flag takes no value argument.
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

// TestSkipsBuiltinCommands checks that Cobra's auto-added help and completion
// commands are left out of the spec.
func TestSkipsBuiltinCommands(t *testing.T) {
	root := &cobra.Command{Use: "app", Version: "1.0.0"}
	root.AddCommand(&cobra.Command{Use: "run", Short: "Run"})
	// Cobra auto-adds "help" and "completion" commands

	got := Generate(root)

	assertNotContains(t, got, `cmd help`)
	assertNotContains(t, got, `cmd completion`)
	assertContains(t, got, `cmd run`)
}

// TestSkipsBuiltinFlags checks that Cobra's auto-added --help and --version flags
// are left out of the spec.
func TestSkipsBuiltinFlags(t *testing.T) {
	cmd := &cobra.Command{Use: "app", Version: "1.0.0"}
	cmd.Flags().String("custom", "", "A custom flag")

	got := Generate(cmd)

	assertNotContains(t, got, `flag --help`)
	assertNotContains(t, got, `flag --version`)
	assertContains(t, got, `flag --custom`)
}

// TestSubcommandRequired checks that a command with subcommands and no Run handler
// is marked subcommand_required.
func TestSubcommandRequired(t *testing.T) {
	root := &cobra.Command{Use: "app"}
	sub := &cobra.Command{Use: "config", Short: "Manage config"}
	sub.AddCommand(&cobra.Command{Use: "get", Short: "Get a value"})
	sub.AddCommand(&cobra.Command{Use: "set", Short: "Set a value"})
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `subcommand_required=#true`)
}

// TestAliases checks that command aliases are rendered as an alias node.
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

// TestGenerateJSON checks the top-level shape of the JSON output: spec metadata
// plus a root cmd object with map-based subcommands.
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

// TestGenerateJSONChoices checks that arg choices serialize under the "choices"
// key that usage-lib expects, not "values".
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

// TestLongHelp checks that Short and Long map to about and long_about.
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

// TestRunnableCommandWithSubcommands checks that a command with both a Run handler
// and subcommands is not marked subcommand_required.
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

// TestCommandPlaceholderSkipped checks that Cobra's [command] placeholder in a Use
// string is not mistaken for a positional arg.
func TestCommandPlaceholderSkipped(t *testing.T) {
	root := &cobra.Command{Use: "app [command]"}
	root.AddCommand(&cobra.Command{Use: "sub", Short: "A sub"})

	got := Generate(root)

	// [command] is a Cobra placeholder, not a real arg
	assertNotContains(t, got, `arg "[command]"`)
	assertContains(t, got, `cmd sub`)
}

// TestStringDefaultZero checks that the string default "0" is preserved rather than
// treated as an empty zero value.
func TestStringDefaultZero(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().String("port", "0", "Port number")

	got := Generate(cmd)

	// "0" is a valid string default and should be preserved
	assertContains(t, got, `default="0"`)
}

// TestSpecialCharacterEscaping checks that kdlQuoteAlways escapes newlines, tabs,
// carriage returns, backslashes, and double quotes.
func TestSpecialCharacterEscaping(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "newline in help",
			input:    "Line one\nLine two",
			expected: `"Line one\nLine two"`,
		},
		{
			name:     "tab in help",
			input:    "col1\tcol2",
			expected: `"col1\tcol2"`,
		},
		{
			name:     "carriage return in help",
			input:    "text\rmore",
			expected: `"text\rmore"`,
		},
		{
			name:     "backslash in help",
			input:    `path\to\file`,
			expected: `"path\\to\\file"`,
		},
		{
			name:     "double quote in help",
			input:    `say "hello"`,
			expected: `"say \"hello\""`,
		},
		{
			name:     "mixed special characters",
			input:    "line1\nline2\ttab\r\nwindows",
			expected: `"line1\nline2\ttab\r\nwindows"`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := kdlQuoteAlways(tt.input)
			if got != tt.expected {
				t.Errorf("kdlQuoteAlways(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

// TestNewlineInCommandHelp checks that a multi-line Long is escaped into a single
// quoted long_about value.
func TestNewlineInCommandHelp(t *testing.T) {
	root := &cobra.Command{
		Use:   "app",
		Short: "Short help",
		Long:  "First line.\nSecond line.\n\nThird paragraph.",
	}

	got := Generate(root)

	assertContains(t, got, `long_about "First line.\nSecond line.\n\nThird paragraph."`)
}

// TestSpecialCharsInFlagHelp checks that a multi-line flag help string is escaped
// into a single quoted property.
func TestSpecialCharsInFlagHelp(t *testing.T) {
	cmd := &cobra.Command{Use: "app"}
	cmd.Flags().String("format", "", "Output format:\n  json\n  yaml")

	got := Generate(cmd)

	assertContains(t, got, `help="Output format:\n  json\n  yaml"`)
}

// TestCommandExample checks that a subcommand's Example becomes an example node.
func TestCommandExample(t *testing.T) {
	root := &cobra.Command{Use: "mycli"}
	deploy := &cobra.Command{
		Use:     "deploy",
		Short:   "Deploy the app",
		Example: "mycli deploy --env prod",
		Run:     func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(deploy)

	got := Generate(root)

	assertContains(t, got, `example "mycli deploy --env prod"`)
}

// TestRootExample checks that the root command's Example becomes an unindented
// top-level example node.
func TestRootExample(t *testing.T) {
	root := &cobra.Command{
		Use:     "mycli",
		Short:   "A CLI",
		Example: "  mycli --help",
	}

	got := Generate(root)

	// The root's example is a top-level node, so it must not be indented.
	line := findLine(got, "example ")
	if line != `example "mycli --help"` {
		t.Errorf("root example should be an unindented top-level node, got %q", line)
	}
}

// TestExampleMultilineDedent checks that a multi-line Example becomes one example
// node with its shared leading indent removed.
func TestExampleMultilineDedent(t *testing.T) {
	root := &cobra.Command{Use: "mycli"}
	deploy := &cobra.Command{
		Use:   "deploy",
		Short: "Deploy the app",
		Example: `  # deploy to production
  mycli deploy --env prod

  # dry run first
  mycli deploy --dry-run`,
		Run: func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(deploy)

	got := Generate(root)

	// One node holding the whole block, dedented, with newlines escaped.
	assertContains(t, got, `example "# deploy to production\nmycli deploy --env prod\n\n# dry run first\nmycli deploy --dry-run"`)
	// The shared two-space indent is gone from inside the quotes.
	assertNotContains(t, got, `\n  mycli deploy`)
}

// TestExampleRelativeIndentPreserved checks that dedenting keeps indentation that
// is relative to the block's common prefix.
func TestExampleRelativeIndentPreserved(t *testing.T) {
	root := &cobra.Command{Use: "mycli"}
	sub := &cobra.Command{
		Use:   "run",
		Short: "Run it",
		Example: `  mycli run
    --with-continuation`,
		Run: func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `example "mycli run\n  --with-continuation"`)
}

// TestExampleAsOnlyChild checks that an example alone is enough to open a cmd
// node's child block.
func TestExampleAsOnlyChild(t *testing.T) {
	root := &cobra.Command{Use: "mycli"}
	sub := &cobra.Command{
		Use:     "ping",
		Short:   "Ping",
		Example: "mycli ping",
		Run:     func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(sub)

	got := Generate(root)

	// The example is the command's only child, so the block must still open.
	line := findLine(got, `cmd ping`)
	if !strings.HasSuffix(line, "{") {
		t.Errorf("cmd node should open a child block, got %q", line)
	}
	assertContains(t, got, `    example "mycli ping"`)
}

// TestNoExample checks that a command without an Example emits no example node.
func TestNoExample(t *testing.T) {
	root := &cobra.Command{Use: "mycli", Short: "A CLI"}
	sub := &cobra.Command{
		Use:   "ping",
		Short: "Ping",
		Run:   func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(sub)

	got := Generate(root)

	assertNotContains(t, got, "example ")
}

// TestExampleWhitespaceOnly checks that a whitespace-only Example is dropped.
func TestExampleWhitespaceOnly(t *testing.T) {
	root := &cobra.Command{Use: "mycli", Short: "A CLI", Example: "  \n\t\n"}

	got := Generate(root)

	assertNotContains(t, got, "example ")
}

// TestExampleSpecialChars checks that quotes and backslashes inside an example are
// escaped.
func TestExampleSpecialChars(t *testing.T) {
	root := &cobra.Command{Use: "mycli"}
	sub := &cobra.Command{
		Use:     "grep",
		Short:   "Grep",
		Example: `mycli grep "a\b"`,
		Run:     func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(sub)

	got := Generate(root)

	assertContains(t, got, `example "mycli grep \"a\\b\""`)
}

// TestExampleCRLF checks that CRLF line endings are normalized to LF and trailing
// newlines are trimmed.
func TestExampleCRLF(t *testing.T) {
	root := &cobra.Command{Use: "mycli", Example: "mycli one\r\nmycli two\r\n"}

	got := Generate(root)

	assertContains(t, got, `example "mycli one\nmycli two"`)
	assertNotContains(t, got, `\r`)
}

// TestGenerateJSONExamples checks that examples serialize with all four fields
// present and that commands without examples still emit an empty array.
func TestGenerateJSONExamples(t *testing.T) {
	root := &cobra.Command{
		Use:     "mycli",
		Short:   "A CLI",
		Example: "mycli --help",
	}
	deploy := &cobra.Command{
		Use:     "deploy",
		Short:   "Deploy",
		Example: "mycli deploy",
		Run:     func(cmd *cobra.Command, args []string) {},
	}
	plain := &cobra.Command{
		Use:   "status",
		Short: "Status",
		Run:   func(cmd *cobra.Command, args []string) {},
	}
	root.AddCommand(deploy, plain)

	data, err := GenerateJSON(root)
	if err != nil {
		t.Fatalf("GenerateJSON failed: %v", err)
	}
	got := string(data)

	// All four SpecExample fields always serialize, matching usage-lib.
	assertContains(t, got, `"code": "mycli --help"`)
	assertContains(t, got, `"header": null`)
	assertContains(t, got, `"help": null`)
	assertContains(t, got, `"lang": ""`)
	assertContains(t, got, `"code": "mycli deploy"`)
	// Command-level examples always emit an array, mirroring Rust's
	// SpecCommand.examples having no skip_serializing_if.
	assertContains(t, got, `"examples": []`)
}

// --- helpers ---

// assertContains fails the test when got does not contain want.
func assertContains(t *testing.T, got, want string) {
	t.Helper()
	if !strings.Contains(got, want) {
		t.Errorf("output does not contain %q\ngot:\n%s", want, got)
	}
}

// assertNotContains fails the test when got contains unwanted.
func assertNotContains(t *testing.T, got, unwanted string) {
	t.Helper()
	if strings.Contains(got, unwanted) {
		t.Errorf("output should not contain %q\ngot:\n%s", unwanted, got)
	}
}

// findLine returns the first line of output containing prefix, or "" if none does.
func findLine(output, prefix string) string {
	for _, line := range strings.Split(output, "\n") {
		if strings.Contains(line, prefix) {
			return line
		}
	}
	return ""
}
