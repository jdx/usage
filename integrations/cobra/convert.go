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
		if len(sc.Args) == 0 {
			sc.SubcommandRequired = true
		}
	}

	return sc
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

	// Set default value (skip zero values)
	if f.DefValue != "" && f.DefValue != "false" && f.DefValue != "0" && f.DefValue != "[]" {
		sf.Default = []string{f.DefValue}
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
		// Skip [flags] or [OPTIONS] placeholders
		lower := strings.ToLower(token)
		if lower == "[flags]" || lower == "[options]" {
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
func isBuiltinCommand(cmd *cobra.Command) bool {
	name := cmd.Name()
	return name == "help" || name == "completion"
}

// isBuiltinFlag returns true for Cobra's auto-generated flags.
func isBuiltinFlag(f *pflag.Flag) bool {
	return f.Name == "help" || f.Name == "version"
}
