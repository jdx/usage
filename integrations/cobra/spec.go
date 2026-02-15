// Package cobra_usage converts Cobra command trees into usage specs.
package cobra_usage

// Spec is the root struct representing a CLI definition.
type Spec struct {
	Name    string        `json:"name,omitempty"`
	Bin     string        `json:"bin,omitempty"`
	Version string        `json:"version,omitempty"`
	About   string        `json:"about,omitempty"`
	Long    string        `json:"long_about,omitempty"`
	Usage   string        `json:"usage,omitempty"`
	Flags   []SpecFlag    `json:"flags,omitempty"`
	Args    []SpecArg     `json:"args,omitempty"`
	Cmds    []SpecCommand `json:"subcommands,omitempty"`
}

// SpecCommand represents a subcommand.
type SpecCommand struct {
	Name               string        `json:"name"`
	Help               string        `json:"help,omitempty"`
	HelpLong           string        `json:"help_long,omitempty"`
	Hide               bool          `json:"hide,omitempty"`
	Deprecated         string        `json:"deprecated,omitempty"`
	Aliases            []string      `json:"aliases,omitempty"`
	SubcommandRequired bool          `json:"subcommand_required,omitempty"`
	Flags              []SpecFlag    `json:"flags,omitempty"`
	Args               []SpecArg     `json:"args,omitempty"`
	Cmds               []SpecCommand `json:"subcommands,omitempty"`
}

// SpecFlag represents a flag/option definition.
type SpecFlag struct {
	Short      string   `json:"short,omitempty"`
	Long       string   `json:"long,omitempty"`
	Help       string   `json:"help,omitempty"`
	HelpLong   string   `json:"help_long,omitempty"`
	Required   bool     `json:"required,omitempty"`
	Hide       bool     `json:"hide,omitempty"`
	Global     bool     `json:"global,omitempty"`
	Count      bool     `json:"count,omitempty"`
	Var        bool     `json:"var,omitempty"`
	Deprecated string   `json:"deprecated,omitempty"`
	Default    []string `json:"default,omitempty"`
	Arg        *SpecArg `json:"arg,omitempty"`
}

// SpecArg represents a positional argument.
type SpecArg struct {
	Name     string       `json:"name"`
	Help     string       `json:"help,omitempty"`
	Required bool         `json:"required,omitempty"`
	Var      bool         `json:"var,omitempty"`
	Hide     bool         `json:"hide,omitempty"`
	Default  []string     `json:"default,omitempty"`
	Choices  *SpecChoices `json:"choices,omitempty"`
}

// SpecChoices represents a set of valid choices for an argument.
type SpecChoices struct {
	Values []string `json:"values"`
}
