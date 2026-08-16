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

A member is given when it ended up with a value from the command line or from its
[`env`](/spec/reference/flag) variable — the same rule
[`conflicts`](/spec/reference/flag#conflicts-and-overrides) and
[`requires`](/spec/reference/flag#requires) follow.

A [`default`](/spec/reference/flag) does not count. A default that satisfied a required
group would make the group unfalsifiable — it could never report anything — and one that
collided with a typed sibling would refuse a command line where the user named exactly
one flag.

## Members

A group needs at least two members, and naming fewer is an error where it is written
rather than a rule that quietly enforces nothing. A group of one is a statement about
that flag, which belongs on the flag as [`required`](/spec/reference/flag) or
[`requires`](/spec/reference/flag#requires).

Only flags can be members. clap allows a positional in a group; a spec generated from
such a command keeps the flags and drops the positional, since the spec has no way to
name one in a relationship and a selector matching nothing would read as a rule that
holds.
