# The config conformance corpus

Test vectors for configuration resolution. Each one pairs a set of settings with
the layers that supply values, and the resolution the two must produce.

The [`config` block](https://usage.jdx.dev/spec/reference/config) is how a spec
_declares_ settings; [resolution](https://usage.jdx.dev/spec/resolution) is what
happens to them. That page is the prose and these vectors are the same rules made
executable, the way the argv grammar and its corpus sit together.

The vectors are KDL because KDL is usage's canonical format. If you are writing a
usage config resolver in another language, this directory is the definition of
correct; consuming usage's format is part of implementing compatibility.

## Why a registry, not a spec

An argv vector carries a KDL spec, because parsing a command line is a question
about a spec. A resolution is not: it is a question about a **registry** — keys,
types, defaults, merge policies — which a CLI's build step produces from its spec
long before anything is resolved.

So a vector describes the registry directly in KDL. How a _declaration_ becomes a
registry is the `usage::Config` derive's question, and its own tests answer it.

## Format

One file per area of resolution, each with a `section`, an `about`, and `vector`
nodes:

```kdl
vector "cli-beats-env-beats-file" doc="The declared order decides." {
  setting "jobs" type="uint" default=4
  layer "cli" id="--jobs" {
    value "jobs" "8"
  }
  layer "env" id="EX_JOBS" {
    value "jobs" "6"
  }
  layer "file" id="hk.toml" {
    value "jobs" "2"
  }
  expect {
    value "jobs" 8
    origin "jobs" "--jobs"
  }
}
```

| field     | meaning                                                                                       |
| --------- | --------------------------------------------------------------------------------------------- |
| `id`      | unique across the corpus, and stable — reports quote it                                       |
| `doc`     | what this vector pins down, in one sentence                                                   |
| `setting` | the registry: what exists, and what each setting says about itself                            |
| `layer`   | what supplies values, **highest precedence first**; absent means none, so only defaults apply |
| `expect`  | the values, where they came from, and what the resolution had to say                          |

### `settings`

`key` and `type` are the whole of most of them. `type` is the spec's own grammar:
`bool`, `int`, `uint`, `float`, `string`, `path`, `url`, `duration`, `object`,
`list<T>`, `set<T>`, `map<string, T>`, `option<T>`, and `any` — which is also what
a union like `bool|string` means here, since a union is precisely the spec saying
that nothing is decided.

The rest are what the setting declares about itself: `default`, `merge`
(`replace`, `union`, `deep`), `scope` (`any`, `global`, `env`), `parse` (a named
splitter such as `list_by_comma`), `choice`, `renamed-to`, `deprecated`. Scalar
defaults are properties; list defaults and choices are child nodes.

### `layers`

A layer is described by what it _supplies_, not by where it read it — nothing in
this corpus opens a file or reads an environment, so no vector's result can depend
on the machine running it.

- `source` — `cli`, `env`, `file`, or a name only the tool knows, like `git`.
- `id` — what a report calls it: the variable's name, the file's path.
- `trust` — `invocation`, `operator`, or `project`, which is what the scope rules
  read. Left out, it is what the kind implies: the command line and the environment
  are the user's own, and anything else is taken to be something a repository could
  carry until it says otherwise.
- `value` — a key and text, the way a layer that reads an environment hands it over.
- `shaped` — a key and a value with a shape of its own, the way a layer that reads
  TOML or JSON hands them over.

### `expect`

`value`, `list`, and `map` nodes are the whole resolution: a setting the vector
leaves out is expected to have no value at all, so a vector says what it means
rather than only what it is interested in. `origin` is checked where a vector names
a key and ignored where it does not, since most vectors are not about where a value
came from.

`warning` names the _kinds_ of thing the resolution had to say, in order —
`unknown-setting`, `wrong-type`, `not-allowed`, `out-of-scope`, `deprecated`,
`renamed`, `not-read`. Kinds and never messages: wording is a
quality-of-implementation concern and is expected to differ between
implementations, which is the same line the argv corpus holds for its error codes.

## No divergence field

The argv corpus records where usage-lib disagrees with the grammar, because the
grammar predates it and has two implementations. Resolution has one, and a
disagreement would be a bug to fix rather than a fact to record. If you implement
this elsewhere, every vector applies to you: there is no layer of a resolution that
can be left to somebody else, the way a binding-only argv parser can leave
`required` to the layer above it.

## Running them

```sh
cargo test -p usage-conformance --test config
```
