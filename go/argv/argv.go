// Package argv binds a command line against static tables.
//
// It is the Go half of what [usage-argv] is in Rust: the same grammar, proved
// against the same [conformance corpus], with the same division of labour. The
// parser answers one question — which token becomes which flag or argument — and
// leaves everything that needs a value's type to the layer above it.
//
// # Why tables rather than a command tree
//
// Every Go CLI framework builds a model of the CLI at run time. cobra constructs
// a [cobra.Command] per subcommand with a flag set each; kong walks a struct with
// reflection. Both pay for the whole CLI on every invocation, including the 200
// commands the user did not type. At mise's size that is 2M instructions for
// cobra and 58M for kong before the first token is read.
//
// The tables here are package-level data with no pointers to build, so the Go
// linker lays them out and nothing runs before main. Binding a mise-sized command
// line then costs about 2,700 instructions, which is below the run-to-run
// variance of the Go runtime's own startup — the parse disappears into the noise
// rather than showing up in it.
//
// Tables are meant to be generated from a usage spec. Writing them by hand is
// supported and is what the tests do, but a real CLI declares its spec and gets
// these emitted.
//
// [usage-argv]: https://github.com/jdx/usage/tree/main/argv
// [conformance corpus]: https://github.com/jdx/usage/tree/main/corpus
package argv

// MaxDepth is how deep a command tree the parser will descend.
//
// The ancestor chain lives in a fixed-size array so that a parse allocates
// nothing; this is that array's size. mise, the largest usage CLI, is four levels
// deep.
const MaxDepth = 16

// Command is a command: its flags, its positional arguments, and its
// subcommands.
//
// Every field is plain data or a slice of pointers to plain data, which is what
// lets a generated table be package-level `var` the linker initializes. Check
// with `go tool nm`: the symbols should be type D, and the package should have no
// init function.
type Command struct {
	// Name is the canonical name, used to select this command.
	Name string
	// Aliases are alternative names that also select it.
	Aliases []string
	Flags   []*Flag
	// Args are positional arguments, in the order they are filled.
	Args        []*Arg
	Subcommands []*Command
	// DefaultSubcommand is where a word goes when it names no subcommand of this
	// one.
	//
	// The spec's default_subcommand. `mise build` means `mise run build`: the word
	// names no command, so the parser descends into `run` and lets run have it —
	// even where this command declares an argument of its own, which is what makes
	// the property worth having rather than a synonym for a positional.
	//
	// Applied at most once per parse, so a CLI cannot loop through it, and only
	// where a subcommand could still be selected.
	DefaultSubcommand *Command
	// ExternalSubcommand is whether an unmatched word is forwarded as an external
	// command plus the rest of argv.
	//
	// clap's allow_external_subcommands. Known subcommands still win; a
	// DefaultSubcommand still catches first. Once the unmatched word is taken,
	// remaining tokens — including --help — are not parsed as this command's flags.
	ExternalSubcommand bool
	// ArgRequiredElseHelp shows this command's help when no argv token follows its name.
	ArgRequiredElseHelp bool
	// SubcommandNegatesReqs lets a selected child satisfy this command's requirements.
	SubcommandNegatesReqs bool
	// ArgsConflictWithSubcommands rejects selecting a child after this command
	// has already bound a flag or positional.
	ArgsConflictWithSubcommands bool
	// SubcommandPrecedenceOverArg lets a known child interrupt a variadic value owner.
	SubcommandPrecedenceOverArg bool
	// AllowMissingPositional lets later required positionals claim remaining words
	// while earlier optional positionals stay empty.
	AllowMissingPositional bool
	// DontDelimitTrailingValues disables delimiter splitting after -- and for
	// automatic trailing arguments. It is inherited by subcommands.
	DontDelimitTrailingValues bool
	// UnknownFlags is what an unrecognized flag-like token means here. Already
	// resolved: inheritance is a question for whoever builds the tables.
	UnknownFlags UnknownFlags
	// Version is whether this command answers to --version and -V.
	//
	// Set on the root, and only when the CLI declares a version: a --version that
	// answers with nothing is worse than one that is not there.
	Version bool
	// Key is a caller-assigned identifier, echoed back in the event.
	//
	// Generated code dispatches on this instead of comparing strings. Rust needs
	// 64 bits here because two macro expansions cannot see each other and must
	// hash their way to uniqueness; a Go generator sees the whole spec at once and
	// can simply count, but the width costs nothing and keeps the two tables
	// interchangeable.
	Key uint64
}

// Flag is a flag, addressed by any of its long or short forms.
type Flag struct {
	// Key is a caller-assigned identifier, echoed back in the event. This is how
	// generated code knows which field to assign without any string comparison.
	Key uint64
	// Name is unused by binding, kept so a table entry can carry its own name for
	// diagnostics.
	Name string
	// Longs are long forms, written without the leading --.
	Longs []string
	// HiddenLongs are accepted long aliases omitted from help and completion.
	// Every entry also appears in Longs so parsing remains table-driven.
	HiddenLongs []string
	// Shorts are short forms, as single bytes.
	//
	// Should be ASCII. A cluster like -xyz is walked one byte at a time, so a
	// non-ASCII short can never be matched, and the remainder after a value-taking
	// one — which becomes its value — would begin in the middle of a character.
	Shorts []byte
	// HiddenShorts are accepted short aliases omitted from help and completion.
	// Every entry also appears in Shorts.
	HiddenShorts []byte
	// Negate is a long form that sets the flag to false, written without the --.
	// Empty means the flag has none.
	Negate string
	// TakesValue is whether the flag takes a value.
	TakesValue bool
	// Variadic is whether one occurrence of this flag keeps taking values, until a
	// flag-like token or the end of the command line.
	//
	// This is the spec's variadic flag argument (--include <pattern>...). It is
	// not the spec's flag-level var=#true, which means the flag may be repeated
	// and takes one value each time — repetition needs nothing from the parser,
	// since it already reports every occurrence separately. Conflating the two
	// makes a merely repeatable flag greedy enough to eat a positional.
	Variadic bool
	// VarMax is how many values one variadic occurrence may take, after which the
	// next word belongs to whatever comes next. Zero means unbounded.
	//
	// Only for Variadic. A merely repeatable flag is bounded on how many times it
	// was given, which no single token can decide, so that bound stays with the
	// metadata and is checked after the parse.
	VarMax uint32
	// AllowHyphenValues is whether a detached value may itself look like a flag.
	//
	// The default is to refuse: `--jobs --force` is far more likely a forgotten
	// value than a jobs of `"--force"`. Declared, the next token is taken
	// whatever it looks like — including `--` — which is clap's
	// allow_hyphen_values and the spec's property of the same name. A variadic
	// occurrence still stops collecting at a later flag-like token, so a second
	// occurrence of the flag is not eaten as a value.
	AllowHyphenValues bool
	// AllowNegativeNumbers accepts negative numeric tokens as detached values
	// without accepting arbitrary dash-prefixed words.
	AllowNegativeNumbers bool
	// ValueTerminator ends one variadic occurrence without becoming a value.
	ValueTerminator string
	// Delimiter splits one token into several values. Zero disables splitting.
	Delimiter byte
	// RequireEquals is whether the value must be attached with `=`.
	//
	// `--flag=value` is accepted and `--flag value` is not, which is clap's
	// require_equals and the spec's property of the same name. A short's
	// attached form (`-i9229`, `-i=9229`) still binds: only the following word
	// is refused.
	RequireEquals bool
	// DefaultMissing is the value used when the flag is present but no value is
	// given. Empty means unset: the flag then errors if a value is missing.
	//
	// clap's default_missing_value and the spec's default_missing. `--color`
	// binds this, `--color=never` binds `never`, and an absent flag is not bound.
	// Combined with RequireEquals, a following word is still refused.
	DefaultMissing string
	// Global is whether the flag is recognized by every command beneath the one
	// that declares it.
	Global bool
}

// Arg is a positional argument.
type Arg struct {
	// Key is a caller-assigned identifier, echoed back in the event.
	Key uint64
	// Required is used while binding when AllowMissingPositional reserves words
	// for later required positionals.
	Required bool
	// Name is unused by binding, kept so a table entry can carry its own name for
	// diagnostics.
	Name string
	// Var is whether this argument keeps taking values once it has one.
	Var bool
	// VarMax is how many words a variadic may take before the next argument gets
	// the rest. Zero means unbounded.
	//
	// A bound belongs here, in the table binding reads, rather than with the
	// metadata: it decides where a word lands, not whether what landed is
	// acceptable. clap's num_args works the same way, and specs are commonly
	// generated from clap commands.
	VarMax uint32
	// AllowNegativeNumbers accepts negative numeric tokens in strict flag mode.
	AllowNegativeNumbers bool
	// ValueTerminator ends this variadic positional without becoming a value.
	ValueTerminator string
	// Delimiter splits one token into several values. Zero disables splitting.
	Delimiter byte
	// DoubleDash is this argument's relationship to the -- separator.
	DoubleDash DoubleDash
}

// UnknownFlags is what to do with a flag-like token that names no flag in scope.
//
// The default is UnknownFlagsValue: the token carries on to the positional
// arguments, because a spec is often parsing a command line whose flags belong to
// something else — a wrapped tool, a task script. A CLI that owns all of its flags
// declares UnknownFlagsError and gets typo detection instead.
//
// Stored per command and already resolved: inheritance is a question for whoever
// builds the tables, and answering it at generation time keeps it out of the
// parse.
type UnknownFlags uint8

const (
	// UnknownFlagsValue offers the token to the positionals. If none can take it,
	// it is an unexpected argument.
	UnknownFlagsValue UnknownFlags = iota
	// UnknownFlagsError rejects the token.
	UnknownFlagsError
)

// DoubleDash is how an argument relates to the -- separator.
type DoubleDash uint8

const (
	// DoubleDashOptional lets values appear on either side of a --.
	DoubleDashOptional DoubleDash = iota
	// DoubleDashRequired accepts values only after a --.
	DoubleDashRequired
	// DoubleDashPreserve keeps a -- as a value rather than consuming it as a
	// separator.
	DoubleDashPreserve
	// DoubleDashAutomatic behaves as if a -- had been given once the argument
	// takes a value, so the rest of the command line is values. A wrapper can then
	// forward flags without its caller typing the separator.
	DoubleDashAutomatic
)

// Kind is which of the things an [Event] reports.
type Kind uint8

const (
	// KindCommand means a subcommand was selected; parsing continues inside it.
	KindCommand Kind = iota
	// KindFlag means a flag was given.
	KindFlag
	// KindArg means a word was bound to a positional argument.
	KindArg
	// KindExternal means an unmatched word was forwarded as an external command:
	// the name, then every remaining token, including flags.
	KindExternal
)

// Event is something the parser bound.
//
// One struct with a Kind rather than three types, so that an event is returned by
// value and costs nothing: a Go interface here would box every binding.
//
// Values are strings that share memory with the argv the parser was given.
// Slicing a Go string does not copy, so a value costs no allocation — and it is
// the raw bytes the operating system supplied, which on Unix need not be valid
// UTF-8. Convert where you build the target type, not here.
type Event struct {
	Kind Kind
	// Command is set when Kind is KindCommand.
	Command *Command
	// Flag is set when Kind is KindFlag.
	Flag *Flag
	// Arg is set when Kind is KindArg.
	Arg *Arg
	// Value is the bound value. Meaningful for KindArg always, and for KindFlag
	// when HasValue is set.
	Value string
	// HasValue distinguishes a flag that took a value from one that did not, and
	// a value-taking flag given the empty string (--jobs=) from a boolean one.
	HasValue bool
	// Negated is true when a flag was set through its Negate form.
	Negated bool
	// Delimit says whether a positional value should honor Arg.Delimiter.
	Delimit bool
	// Values is the remaining argv when Kind is KindExternal: the unmatched name
	// first, then every token after it. Shares memory with the argv the parser
	// was given.
	Values []string
}

// SplitValue applies a generated table's ASCII delimiter to one bound value.
// Delimit is false for trailing positional values protected by
// DontDelimitTrailingValues.
func SplitValue(value string, delimiter byte, delimit bool) []string {
	if delimiter == 0 || !delimit {
		return []string{value}
	}
	out := make([]string, 0, valuesIn(value, delimiter))
	start := 0
	for i := 0; i < len(value); i++ {
		if value[i] == delimiter {
			out = append(out, value[start:i])
			start = i + 1
		}
	}
	return append(out, value[start:])
}

// Code is a class of binding failure.
//
// The grammar specifies the class, not the wording: diagnostics are a
// quality-of-implementation concern, but a strict parser and a lenient one must
// be tellable apart mechanically. These are the codes the conformance corpus
// uses.
type Code uint8

const (
	// CodeUnknownFlag means a flag-like token matched no flag in scope.
	CodeUnknownFlag Code = iota
	// CodeMissingFlagValue means a flag needing a value did not get one.
	CodeMissingFlagValue
	// CodeUnexpectedArg means a word arrived with no argument left to hold it.
	CodeUnexpectedArg
	// CodeSubcommandConflict means an argument was bound before a subcommand.
	CodeSubcommandConflict
	// CodeArgRequiresDoubleDash means a double_dash="required" argument was
	// offered a word before any -- had been seen.
	CodeArgRequiresDoubleDash
	// CodeTooDeep means the command tree is deeper than MaxDepth.
	CodeTooDeep
	// CodeHelp means --help or -h was given. Not a failure; see [Error].
	CodeHelp
	// CodeVersion means --version or -V was given. Not a failure either.
	CodeVersion

	// The rest are raised after the parse, by [Check], because they need to know
	// a value's declared type or its final count. They share this type so a caller
	// has one error to handle rather than two.

	// CodeMissingRequiredFlag means a required flag never appeared.
	CodeMissingRequiredFlag
	// CodeMissingRequiredArg means a required argument was never filled.
	CodeMissingRequiredArg
	// CodeInvalidChoice means a value was given that is not among the declared
	// choices.
	CodeInvalidChoice
	// CodeVarTooFew means fewer values than var_min.
	CodeVarTooFew
	// CodeVarTooMany means more occurrences than a repeatable flag's var_max.
	CodeVarTooMany
	// CodeConflictingFlags means two flags declared to conflict were both given.
	CodeConflictingFlags
	// CodeInvalidValue means a value was given that the target type could not be
	// built from.
	CodeInvalidValue
	// CodeDuplicateFlag means a strict command received a single-valued flag
	// more than once.
	CodeDuplicateFlag
)

var codeNames = [...]string{
	CodeUnknownFlag:           "unknown_flag",
	CodeMissingFlagValue:      "missing_flag_value",
	CodeUnexpectedArg:         "unexpected_arg",
	CodeSubcommandConflict:    "subcommand_conflict",
	CodeArgRequiresDoubleDash: "arg_requires_double_dash",
	CodeTooDeep:               "too_deep",
	CodeHelp:                  "help",
	CodeVersion:               "version",
	CodeMissingRequiredFlag:   "missing_required_flag",
	CodeMissingRequiredArg:    "missing_required_arg",
	CodeInvalidChoice:         "invalid_choice",
	CodeVarTooFew:             "var_too_few",
	CodeVarTooMany:            "var_too_many",
	CodeConflictingFlags:      "conflicting_flags",
	CodeInvalidValue:          "invalid_value",
	CodeDuplicateFlag:         "duplicate_flag",
}

// String gives the code the corpus spells it with.
func (c Code) String() string {
	if int(c) < len(codeNames) {
		return codeNames[c]
	}
	return "unknown"
}

// Error is a binding failure.
//
// It carries the offending token so a caller can render a good message, but no
// message of its own beyond the code: rendering belongs to a cold path, and
// building a string here would allocate on the way to reporting that nothing was
// allocated.
//
// Help and Version ride in this type without being failures, as clap and
// usage-argv both have them: a parse that stops to print help has not produced a
// value, and every caller already handles the "no value" shape.
//
// The parser holds one of these inline and hands back a pointer to it, so a
// failing parse allocates nothing either.
type Error struct {
	Code Code
	// Token is the whole token as typed, for CodeUnknownFlag and
	// CodeUnexpectedArg. A bundle containing an unrecognized letter reports -fz
	// rather than the letter alone, which is also the unit in which it is
	// rejected.
	Token string
	// Flag is set for CodeMissingFlagValue.
	Flag *Flag
	// Arg is set for CodeArgRequiresDoubleDash.
	Arg *Arg
	// Cmd is what CodeHelp was asked about.
	Cmd *Command
	// Long distinguishes --help from -h, which print different amounts.
	Long bool

	// Name is the flag or argument the post-binding rules rejected, as the spec
	// spells it.
	Name string
	// Spelling is how the entry is typed, where the rule that raised this knew —
	// see [Meta.Spelling]. Empty means only the name is known.
	Spelling string
	// OtherSpelling is the same for [Error.Other].
	OtherSpelling string
	// Choices carries the declared list for CodeInvalidChoice, rather than the
	// offending value: the value is the caller's to render, and it has it.
	Choices []string
	// Bound and Got are the declared limit and what was actually counted, for the
	// two var codes.
	Bound uint32
	Got   int
	// Value is the text that would not convert, and Want the type it was being
	// converted to, for CodeInvalidValue. The text is carried because the whole
	// point of the error is to show it back.
	Value string
	Want  string
	// Reason explains a declarative validation failure. Empty for ordinary typed
	// conversion failures, which use Want instead.
	Reason string
	// Other is the flag [Name] cannot be given with, for CodeConflictingFlags.
	// Both are carried because either alone reads as a puzzle: which flag is
	// unwelcome depends on what else was given.
	Other string
}

// Error is the message Go's own error interface asks for.
//
// The tokens go through `safe` here as they do in [Render]: this string reaches a
// terminal too, by way of whatever logs or prints it, and a rejected argument
// carrying an escape sequence can recolour that output or forge a line in it.
// Where the message quotes the spec — a flag's name, an argument's — there is
// nothing to escape, because the author wrote it and the parse tables hold it.
func (e *Error) Error() string {
	switch e.Code {
	case CodeUnknownFlag:
		return "unknown flag: " + safe(e.Token)
	case CodeMissingFlagValue:
		return "missing value for flag: " + e.Flag.Name
	case CodeUnexpectedArg:
		return "unexpected argument: " + safe(e.Token)
	case CodeSubcommandConflict:
		return "subcommand conflicts with an earlier argument: " + safe(e.Token)
	case CodeArgRequiresDoubleDash:
		return "argument requires a -- separator: " + e.Arg.Name
	case CodeTooDeep:
		return "command tree deeper than MaxDepth"
	case CodeHelp:
		return "help requested"
	case CodeVersion:
		return "version requested"
	case CodeMissingRequiredFlag, CodeMissingRequiredArg:
		return "missing required: " + e.Name
	case CodeInvalidChoice:
		return "invalid value for " + e.Name
	case CodeVarTooFew:
		return "too few values for " + e.Name
	case CodeVarTooMany:
		return "too many occurrences of " + e.Name
	case CodeConflictingFlags:
		return e.Name + " cannot be given with " + e.Other
	case CodeInvalidValue:
		// Through `safe` for the same reason the tokens are: the rejected text came
		// off the command line, and this one is likelier than most to hold
		// something strange — it exists because the text was not what the type
		// expected.
		message := "invalid value for " + e.Name + ": " + safe(e.Value)
		if e.Reason != "" {
			message += ": " + e.Reason
		}
		return message
	}
	return "parse error"
}

// The two flags every CLI answers to without declaring them. Package-level so
// that an event can point at one without allocating, and so that a caller can
// compare a reported flag against them by identity.
var (
	// HelpLong is the synthetic --help.
	HelpLong = &Flag{Key: ^uint64(0), Name: "help", Longs: []string{"help"}}
	// HelpShort is the synthetic -h.
	HelpShort = &Flag{Key: ^uint64(0) - 1, Name: "help", Shorts: []byte{'h'}}
	// VersionLong is the synthetic --version, offered only where the command says
	// Version.
	VersionLong = &Flag{Key: ^uint64(0) - 2, Name: "version", Longs: []string{"version"}}
	// VersionShort is the synthetic -V.
	VersionShort = &Flag{Key: ^uint64(0) - 3, Name: "version", Shorts: []byte{'V'}}
)

// IsHelpFlag reports whether a flag is one the parser supplied for --help or -h.
func IsHelpFlag(f *Flag) bool { return f == HelpLong || f == HelpShort }

// IsVersionFlag reports whether a flag is one the parser supplied for --version
// or -V.
func IsVersionFlag(f *Flag) bool { return f == VersionLong || f == VersionShort }
