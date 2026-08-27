package cobra_usage

import (
	"fmt"
	"strings"
)

// JSON types matching the usage-lib JSON schema (Spec -> serde serialization).

type jsonSpec struct {
	Name     string        `json:"name,omitempty"`
	Bin      string        `json:"bin,omitempty"`
	Cmd      jsonCommand   `json:"cmd"`
	Version  string        `json:"version,omitempty"`
	Usage    string        `json:"usage,omitempty"`
	About    string        `json:"about,omitempty"`
	AboutLon string        `json:"about_long,omitempty"`
	Examples []jsonExample `json:"examples,omitempty"`
}

type jsonCommand struct {
	FullCmd            []string                `json:"full_cmd"`
	Usage              string                  `json:"usage,omitempty"`
	Subcommands        map[string]*jsonCommand `json:"subcommands"`
	Args               []jsonArg               `json:"args,omitempty"`
	Flags              []jsonFlag              `json:"flags,omitempty"`
	Hide               bool                    `json:"hide"`
	SubcommandRequired bool                    `json:"subcommand_required,omitempty"`
	Name               string                  `json:"name"`
	Help               string                  `json:"help,omitempty"`
	HelpLong           string                  `json:"help_long,omitempty"`
	Deprecated         string                  `json:"deprecated,omitempty"`
	Aliases            []string                `json:"aliases"`
	HiddenAliases      []string                `json:"hidden_aliases"`
	Examples           []jsonExample           `json:"examples"`
}

type jsonFlag struct {
	Name       string   `json:"name"`
	Usage      string   `json:"usage,omitempty"`
	Help       string   `json:"help,omitempty"`
	Short      []string `json:"short"`
	Long       []string `json:"long"`
	Required   bool     `json:"required,omitempty"`
	Hide       bool     `json:"hide"`
	Global     bool     `json:"global"`
	Count      bool     `json:"count,omitempty"`
	Var        bool     `json:"var,omitempty"`
	Deprecated string   `json:"deprecated,omitempty"`
	Default    []string `json:"default,omitempty"`
	Arg        *jsonArg `json:"arg,omitempty"`
}

type jsonArg struct {
	Name     string       `json:"name"`
	Usage    string       `json:"usage,omitempty"`
	Help     string       `json:"help,omitempty"`
	Required bool         `json:"required"`
	Hide     bool         `json:"hide"`
	Var      bool         `json:"var,omitempty"`
	Default  []string     `json:"default,omitempty"`
	Choices  *jsonChoices `json:"choices,omitempty"`
}

type jsonChoices struct {
	Choices []string `json:"choices"`
}

// jsonExample mirrors usage-lib's SpecExample, which derives a plain Serialize:
// every field is always present, and header/help are null when unset.
type jsonExample struct {
	Code   string  `json:"code"`
	Header *string `json:"header"`
	Help   *string `json:"help"`
	Lang   string  `json:"lang"`
}

// toJSON converts internal Spec to the JSON-serializable format matching usage-lib.
func toJSON(spec *Spec) jsonSpec {
	js := jsonSpec{
		Name:     spec.Name,
		Bin:      spec.Bin,
		Version:  spec.Version,
		About:    spec.About,
		AboutLon: spec.Long,
	}

	js.Examples = examplesToJSON(spec.Examples)

	// Root command
	js.Cmd = jsonCommand{
		FullCmd:       []string{},
		Name:          spec.Name,
		Aliases:       []string{},
		HiddenAliases: []string{},
		Examples:      []jsonExample{},
		Subcommands:   make(map[string]*jsonCommand),
	}

	for _, f := range spec.Flags {
		js.Cmd.Flags = append(js.Cmd.Flags, flagToJSON(&f))
	}
	for _, a := range spec.Args {
		js.Cmd.Args = append(js.Cmd.Args, argToJSON(&a))
	}
	for _, c := range spec.Cmds {
		jc := commandToJSON(&c, []string{c.Name})
		js.Cmd.Subcommands[c.Name] = &jc
	}

	return js
}

// commandToJSON converts a SpecCommand and its subcommand tree into the JSON
// shape usage-lib expects, threading the full command path down to each child.
func commandToJSON(cmd *SpecCommand, fullCmd []string) jsonCommand {
	jc := jsonCommand{
		FullCmd:            fullCmd,
		Name:               cmd.Name,
		Help:               cmd.Help,
		HelpLong:           cmd.HelpLong,
		Hide:               cmd.Hide,
		Deprecated:         cmd.Deprecated,
		SubcommandRequired: cmd.SubcommandRequired,
		Aliases:            cmd.Aliases,
		HiddenAliases:      []string{},
		Examples:           examplesToJSON(cmd.Examples),
		Subcommands:        make(map[string]*jsonCommand),
	}
	if jc.Aliases == nil {
		jc.Aliases = []string{}
	}

	for _, f := range cmd.Flags {
		jc.Flags = append(jc.Flags, flagToJSON(&f))
	}
	for _, a := range cmd.Args {
		jc.Args = append(jc.Args, argToJSON(&a))
	}
	for _, sub := range cmd.Cmds {
		childPath := append(append([]string{}, fullCmd...), sub.Name)
		sc := commandToJSON(&sub, childPath)
		jc.Subcommands[sub.Name] = &sc
	}

	return jc
}

// examplesToJSON converts internal examples, always returning a non-nil slice so
// the field marshals as [] rather than null - usage-lib's SpecCommand.examples
// has no skip_serializing_if and always emits an array.
func examplesToJSON(examples []SpecExample) []jsonExample {
	out := make([]jsonExample, 0, len(examples))
	for i := range examples {
		out = append(out, exampleToJSON(&examples[i]))
	}
	return out
}

// exampleToJSON converts a single example, leaving the optional header and help
// fields nil when unset so they marshal as null rather than "".
func exampleToJSON(ex *SpecExample) jsonExample {
	je := jsonExample{
		Code: ex.Code,
		Lang: ex.Lang,
	}
	if ex.Header != "" {
		je.Header = &ex.Header
	}
	if ex.Help != "" {
		je.Help = &ex.Help
	}
	return je
}

// flagToJSON converts a SpecFlag, deriving the flag's name and usage string from
// its long and short spellings and its optional value argument.
func flagToJSON(f *SpecFlag) jsonFlag {
	jf := jsonFlag{
		Help:       f.Help,
		Required:   f.Required,
		Hide:       f.Hide,
		Global:     f.Global,
		Count:      f.Count,
		Var:        f.Var,
		Deprecated: f.Deprecated,
		Default:    f.Default,
		Short:      []string{},
		Long:       []string{},
	}

	if f.Short != "" {
		jf.Short = []string{f.Short}
	}
	if f.Long != "" {
		jf.Long = []string{f.Long}
	}

	// Derive name from long or short
	if f.Long != "" {
		jf.Name = f.Long
	} else if f.Short != "" {
		jf.Name = f.Short
	}

	// Build usage string
	var parts []string
	if f.Short != "" {
		parts = append(parts, "-"+f.Short)
	}
	if f.Long != "" {
		parts = append(parts, "--"+f.Long)
	}
	usage := strings.Join(parts, " ")
	if f.Arg != nil {
		usage = fmt.Sprintf("%s <%s>", usage, f.Arg.Name)
	}
	jf.Usage = usage

	if f.Arg != nil {
		a := argToJSON(f.Arg)
		jf.Arg = &a
	}

	return jf
}

// argToJSON converts a SpecArg, rendering its usage string as <name> when the arg
// is required and [name] when it is not.
func argToJSON(a *SpecArg) jsonArg {
	ja := jsonArg{
		Name:     a.Name,
		Help:     a.Help,
		Required: a.Required,
		Hide:     a.Hide,
		Var:      a.Var,
		Default:  a.Default,
	}

	// Build usage string
	if a.Required {
		ja.Usage = fmt.Sprintf("<%s>", a.Name)
	} else {
		ja.Usage = fmt.Sprintf("[%s]", a.Name)
	}

	if a.Choices != nil {
		ja.Choices = &jsonChoices{Choices: a.Choices.Values}
	}

	return ja
}
