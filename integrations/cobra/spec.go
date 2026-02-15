// Package cobra_usage converts Cobra command trees into usage specs.
package cobra_usage

// Spec is the root struct representing a CLI definition.
// Used internally for KDL rendering.
type Spec struct {
	Name    string
	Bin     string
	Version string
	About   string
	Long    string
	Usage   string
	Flags   []SpecFlag
	Args    []SpecArg
	Cmds    []SpecCommand
}

// SpecCommand represents a subcommand.
type SpecCommand struct {
	Name               string
	Help               string
	HelpLong           string
	Hide               bool
	Deprecated         string
	Aliases            []string
	SubcommandRequired bool
	Flags              []SpecFlag
	Args               []SpecArg
	Cmds               []SpecCommand
}

// SpecFlag represents a flag/option definition.
type SpecFlag struct {
	Short      string
	Long       string
	Help       string
	HelpLong   string
	Required   bool
	Hide       bool
	Global     bool
	Count      bool
	Var        bool
	Deprecated string
	Default    []string
	Arg        *SpecArg
}

// SpecArg represents a positional argument.
type SpecArg struct {
	Name     string
	Help     string
	Required bool
	Var      bool
	Hide     bool
	Default  []string
	Choices  *SpecChoices
}

// SpecChoices represents a set of valid choices for an argument.
type SpecChoices struct {
	Values []string
}
