package cobra_usage

import (
	"fmt"
	"strings"
)

// JSON types matching the usage-lib JSON schema (Spec -> serde serialization).

type jsonSpec struct {
	Name     string       `json:"name,omitempty"`
	Bin      string       `json:"bin,omitempty"`
	Cmd      jsonCommand  `json:"cmd"`
	Version  string       `json:"version,omitempty"`
	Usage    string       `json:"usage,omitempty"`
	About    string       `json:"about,omitempty"`
	AboutLon string       `json:"about_long,omitempty"`
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

// toJSON converts internal Spec to the JSON-serializable format matching usage-lib.
func toJSON(spec *Spec) jsonSpec {
	js := jsonSpec{
		Name:     spec.Name,
		Bin:      spec.Bin,
		Version:  spec.Version,
		About:    spec.About,
		AboutLon: spec.Long,
	}

	// Root command
	js.Cmd = jsonCommand{
		FullCmd:       []string{},
		Name:          spec.Name,
		Aliases:       []string{},
		HiddenAliases: []string{},
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
