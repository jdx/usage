package cobra_usage

import (
	"strings"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// convertRoot converts a root cobra.Command into a Spec.
func convertRoot(cmd *cobra.Command) Spec {
	spec := Spec{
		Name: cmd.Name(),
		Bin:  cmd.Name(),
	}
	if cmd.Version != "" {
		spec.Version = cmd.Version
	}
	if cmd.Short != "" {
		spec.About = cmd.Short
	}
	if cmd.Long != "" {
		spec.Long = cmd.Long
	}
	// The root's examples belong at spec level, which is where a top-level KDL
	// example node parses to.
	spec.Examples = convertExamples(cmd.Example)

	spec.Flags = convertPersistentFlags(cmd)
	spec.Flags = append(spec.Flags, convertLocalFlags(cmd, true)...)
	spec.Args = parseArgsFromUse(cmd)

	if len(cmd.ValidArgs) > 0 && len(spec.Args) > 0 {
		spec.Args[0].Choices = &SpecChoices{Values: cmd.ValidArgs}
	}

	for _, sub := range cmd.Commands() {
		if isBuiltinCommand(sub) {
			continue
		}
		spec.Cmds = append(spec.Cmds, convertCommand(sub))
	}
	return spec
}

// convertCommand recursively converts a cobra.Command into a SpecCommand.
func convertCommand(cmd *cobra.Command) SpecCommand {
	sc := SpecCommand{
		Name: cmd.Name(),
	}
	if cmd.Short != "" {
		sc.Help = cmd.Short
	}
	if cmd.Long != "" {
		sc.HelpLong = cmd.Long
	}
	sc.Examples = convertExamples(cmd.Example)
	if cmd.Hidden {
		sc.Hide = true
	}
	if cmd.Deprecated != "" {
		sc.Deprecated = cmd.Deprecated
	}
	if len(cmd.Aliases) > 0 {
		sc.Aliases = cmd.Aliases
	}

	sc.Flags = convertPersistentFlags(cmd)
	sc.Flags = append(sc.Flags, convertLocalFlags(cmd, false)...)
	sc.Args = parseArgsFromUse(cmd)

	if len(cmd.ValidArgs) > 0 && len(sc.Args) > 0 {
		sc.Args[0].Choices = &SpecChoices{Values: cmd.ValidArgs}
	}

	var subcommands []SpecCommand
	for _, sub := range cmd.Commands() {
		if isBuiltinCommand(sub) {
			continue
		}
		subcommands = append(subcommands, convertCommand(sub))
	}
	if len(subcommands) > 0 {
		sc.Cmds = subcommands
		// Only require a subcommand if the command itself is not runnable
		// and has no positional args defined.
		if len(sc.Args) == 0 && !isRunnable(cmd) {
			sc.SubcommandRequired = true
		}
	}

	return sc
}

// convertExamples turns a cobra Example string into spec examples.
// Cobra's Example is one free-form block with no header/help split of its own, so
// it maps to a single example node holding the whole dedented text as its code.
// Comment lines the author wrote stay inside that code rather than being lifted
// out, so nothing is reordered or dropped.
func convertExamples(example string) []SpecExample {
	code := dedent(example)
	if code == "" {
		return nil
	}
	return []SpecExample{{Code: code}}
}

// dedent normalizes line endings, drops surrounding blank lines, strips the
// leading whitespace prefix shared by every non-blank line, and trims trailing
// whitespace from each line. Cobra examples are conventionally written indented
// inside a raw string literal; without this every line would render indented
// inside the generated code block.
func dedent(s string) string {
	s = strings.ReplaceAll(s, "\r\n", "\n")
	s = strings.ReplaceAll(s, "\r", "\n")

	lines := strings.Split(s, "\n")
	for i, line := range lines {
		lines[i] = strings.TrimRight(line, " \t")
	}
	for len(lines) > 0 && lines[0] == "" {
		lines = lines[1:]
	}
	for len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	if len(lines) == 0 {
		return ""
	}

	prefix := ""
	seen := false
	for _, line := range lines {
		if line == "" {
			continue
		}
		indent := line[:len(line)-len(strings.TrimLeft(line, " \t"))]
		if !seen {
			prefix = indent
			seen = true
			continue
		}
		prefix = commonPrefix(prefix, indent)
		if prefix == "" {
			break
		}
	}
	if prefix != "" {
		for i, line := range lines {
			lines[i] = strings.TrimPrefix(line, prefix)
		}
	}

	return strings.Join(lines, "\n")
}

// commonPrefix returns the longest common leading run of a and b.
func commonPrefix(a, b string) string {
	n := len(a)
	if len(b) < n {
		n = len(b)
	}
	i := 0
	for i < n && a[i] == b[i] {
		i++
	}
	return a[:i]
}

// convertPersistentFlags converts persistent flags from a command (global=true).
func convertPersistentFlags(cmd *cobra.Command) []SpecFlag {
	var flags []SpecFlag
	cmd.PersistentFlags().VisitAll(func(f *pflag.Flag) {
		if isBuiltinFlag(f) {
			return
		}
		sf := convertFlag(f)
		sf.Global = true
		flags = append(flags, sf)
	})
	return flags
}

// convertLocalFlags converts local (non-persistent) flags.
// skipPersistent skips flags that are also in the persistent set (for root command).
func convertLocalFlags(cmd *cobra.Command, isRoot bool) []SpecFlag {
	var flags []SpecFlag
	cmd.LocalFlags().VisitAll(func(f *pflag.Flag) {
		if isBuiltinFlag(f) {
			return
		}
		// Skip persistent flags that already got handled
		if isRoot && cmd.PersistentFlags().Lookup(f.Name) != nil {
			return
		}
		flags = append(flags, convertFlag(f))
	})
	return flags
}

// convertFlag converts a pflag.Flag into a SpecFlag.
func convertFlag(f *pflag.Flag) SpecFlag {
	sf := SpecFlag{}

	if f.Shorthand != "" {
		sf.Short = f.Shorthand
	}
	if f.Name != "" {
		sf.Long = f.Name
	}
	if f.Usage != "" {
		sf.Help = f.Usage
	}
	if f.Hidden {
		sf.Hide = true
	}
	if f.Deprecated != "" {
		sf.Deprecated = f.Deprecated
	}

	annotations := f.Annotations
	if annotations != nil {
		if _, ok := annotations[cobra.BashCompOneRequiredFlag]; ok {
			sf.Required = true
		}
	}

	typeName := f.Value.Type()

	switch typeName {
	case "bool":
		// Boolean flags have no arg child
	case "count":
		sf.Count = true
		sf.Var = true
	default:
		argName := strings.ToUpper(strings.ReplaceAll(f.Name, "-", "_"))
		arg := &SpecArg{
			Name:     argName,
			Required: true,
		}
		sf.Arg = arg
	}

	// Set default value, skipping type-appropriate zero values.
	if f.DefValue != "" && f.DefValue != "[]" {
		switch typeName {
		case "bool":
			// Skip "false" for bool flags (the natural zero value)
			if f.DefValue != "false" {
				sf.Default = []string{f.DefValue}
			}
		case "count":
			// Skip "0" for count flags
			if f.DefValue != "0" {
				sf.Default = []string{f.DefValue}
			}
		default:
			sf.Default = []string{f.DefValue}
		}
	}

	return sf
}

// parseArgsFromUse parses positional argument definitions from cmd.Use.
// Cobra's Use field has the format "command [flags] <required> [optional] [files...]"
func parseArgsFromUse(cmd *cobra.Command) []SpecArg {
	use := cmd.Use
	// Extract just the args portion (after the command name)
	parts := strings.Fields(use)
	if len(parts) <= 1 {
		return nil
	}

	var args []SpecArg
	for _, token := range parts[1:] {
		// Skip common Cobra placeholders that are not real positional args
		lower := strings.ToLower(token)
		switch lower {
		case "[flags]", "[options]", "[command]", "[subcommand]":
			continue
		}

		arg := parseArgToken(token)
		if arg != nil {
			args = append(args, *arg)
		}
	}
	return args
}

// parseArgToken parses a single argument token like "<file>", "[name]", "<files>...", etc.
func parseArgToken(token string) *SpecArg {
	arg := &SpecArg{}

	// Check for variadic suffix
	if strings.HasSuffix(token, "...") {
		arg.Var = true
		token = strings.TrimSuffix(token, "...")
	}

	// Determine required vs optional
	if strings.HasPrefix(token, "<") && strings.HasSuffix(token, ">") {
		arg.Required = true
		arg.Name = strings.TrimPrefix(strings.TrimSuffix(token, ">"), "<")
	} else if strings.HasPrefix(token, "[") && strings.HasSuffix(token, "]") {
		arg.Required = false
		arg.Name = strings.TrimPrefix(strings.TrimSuffix(token, "]"), "[")
	} else {
		// Not a recognized arg pattern
		return nil
	}

	return arg
}

// isBuiltinCommand returns true for Cobra's auto-generated commands.
// Uses the command's annotations to detect the built-in help and completion
// commands rather than matching by name, so user-defined commands with
// those names are preserved.
func isBuiltinCommand(cmd *cobra.Command) bool {
	if cmd.Annotations != nil {
		if _, ok := cmd.Annotations["cobra_annotation_command_is_help_command"]; ok {
			return true
		}
		if _, ok := cmd.Annotations["cobra_annotation_command_is_completion_command"]; ok {
			return true
		}
	}
	// Cobra's built-in help command has no Run/RunE set by default and its
	// name is "help". The built-in completion command similarly has name
	// "completion". Fall back to name matching only when the command has no
	// custom Run handler, indicating it is likely the auto-generated one.
	name := cmd.Name()
	if (name == "help" || name == "completion") && !isRunnable(cmd) {
		return true
	}
	return false
}

// isBuiltinFlag returns true for Cobra's auto-generated flags.
// Checks whether the flag was added by Cobra itself rather than by user code,
// using pflag's Annotation field that Cobra sets on its own flags.
func isBuiltinFlag(f *pflag.Flag) bool {
	if f.Annotations != nil {
		if _, ok := f.Annotations["cobra_annotation_bash_completion_one_required_flag"]; ok {
			// This is a user flag with required annotation, not built-in
			return false
		}
	}
	// Cobra always adds --help and optionally --version. These are the only
	// flags we skip. We check by name since Cobra doesn't annotate them.
	return f.Name == "help" || f.Name == "version"
}

// isRunnable returns true if the command has a Run or RunE handler.
func isRunnable(cmd *cobra.Command) bool {
	return cmd.Run != nil || cmd.RunE != nil
}
