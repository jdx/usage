// Package spec reads a usage spec that has been lowered to JSON and builds the
// static tables [argv] binds against.
//
// The spec's own format is KDL, and nothing here parses KDL: the `usage` CLI does
// that, and `usage generate json` is the lowering. That division is deliberate.
// A generated table is produced once, at build time, by a maintainer who already
// has the usage CLI; a program that ships to end users carries the tables and
// never sees a spec at all. Putting a KDL parser in this module would add a
// dependency to every adopter's binary to solve a problem none of them have.
//
// So this package is used by two callers, both of them build-time: the code
// generator, and the conformance suite, which builds tables per vector rather
// than generating a package per vector.
package spec

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/jdx/usage/go/argv"
)

// Spec is a usage spec as `usage generate json` writes it.
//
// Only the fields binding needs are described. The lowering carries much more —
// help text, examples, completers, config — and leaving those out here is the
// same split the hot path makes: this struct is the parse tables' source, and
// help is a cold table generated separately.
type Spec struct {
	Name string `json:"name"`
	Bin  string `json:"bin"`
	Cmd  Cmd    `json:"cmd"`
	// UnknownFlags is the CLI-wide default, which a command may override and
	// which is otherwise inherited all the way down.
	UnknownFlags string `json:"unknown_flags"`
	// DefaultSubcommand is declared once, at the top, and names a subcommand of
	// the root.
	DefaultSubcommand string `json:"default_subcommand"`
	// Multicall is whether argv[0]'s basename selects a subcommand (busybox-style
	// applets). clap's multicall. The dispatcher names (Name / Bin) are skipped;
	// any other basename is parsed as the first word.
	Multicall   bool   `json:"multicall"`
	Version     string `json:"version"`
	LongVersion string `json:"long_version"`
	About       string `json:"about"`
	AboutLong   string `json:"about_long"`
	// Complete is the completers a spec declares, keyed by the lowercased name of
	// the argument or flag value they belong to — which is how usage-lib keys
	// them, and how a lookup has to be spelled.
	Complete       map[string]Completer `json:"complete"`
	BeforeHelp     string               `json:"before_help"`
	AfterHelp      string               `json:"after_help"`
	BeforeHelpLong string               `json:"before_help_long"`
	AfterHelpLong  string               `json:"after_help_long"`
	Usage          string               `json:"usage"`
}

// HelpSpec is what a page needs from the spec's root rather than from a command.
func (s *Spec) HelpSpec() argv.HelpSpec {
	return argv.HelpSpec{
		Name:           s.Name,
		Bin:            s.Bin,
		Version:        s.Version,
		LongVersion:    s.LongVersion,
		About:          s.About,
		LongAbout:      s.AboutLong,
		BeforeHelp:     s.BeforeHelp,
		AfterHelp:      s.AfterHelp,
		BeforeLongHelp: s.BeforeHelpLong,
		AfterLongHelp:  s.AfterHelpLong,
	}
}

// Completer is one `complete` block. Only the type is read here: running the
// `run=` script is a completion's job at run time, and needs a subprocess this
// package has no business starting.
type Completer struct {
	Name string `json:"name"`
	Type string `json:"type_"`
}

// Cmd is one command in the lowered spec.
type Cmd struct {
	Name                        string      `json:"name"`
	Hide                        bool        `json:"hide"`
	Help                        string      `json:"help"`
	HelpLong                    string      `json:"help_long"`
	HelpHeading                 string      `json:"help_heading"`
	DisplayOrder                *uint32     `json:"display_order"`
	Usage                       string      `json:"usage"`
	BeforeHelp                  string      `json:"before_help"`
	AfterHelp                   string      `json:"after_help"`
	BeforeHelpLong              string      `json:"before_help_long"`
	AfterHelpLong               string      `json:"after_help_long"`
	Examples                    []Example   `json:"examples"`
	Aliases                     []string    `json:"aliases"`
	HiddenAliases               []string    `json:"hidden_aliases"`
	Subcommands                 Subcommands `json:"subcommands"`
	Args                        []Arg       `json:"args"`
	Flags                       []Flag      `json:"flags"`
	UnknownFlags                *string     `json:"unknown_flags"`
	ExternalSubcommand          bool        `json:"external_subcommand"`
	ArgRequiredElseHelp         bool        `json:"arg_required_else_help"`
	DisableHelpFlag             bool        `json:"disable_help_flag"`
	DisableHelpSubcommand       bool        `json:"disable_help_subcommand"`
	DisableVersionFlag          bool        `json:"disable_version_flag"`
	DontDelimitTrailingValues   bool        `json:"dont_delimit_trailing_values"`
	ArgsOverrideSelf            bool        `json:"args_override_self"`
	SubcommandNegatesReqs       bool        `json:"subcommand_negates_reqs"`
	ArgsConflictWithSubcommands bool        `json:"args_conflicts_with_subcommands"`
	SubcommandPrecedenceOverArg bool        `json:"subcommand_precedence_over_arg"`
	AllowMissingPositional      bool        `json:"allow_missing_positional"`
	SubcommandHelpHeading       string      `json:"subcommand_help_heading"`
	SubcommandValueName         string      `json:"subcommand_value_name"`
	SubcommandRequired          bool        `json:"subcommand_required"`
	NextLineHelp                bool        `json:"next_line_help"`
	FlattenHelp                 bool        `json:"flatten_help"`
}

// Subcommands is a command's children, in the order the spec declared them.
//
// A slice rather than the map the JSON object suggests, because the order is
// worth keeping and a Go map does not have one. usage-lib writes its object in
// declaration order — holding that order is something the CLI does deliberately,
// so that a spec generated from a clap program lists its commands the way that
// program does — and `usage generate go` emits its tables in the same order.
// Sorting here instead would give a run-time lowering a different table from the
// generated one built out of the same spec, keys included. They are compared
// against each other, and expected to agree exactly.
type Subcommands []NamedCmd

// NamedCmd is one entry of a subcommand object: the name it was filed under,
// and the command itself.
type NamedCmd struct {
	Name string
	Cmd  Cmd
}

// UnmarshalJSON reads the object a key at a time, which is the only way to see
// the order the keys were written in.
func (s *Subcommands) UnmarshalJSON(data []byte) error {
	// Into a list of its own, assigned at the end. A decoder is allowed to be
	// handed a value that already holds something — decoding twice into the same
	// Spec, or a Spec kept around and refilled — and appending to the receiver
	// would leave the previous spec's commands in the table beside this one's.
	// Assigning only on success is the other half: a spec that failed to decode
	// should not have half-replaced the one that did.
	var out Subcommands

	dec := json.NewDecoder(bytes.NewReader(data))
	tok, err := dec.Token()
	if err != nil {
		return err
	}
	// `null` is a command with no subcommands, and it replaces whatever was there
	// rather than leaving it.
	if tok == nil {
		*s = nil
		return nil
	}
	if delim, ok := tok.(json.Delim); !ok || delim != '{' {
		return fmt.Errorf("subcommands: expected an object, got %v", tok)
	}
	for dec.More() {
		key, err := dec.Token()
		if err != nil {
			return err
		}
		name, ok := key.(string)
		if !ok {
			return fmt.Errorf("subcommands: expected a name, got %v", key)
		}
		var cmd Cmd
		if err := dec.Decode(&cmd); err != nil {
			return err
		}
		out = append(out, NamedCmd{Name: name, Cmd: cmd})
	}
	// The closing brace, so that a truncated object is an error rather than a
	// short list.
	if _, err := dec.Token(); err != nil {
		return err
	}
	*s = out
	return nil
}

// Get is the command filed under a name, if there is one.
func (s Subcommands) Get(name string) (Cmd, bool) {
	for _, sub := range s {
		if sub.Name == name {
			return sub.Cmd, true
		}
	}
	return Cmd{}, false
}

// Flag is one flag in the lowered spec.
type Flag struct {
	Name               string   `json:"name"`
	Long               []string `json:"long"`
	Short              []string `json:"short"`
	HiddenAliases      []string `json:"hidden_aliases"`
	HiddenShortAliases []string `json:"hidden_short_aliases"`
	// Negate arrives with its dashes, as usage-lib stores it. The table wants the
	// bare name.
	Negate string `json:"negate"`
	Global bool   `json:"global"`
	// Var means the flag may be repeated, taking one value each time. It is not
	// the same as a variadic argument, and the parser needs nothing for it: every
	// occurrence is reported separately either way.
	Var bool `json:"var"`
	// Count means each occurrence is tallied rather than replacing the last.
	Count bool `json:"count"`
	// VarMax here bounds occurrences, which is a post-binding check. The bound
	// binding cares about is the one on Arg.
	VarMax int `json:"var_max"`
	// VarMin is a post-binding check too, and for the same reason: no single
	// token can tell you a repeatable flag will end up short.
	VarMin             int      `json:"var_min"`
	Required           bool     `json:"required"`
	Default            []string `json:"default"`
	Env                string   `json:"env"`
	Hide               bool     `json:"hide"`
	HideDefaultValue   bool     `json:"hide_default_value"`
	HideEnv            bool     `json:"hide_env"`
	HideEnvValues      bool     `json:"hide_env_values"`
	HidePossibleValues bool     `json:"hide_possible_values"`
	HideShortHelp      bool     `json:"hide_short_help"`
	HideLongHelp       bool     `json:"hide_long_help"`
	Help               string   `json:"help"`
	HelpFirstLine      string   `json:"help_first_line"`
	HelpLong           string   `json:"help_long"`
	HelpHeading        string   `json:"help_heading"`
	// The four that name another flag. They arrive as written, dashes included.
	Conflicts         []string       `json:"conflicts"`
	Overrides         []string       `json:"overrides"`
	RequiredIf        []string       `json:"required_if"`
	RequiredIfEq      []RequiredIfEq `json:"required_if_eq"`
	RequiredIfEqAll   []RequiredIfEq `json:"required_if_eq_all"`
	RequiredUnless    []string       `json:"required_unless"`
	RequiredUnlessAll []string       `json:"required_unless_all"`
	Requires          []string       `json:"requires"`
	RequiresIf        []RequiresIf   `json:"requires_if"`
	DefaultIf         []DefaultIf    `json:"default_if"`
	RequireEquals     bool           `json:"require_equals"`
	ValueOptional     bool           `json:"value_optional"`
	BoolValue         bool           `json:"bool_value"`
	// Empty means unset: usage-lib stores Option, and a missing default of "" is
	// not carried across the lowering. The corpus never uses one.
	DefaultMissing string `json:"default_missing"`
	Arg            *Arg   `json:"arg"`
	Action         string `json:"action"`
}

type RequiredIfEq struct {
	Selector string `json:"selector"`
	Value    string `json:"value"`
}

// RequiresIf is one explicit value and the flag that value requires.
type RequiresIf struct {
	Value    string `json:"value"`
	Requires string `json:"requires"`
}

// DefaultIf is a default that applies when another flag is given.
type DefaultIf struct {
	Selector string `json:"selector"`
	When     string `json:"when"`
	Value    string `json:"value"`
}

// spelling is how a user types a flag: its first long form, else its first short.
//
// Worked out here, where the forms are visible, because the rules that judge an
// entry never see one — and guessing from the name gets `--a` and `-a` the wrong
// way round.
func spelling(f *Flag) string {
	if len(f.Long) > 0 {
		return "--" + f.Long[0]
	}
	if len(f.Short) > 0 && f.Short[0] != "" {
		return "-" + f.Short[0]
	}
	return ""
}

// choices for a flag are declared on the value it takes, not on the flag.
func (f *Flag) choices() []string {
	if f.Arg == nil {
		return nil
	}
	return f.Arg.Choices.visible()
}

func (f *Flag) acceptedChoices() []string {
	if f.Arg == nil {
		return nil
	}
	return f.Arg.Choices.accepted()
}

// defaults reads through to the value a flag takes, which is the other place a
// default can be written:
//
//	flag "--jobs <n>" {
//	    arg "<n>" default="4"
//	}
//
// usage-lib falls back to it — `lib/src/parse.rs` says so in as many words and a
// live parse confirms it — so a Go CLI generated from the same spec has to as
// well, or the two disagree about what `ex` alone means.
//
// Only the default. A nested `env` is *not* read, because usage-lib does not read
// it either: with `arg "<m>" env="EX_MODE"` inside a flag, `EX_MODE=turbo` leaves
// the flag unset. Following the nesting for one and not the other looks arbitrary
// until you try it, so it is recorded here rather than rediscovered.
func (f *Flag) defaults() []string {
	if len(f.Default) > 0 {
		return f.Default
	}
	if f.Arg != nil {
		return f.Arg.Default
	}
	return nil
}

// Arg is one positional argument, or a flag's value, in the lowered spec.
type Arg struct {
	Name                 string         `json:"name"`
	ValueNames           []string       `json:"value_names"`
	Required             bool           `json:"required"`
	Var                  bool           `json:"var"`
	VarMax               int            `json:"var_max"`
	VarMin               int            `json:"var_min"`
	DoubleDash           string         `json:"double_dash"`
	AllowNegativeNumbers bool           `json:"allow_negative_numbers"`
	ValueTerminator      string         `json:"value_terminator"`
	Delimiter            string         `json:"delimiter"`
	Choices              *Choices       `json:"choices"`
	Default              []string       `json:"default"`
	Env                  string         `json:"env"`
	Hide                 bool           `json:"hide"`
	HideDefaultValue     bool           `json:"hide_default_value"`
	HideEnv              bool           `json:"hide_env"`
	HideEnvValues        bool           `json:"hide_env_values"`
	HidePossibleValues   bool           `json:"hide_possible_values"`
	HideShortHelp        bool           `json:"hide_short_help"`
	HideLongHelp         bool           `json:"hide_long_help"`
	Help                 string         `json:"help"`
	HelpFirstLine        string         `json:"help_first_line"`
	HelpLong             string         `json:"help_long"`
	HelpHeading          string         `json:"help_heading"`
	Validate             string         `json:"validate"`
	ValidateError        string         `json:"validate_error"`
	Conflicts            []string       `json:"conflicts"`
	Requires             []string       `json:"requires"`
	RequiredIf           []string       `json:"required_if"`
	RequiredIfEq         []RequiredIfEq `json:"required_if_eq"`
	RequiredIfEqAll      []RequiredIfEq `json:"required_if_eq_all"`
	RequiredUnless       []string       `json:"required_unless"`
	RequiredUnlessAll    []string       `json:"required_unless_all"`
}

// Example is a worked invocation a page prints.
type Example struct {
	Header string `json:"header"`
	Code   string `json:"code"`
	Help   string `json:"help"`
}

// Choices is the declared set of values, which the lowering nests one level.
type Choices struct {
	Choices    []string `json:"choices"`
	Details    []Choice `json:"details"`
	IgnoreCase bool     `json:"ignore_case"`
}

type Choice struct {
	Value   string        `json:"value"`
	Help    string        `json:"help"`
	Hide    bool          `json:"hide"`
	Aliases []ChoiceAlias `json:"aliases"`
}

type ChoiceAlias struct {
	Value string `json:"value"`
	Hide  bool   `json:"hide"`
}

func (c *Choices) accepted() []string {
	if c == nil {
		return nil
	}
	out := append([]string(nil), c.Choices...)
	for _, detail := range c.Details {
		for _, alias := range detail.Aliases {
			out = append(out, alias.Value)
		}
	}
	return out
}

func (c *Choices) visible() []string {
	if c == nil {
		return nil
	}
	out := make([]string, 0, len(c.Choices))
	for _, value := range c.Choices {
		hidden := false
		for _, detail := range c.Details {
			if detail.Value == value && detail.Hide {
				hidden = true
				break
			}
		}
		if !hidden {
			out = append(out, value)
		}
	}
	for _, detail := range c.Details {
		for _, alias := range detail.Aliases {
			if !alias.Hide {
				out = append(out, alias.Value)
			}
		}
	}
	return out
}

// Multi is how a flag accumulates when it is given more than once.
type Multi uint8

const (
	// MultiNone means a later occurrence replaces an earlier one.
	MultiNone Multi = iota
	// MultiCount means occurrences are tallied.
	MultiCount
	// MultiVar means values are collected into a list.
	MultiVar
)

// Tables builds the parse tables for a spec, discarding the cold half.
func (s *Spec) Tables() *argv.Command {
	root, _ := s.Build()
	return root
}

// Build produces both tables: the hot one binding reads, and the cold one the
// post-binding rules read.
//
// Together, because they share the keys that tie them to each other. Building
// them in separate passes would mean two places assigning identifiers and one
// bug away from a `Meta` describing a different entry than the one it names.
//
// Inheritance is resolved here rather than in the parser: each command's entry
// holds the effective value, which is what a generated table would carry.
func (s *Spec) Build() (*argv.Command, argv.Metadata) {
	root, meta, _ := s.BuildAll()
	return root, meta
}

// BuildAll produces all three tables: the hot one, the post-binding one, and the
// help one. They share the keys that tie them together, so they are built in one
// pass for the same reason the first two were.
func (s *Spec) BuildAll() (*argv.Command, argv.Metadata, argv.HelpTable) {
	b := &builder{complete: s.Complete}
	root := b.command(&s.Cmd, unknownFlags(s.UnknownFlags, argv.UnknownFlagsValue))

	// default_subcommand is a property of the spec rather than of a command, so it
	// is resolved once, here, against the root's own subcommands. A name that
	// answers to nothing is left unset: the spec is what it is, and a vector or a
	// build that expects routing should fail loudly rather than have this guess.
	// Names before aliases, the same precedence a typed word gets: a command's own
	// name outranks another command's alias, so this does not depend on the order
	// the spec declares them in.
	if s.DefaultSubcommand != "" {
		for _, sub := range root.Subcommands {
			if sub.Name == s.DefaultSubcommand {
				root.DefaultSubcommand = sub
				break
			}
		}
		if root.DefaultSubcommand == nil {
			for _, sub := range root.Subcommands {
				if contains(sub.Aliases, s.DefaultSubcommand) {
					root.DefaultSubcommand = sub
					break
				}
			}
		}
	}
	return root, b.meta, b.help
}

// MultiFlags reports which flags accumulate rather than replace, keyed by the
// name the spec gives them.
//
// The parser deliberately does not know this: it reports each occurrence and lets
// the caller decide what to do with it, because whether a second --tag replaces
// the first or joins it is a question about the target type. Generated code
// answers it by assigning to a field or appending to a slice; a harness with no
// target type asks here.
func (s *Spec) MultiFlags() map[string]Multi {
	out := map[string]Multi{}
	collectMulti(&s.Cmd, out)
	return out
}

func collectMulti(c *Cmd, out map[string]Multi) {
	for i := range c.Flags {
		f := &c.Flags[i]
		switch {
		case f.Count:
			out[f.Name] = MultiCount
		case f.Var || (f.Arg != nil && f.Arg.Var):
			out[f.Name] = MultiVar
		}
	}
	for i := range c.Subcommands {
		collectMulti(&c.Subcommands[i].Cmd, out)
	}
}

type builder struct {
	// key hands out identifiers. A Go generator sees the whole spec at once, so it
	// can simply count where the Rust derive has to hash: two macro expansions
	// cannot see each other, and two `go generate` runs over one spec can.
	key uint64
	// meta grows in step with the keys, so entry `Key` lands at `meta[Key-1]` and
	// a lookup is an index.
	meta argv.Metadata
	// help grows the same way, and is separate for the reason the tables are:
	// a caller that renders no pages should not carry the strings.
	help argv.HelpTable
	// scope is the chain above the command being built, so a relationship can
	// name an inherited global. Ancestors only: a command cannot see its own
	// children's flags, and neither can a declaration.
	scope []*argv.Command
	// negation holds each flag's `negate` exactly as the spec wrote it, dashes
	// included. The parse table stores it bare, because that is what the parser
	// has after stripping the `--`; a relationship names a *form*, so comparing
	// it needs the original. A spec declaring `negate="-no-color"` is not named
	// by `--no-color`, and usage-lib does not resolve it either.
	negation map[uint64]string
	// complete is the spec's `complete` blocks, keyed as usage-lib keys them.
	complete map[string]Completer
}

// record files an entry's cold half at the position its key indexes.
func (b *builder) recordNegation(key uint64, raw string) {
	if raw == "" {
		return
	}
	if b.negation == nil {
		b.negation = map[uint64]string{}
	}
	b.negation[key] = raw
}

func (b *builder) record(key uint64, m argv.Meta) {
	m.Key = key
	for uint64(len(b.meta)) < key {
		b.meta = append(b.meta, argv.Meta{})
	}
	b.meta[key-1] = m
}

func (b *builder) recordHelp(key uint64, h argv.Help) {
	h.Key = key
	for uint64(len(b.help)) < key {
		b.help = append(b.help, argv.Help{})
	}
	b.help[key-1] = h
}

// visibleAliases is the aliases a page should advertise: those the spec did not
// also hide.
//
// Nil where none survive, rather than an empty slice, so that a command with no
// visible aliases compares equal to the generated table's zero value.
func visibleAliases(c *Cmd) []string {
	var out []string
	for _, a := range c.Aliases {
		if !contains(c.HiddenAliases, a) {
			out = append(out, a)
		}
	}
	return out
}

// valueOf is what a flag's value is called, or empty where the flag takes none.
func valueOf(f *Flag) string {
	if f.Arg == nil {
		return ""
	}
	return f.Arg.Name
}

// completeType is the type the spec's `complete` block names for an entry.
//
// By lowercased name, which is how usage-lib files them: `complete "FILE"` and an
// argument written `<file>` are the same position as far as the reference is
// concerned.
func (b *builder) completeType(name string) string {
	if b.complete == nil || name == "" {
		return ""
	}
	return b.complete[strings.ToLower(name)].Type
}

func first(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}

func flagAction(action string) argv.ArgAction {
	switch action {
	case "help":
		return argv.ActionHelp
	case "help_short":
		return argv.ActionHelpShort
	case "help_long":
		return argv.ActionHelpLong
	case "version":
		return argv.ActionVersion
	default:
		return argv.ActionSet
	}
}

func (b *builder) next() uint64 {
	b.key++
	return b.key
}

func (b *builder) command(c *Cmd, inherited argv.UnknownFlags) *argv.Command {
	unknown := inherited
	if c.UnknownFlags != nil {
		unknown = unknownFlags(*c.UnknownFlags, inherited)
	}

	out := &argv.Command{
		Name:                        c.Name,
		UnknownFlags:                unknown,
		ExternalSubcommand:          c.ExternalSubcommand,
		ArgRequiredElseHelp:         c.ArgRequiredElseHelp,
		DisableHelpFlag:             c.DisableHelpFlag,
		DisableHelpSubcommand:       c.DisableHelpSubcommand,
		DisableVersionFlag:          c.DisableVersionFlag,
		SubcommandNegatesReqs:       c.SubcommandNegatesReqs,
		ArgsConflictWithSubcommands: c.ArgsConflictWithSubcommands,
		SubcommandPrecedenceOverArg: c.SubcommandPrecedenceOverArg,
		AllowMissingPositional:      c.AllowMissingPositional,
		DontDelimitTrailingValues:   c.DontDelimitTrailingValues,
		Key:                         b.next(),
	}
	examples := make([]argv.Example, 0, len(c.Examples))
	for _, e := range c.Examples {
		examples = append(examples, argv.Example{Header: e.Header, Code: e.Code, Help: e.Help})
	}
	commandHelp := argv.Help{
		Hide:    c.Hide,
		Heading: c.HelpHeading,
		Short:   first(c.Help, c.HelpLong),
		// No fallback to the short text, because the emitter does not write one for
		// a command: the page renderer falls back for itself, so carrying the same
		// string twice would only put it in the table twice. What matters is that
		// the two producers of this table agree — see TestTheTwoProducersAgree.
		Long: c.HelpLong,
		// Visible only: the parse table merges the hidden ones in beside these,
		// because binding does not care which is which. A spec may declare the same
		// alias twice, once hidden, and hiding wins — usage-lib reports it in both
		// lists, so the visible list has to be filtered rather than taken as it is.
		VisibleAliases:        visibleAliases(c),
		BeforeHelp:            c.BeforeHelp,
		AfterHelp:             c.AfterHelp,
		BeforeLongHelp:        c.BeforeHelpLong,
		AfterLongHelp:         c.AfterHelpLong,
		SubcommandHelpHeading: c.SubcommandHelpHeading,
		SubcommandValueName:   c.SubcommandValueName,
		NextLineHelp:          c.NextLineHelp,
		FlattenHelp:           c.FlattenHelp,
		SubcommandRequired:    c.SubcommandRequired,
		Examples:              examples,
	}
	if c.DisplayOrder != nil {
		commandHelp.DisplayOrder = *c.DisplayOrder
		commandHelp.DisplayOrderSet = true
	}
	b.recordHelp(out.Key, commandHelp)

	if n := len(c.Aliases) + len(c.HiddenAliases); n > 0 {
		out.Aliases = make([]string, 0, n)
		out.Aliases = append(out.Aliases, c.Aliases...)
		// A hidden alias selects a command just as a visible one does; hiding is
		// about help output, which binding never reads.
		out.Aliases = append(out.Aliases, c.HiddenAliases...)
	}

	for i := range c.Flags {
		out.Flags = append(out.Flags, b.flag(&c.Flags[i], !c.ArgsOverrideSelf))
	}
	for i := range c.Args {
		out.Args = append(out.Args, b.arg(&c.Args[i]))
	}
	// After the flags, because a relationship names a sibling and every sibling
	// needs a key before any of them can be pointed at.
	b.resolveRelationships(c, out)
	// In scope for everything below, and out of scope again afterwards.
	b.scope = append(b.scope, out)
	for i := range c.Subcommands {
		out.Subcommands = append(out.Subcommands, b.command(&c.Subcommands[i].Cmd, unknown))
	}
	b.scope = b.scope[:len(b.scope)-1]
	return out
}

// resolveRelationships turns the names in `conflicts`, `overrides`,
// `required_if` and `required_unless` into the keys they refer to.
//
// Done here, where the whole command is visible, so that nothing downstream has
// to search by name on a path where it would be repeating the work per parse.
//
// The names arrive as they are written — `--stdin`, dashes and all — so they are
// matched against a flag's long forms, its shorts and the name the spec gives it.
// A name nothing answers to is dropped rather than guessed at; the generator is
// where that should be reported, since it is the one a spec author runs.
func (b *builder) resolveRelationships(c *Cmd, out *argv.Command) {
	find := func(name string) (uint64, bool) {
		// This command's own flags first, then any ancestor's globals — the same
		// scope a token has, and in the same order, so a subcommand redeclaring an
		// inherited name shadows it here exactly as it does at parse time.
		//
		// Searching only locally was a silent hole: `conflicts="--quiet"` on a
		// subcommand flag, where `--quiet` is a root global, resolved to nothing
		// and the rule was simply never enforced. usage-lib enforces it.
		if key, ok := b.matchFlag(out.Flags, name, false); ok {
			return key, true
		}
		if !strings.HasPrefix(name, "-") {
			for _, arg := range out.Args {
				if arg.Name == name {
					return arg.Key, true
				}
			}
		}
		for i := len(b.scope) - 1; i >= 0; i-- {
			if key, ok := b.matchFlag(b.scope[i].Flags, name, true); ok {
				return key, true
			}
		}
		return 0, false
	}
	resolve := func(names []string) []uint64 {
		var out []uint64
		for _, name := range names {
			if key, ok := find(name); ok {
				out = append(out, key)
			}
		}
		return out
	}
	resolveValues := func(requirements []RequiresIf) []argv.ValueRequirement {
		var out []argv.ValueRequirement
		for _, requirement := range requirements {
			if key, ok := find(requirement.Requires); ok {
				out = append(out, argv.ValueRequirement{Value: requirement.Value, Key: key})
			}
		}
		return out
	}
	resolveConditions := func(conditions []RequiredIfEq) []argv.ValueCondition {
		var out []argv.ValueCondition
		for _, condition := range conditions {
			if key, ok := find(condition.Selector); ok {
				out = append(out, argv.ValueCondition{Key: key, Value: condition.Value})
			}
		}
		return out
	}
	resolveDefaultIf := func(conditions []DefaultIf) []argv.DefaultIf {
		var out []argv.DefaultIf
		for _, condition := range conditions {
			if key, ok := find(condition.Selector); ok {
				out = append(out, argv.DefaultIf{Key: key, When: condition.When, Value: condition.Value})
			}
		}
		return out
	}

	// `c.Flags` and `out.Flags` are built in step, so the index is the join.
	for i := range c.Flags {
		src := &c.Flags[i]
		m := &b.meta[out.Flags[i].Key-1]
		m.Conflicts = resolve(src.Conflicts)
		m.Overrides = resolve(src.Overrides)
		m.RequiredUnless = resolve(src.RequiredUnless)
		m.RequiredUnlessAll = resolve(src.RequiredUnlessAll)
		m.RequiredIf = resolve(src.RequiredIf)
		m.RequiredIfEq = resolveConditions(src.RequiredIfEq)
		m.RequiredIfEqAll = resolveConditions(src.RequiredIfEqAll)
		m.Requires = resolve(src.Requires)
		m.RequiresIf = resolveValues(src.RequiresIf)
		m.DefaultIf = resolveDefaultIf(src.DefaultIf)
	}
	for i := range c.Args {
		src := &c.Args[i]
		m := &b.meta[out.Args[i].Key-1]
		m.Conflicts = resolve(src.Conflicts)
		m.Requires = resolve(src.Requires)
		m.RequiredIf = resolve(src.RequiredIf)
		m.RequiredIfEq = resolveConditions(src.RequiredIfEq)
		m.RequiredIfEqAll = resolveConditions(src.RequiredIfEqAll)
		m.RequiredUnless = resolve(src.RequiredUnless)
		m.RequiredUnlessAll = resolve(src.RequiredUnlessAll)
	}
}

// matchFlag finds a flag by any spelling a declaration may use for it.
//
// The negation counts, and resolves to the same entry: usage-lib treats
// `conflicts="--no-color"` as naming the `color` flag, and reports the conflict
// whichever of the two spellings was typed. The relationship is between entries
// rather than between tokens, which is what this key model already assumes.
func (b *builder) matchFlag(flags []*argv.Flag, name string, globalsOnly bool) (uint64, bool) {
	// Two passes, in the order the parser itself looks: every ordinary form
	// first, then negations.
	//
	// That order is not a nicety. `longFlag` tries `findLong` across all flags
	// before it tries `findNegation`, so with `--a` declaring `negate="--zap"`
	// and a separate `--zap`, typing `--zap` binds *zap*. A per-candidate search
	// would hand the relationship to `a`, and the table would then enforce a rule
	// against a flag the command line never binds. The table has to agree with
	// the binder it feeds.
	eligible := func(f *argv.Flag) bool { return !globalsOnly || f.Global }

	// The form is part of the name: `--q` does not reach the short `-q`, and
	// `-color` does not reach the long `--color`. usage-lib resolves neither.
	long, short, bare := "", byte(0), ""
	switch {
	case strings.HasPrefix(name, "--"):
		long = name[2:]
	case strings.HasPrefix(name, "-") && len(name) == 2:
		short = name[1]
	case !strings.HasPrefix(name, "-"):
		// Undashed, which is the name the spec gives the flag rather than a form
		// it can be typed as.
		bare = name
	}

	for _, f := range flags {
		if !eligible(f) {
			continue
		}
		if bare != "" && f.Name == bare {
			return f.Key, true
		}
		if long != "" {
			for _, l := range f.Longs {
				if l == long {
					return f.Key, true
				}
			}
		}
		if short != 0 {
			for _, s := range f.Shorts {
				if s == short {
					return f.Key, true
				}
			}
		}
	}

	// Negations, compared exactly as both sides were written — dashes included.
	// `negate="-no-tint"` is named by `-no-tint` and not by `--no-tint`, and
	// usage-lib resolves it that way round too.
	for _, f := range flags {
		if eligible(f) && b.negation[f.Key] != "" && b.negation[f.Key] == name {
			return f.Key, true
		}
	}
	return 0, false
}
func (b *builder) flag(f *Flag, strictDuplicates bool) *argv.Flag {
	out := &argv.Flag{
		Key:         b.next(),
		Name:        f.Name,
		Longs:       f.Long,
		HiddenLongs: f.HiddenAliases,
		Negate:      strings.TrimLeft(f.Negate, "-"),
		TakesValue:  f.Arg != nil,
		// Stored on the nested arg as double_dash=automatic, the same place
		// usage-lib keeps allow_hyphen_values. Trailing positionals use Arg.DoubleDash
		// on the command, not here.
		AllowHyphenValues:    f.Arg != nil && strings.EqualFold(f.Arg.DoubleDash, "automatic"),
		AllowNegativeNumbers: f.Arg != nil && f.Arg.AllowNegativeNumbers,
		RequireEquals:        f.RequireEquals,
		ValueOptional:        f.ValueOptional,
		BoolValue:            f.BoolValue,
		DefaultMissing:       f.DefaultMissing,
		Global:               f.Global,
		Action:               flagAction(f.Action),
	}
	if f.Arg != nil && len(f.Arg.Delimiter) == 1 {
		out.Delimiter = f.Arg.Delimiter[0]
	}
	b.recordNegation(out.Key, f.Negate)
	for _, s := range f.Short {
		if s != "" {
			// One byte: a cluster is walked a byte at a time, so a multi-byte short
			// could never be matched anyway.
			out.Shorts = append(out.Shorts, s[0])
		}
	}
	for _, s := range f.HiddenShortAliases {
		if s != "" {
			out.HiddenShorts = append(out.HiddenShorts, s[0])
		}
	}
	valueName, valueDemanded := "", false
	if f.Arg != nil {
		if f.Arg.Name != f.Name {
			valueName = f.Arg.Name
		}
		valueDemanded = f.Arg.Required && len(f.Arg.Default) == 0
	}
	b.recordHelp(out.Key, argv.Help{
		Hide:               f.Hide,
		HideDefaultValue:   f.HideDefaultValue,
		HideEnv:            f.HideEnv,
		HideEnvValues:      f.HideEnvValues,
		HidePossibleValues: f.HidePossibleValues,
		HideShortHelp:      f.HideShortHelp,
		HideLongHelp:       f.HideLongHelp,
		// Required *and* undefaulted: a required flag with a default is one the
		// user never has to type, so the line brackets it.
		Demanded:      f.Required && len(f.Default) == 0,
		Repeatable:    f.Var,
		ValueName:     valueName,
		ValueNames:    valueNames(f.Arg),
		ValueArity:    exactArity(f.Arg),
		ValueDemanded: valueDemanded,
		Short:         first(f.Help, f.HelpFirstLine),
		Long:          first(f.HelpLong, f.Help),
		Heading:       f.HelpHeading,
		Choices:       f.choices(),
		Env:           f.Env,
		Default:       f.defaults(),
	})
	b.record(out.Key, argv.Meta{
		Name:              f.Name,
		Flag:              true,
		RequiresIfBoolean: len(f.RequiresIf) > 0 && f.Arg == nil,
		Spelling:          spelling(f),
		ValueName:         valueOf(f),
		CompleteType:      b.completeType(first(valueOf(f), f.Name)),
		Required:          f.Required,
		RejectDuplicate:   strictDuplicates && !f.Var && !f.Count && (f.Arg == nil || !f.Arg.Var),
		Choices:           f.choices(),
		AcceptedChoices:   f.acceptedChoices(),
		IgnoreCase:        f.Arg != nil && f.Arg.Choices != nil && f.Arg.Choices.IgnoreCase,
		Default:           f.defaults(),
		Env:               f.Env,
		VarMin:            clampVarMax(f.valueMinimum()),
		Validate:          valueValidation(f.Arg),
		ValidateError:     valueValidationError(f.Arg),
		// Occurrences. The per-occurrence value bound is a limit binding applies,
		// and is set on the parse table below rather than here.
		VarMax: clampVarMax(f.VarMax),
	})

	if f.Arg != nil && f.Arg.Var {
		// Only a variadic argument is greedy. A var flag with a single-value argument
		// is repeatable instead: one value per occurrence, which the parser gets by
		// not collecting.
		out.Variadic = true
		// The bound on one occurrence's values, which is the argument's. A repeatable
		// flag's own var_max counts occurrences and is checked after the parse, so it
		// does not belong in this table.
		out.VarMax = clampVarMax(f.Arg.VarMax)
		out.ValueTerminator = f.Arg.ValueTerminator
	}
	return out
}

// valueMinimum is the post-binding minimum for this flag. A variadic value has
// one occurrence, so its nested minimum is also its total minimum; an ordinary
// repeatable flag instead uses the flag-level occurrence minimum.
func (f *Flag) valueMinimum() int {
	if f.Arg != nil && f.Arg.Var {
		if f.Arg.VarMin != 0 {
			return f.Arg.VarMin
		}
		return f.VarMin
	}
	return f.VarMin
}

func (b *builder) arg(a *Arg) *argv.Arg {
	out := &argv.Arg{
		Key:                  b.next(),
		Name:                 a.Name,
		Required:             a.Required,
		Var:                  a.Var,
		DoubleDash:           doubleDash(a.DoubleDash),
		AllowNegativeNumbers: a.AllowNegativeNumbers,
		ValueTerminator:      a.ValueTerminator,
	}
	if len(a.Delimiter) == 1 {
		out.Delimiter = a.Delimiter[0]
	}
	if a.Var {
		out.VarMax = clampVarMax(a.VarMax)
	}
	b.recordHelp(out.Key, argv.Help{
		Hide:               a.Hide,
		HideDefaultValue:   a.HideDefaultValue,
		HideEnv:            a.HideEnv,
		HideEnvValues:      a.HideEnvValues,
		HidePossibleValues: a.HidePossibleValues,
		HideShortHelp:      a.HideShortHelp,
		HideLongHelp:       a.HideLongHelp,
		Demanded:           a.Required && len(a.Default) == 0,
		ValueNames:         a.ValueNames,
		ValueArity:         exactArity(a),
		Short:              first(a.Help, a.HelpFirstLine),
		Long:               first(a.HelpLong, a.Help),
		Heading:            a.HelpHeading,
		Choices:            a.Choices.visible(),
		Env:                a.Env,
		Default:            a.Default,
	})
	b.record(out.Key, argv.Meta{
		Name:            a.Name,
		Required:        a.Required,
		CompleteType:    b.completeType(a.Name),
		Choices:         a.Choices.visible(),
		AcceptedChoices: a.Choices.accepted(),
		IgnoreCase:      a.Choices != nil && a.Choices.IgnoreCase,
		Default:         a.Default,
		Env:             a.Env,
		VarMin:          clampVarMax(a.VarMin),
		Validate:        a.Validate,
		ValidateError:   a.ValidateError,
		// No VarMax: for an argument the bound is a limit binding applies, which
		// is what makes `[a]… [b]` fillable at all, so judging it again here would
		// fail an invocation that never broke it.
	})
	return out
}

func valueNames(a *Arg) []string {
	if a == nil || len(a.ValueNames) == 0 {
		return nil
	}
	return a.ValueNames
}

func exactArity(a *Arg) uint32 {
	if a != nil && a.Var && a.VarMin > 1 && a.VarMin == a.VarMax {
		return uint32(a.VarMin)
	}
	return 0
}

func valueValidation(arg *Arg) string {
	if arg == nil {
		return ""
	}
	return arg.Validate
}

func valueValidationError(arg *Arg) string {
	if arg == nil {
		return ""
	}
	return arg.ValidateError
}

// clampVarMax turns the spec's bound into the table's.
//
// Zero means unbounded in the table, which is also what an absent var_max lowers
// to, so the two agree. A bound larger than a uint32 saturates rather than
// wrapping: truncating four billion and one to one would read as "stop at once"
// rather than "no real limit".
func clampVarMax(n int) uint32 {
	switch {
	case n <= 0:
		return 0
	case n > int(^uint32(0)):
		return ^uint32(0)
	}
	return uint32(n)
}

func unknownFlags(s string, fallback argv.UnknownFlags) argv.UnknownFlags {
	switch strings.ToLower(s) {
	case "error":
		return argv.UnknownFlagsError
	case "value":
		return argv.UnknownFlagsValue
	}
	return fallback
}

func doubleDash(s string) argv.DoubleDash {
	switch strings.ToLower(s) {
	case "required":
		return argv.DoubleDashRequired
	case "preserve":
		return argv.DoubleDashPreserve
	case "automatic":
		return argv.DoubleDashAutomatic
	}
	return argv.DoubleDashOptional
}

func contains(list []string, s string) bool {
	for _, x := range list {
		if x == s {
			return true
		}
	}
	return false
}
