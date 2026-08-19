# `group`

A set of flags that relate to one another as a set.

```kdl
flag "--file <file>"
flag "--url <url>"
flag "--stdin"

// at most one of these
group "input" "--file" "--url" "--stdin"

// exactly one of these: one is required, and only one is allowed
group "input" "--file" "--url" "--stdin" required=#true

// at least one of these
group "input" "--file" "--url" "--stdin" required=#true multiple=#true

// the same, written out when the members do not sit comfortably on one line
group "input" required=#true {
  flag "--file" "--url"
  flag "--stdin"
}
```

The name comes first, then the members, spelled the way every other relationship names
a flag — `--long` or `-s`. A group lives on the command whose flags it names, and a
group naming [global flags](/spec/reference/flag#global) belongs to the command that
declares them.

## What the two properties mean

| `required` | `multiple` | meaning               |
| ---------- | ---------- | --------------------- |
| —          | —          | at most one of these  |
| `#true`    | —          | exactly one of these  |
| —          | `#true`    | nothing is enforced   |
| `#true`    | `#true`    | at least one of these |

Both default to false, so a bare group is mutual exclusion. These are clap's two
properties read the same way, so a spec generated from a clap command means by them what
clap meant.

## Why not just `conflicts`

For "at most one", [`conflicts`](/spec/reference/flag#conflicts-and-overrides) says the
same thing — but it says it once per pair, so three flags need three declarations and six
need fifteen, and a flag added later has to be added to every sibling.

The part `conflicts` cannot say at all is `required`. "One of these is needed" is a
statement about the set: no rule written on an individual flag expresses it, because no
individual flag is the one that must be given.

## What counts as given

The two halves of a group are two kinds of rule, and they read a
[`default`](/spec/reference/flag) differently.

**Exclusivity** counts what was supplied — from the command line or from a member's
[`env`](/spec/reference/flag) variable — and not what was defaulted. This is the rule
[`conflicts`](/spec/reference/flag#conflicts-and-overrides) follows, and it has to be:
a defaulted member counted as supplied would collide with the sibling the user actually
typed, and refuse a correct command line.

**Requiredness** asks whether a member ended up with a value, and a default is a value.
This is the rule [`requires`](/spec/reference/flag#requires) and plain
[`required`](/spec/reference/flag) follow. A required group whose member has a default is
therefore always satisfied — which is worth noticing when you write one, since it means
the group enforces nothing.

## Members

A group needs at least two members, and naming fewer is an error where it is written
rather than a rule that quietly enforces nothing. A group of one is a statement about
that argument, which belongs on the argument as `required` or a relationship.

Flags are named by a dashed selector such as `--file` or `-f`. A positional is named by
its bare argument name, such as `TARGET`. The clap bridge resolves argument IDs to these
spellings, so mixed flag/positional groups keep every member.

Members are counted by the argument they name, not by the selector, so a group listing
both `-f` and `--file` holds one member and not two. Listing both is redundant rather
than wrong, and a flag is never in conflict with itself.

## From the derive

`#[derive(usage::Cli)]` writes the same group with membership on the fields and the
properties on the struct:

```rust
#[usage(group("input", required))]
struct Ex {
    #[usage(long, group = "input")]
    file: Option<String>,
    #[usage(long, group = "input")]
    url: Option<String>,
}
```

The `#[usage(group(...))]` line can be left out when the group is a plain "at most one".
A group with fewer than two members, or a declaration no field joins, is a compile error
rather than a rule that quietly holds for nothing.

## Coming from clap

`ArgGroup` carries across, with `required` and `multiple` read the same way. A group that
is `multiple` without being `required` states no rule at all, so it is dropped rather
than written out — which matters more than it sounds, because clap's _derive_ creates
exactly such a group for every `#[derive(Args)]` struct, named after the struct, to make
`flatten` work. Those are clap's bookkeeping, not a rule anyone declared.
