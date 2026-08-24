# Third-party notices

Usage itself is licensed under the MIT License; see [LICENSE](LICENSE). This file
records the third-party work that Usage vendors, derives from, or is closely
modeled on, together with the license each is used under.

## kdl-rs and miette

`src/kdl/` is a trimmed and modified copy of
[kdl-rs 6.7.1](https://github.com/kdl-org/kdl-rs/tree/v6.7.1). It retains the KDL v2
document model, formatting, parser, and source spans; serde support, queries, KDL v1
fallback, and miette integration were removed. kdl-rs is copyright Kat Marchán and
the KDL Community.

`src/miette.rs` is Usage's small diagnostic compatibility layer. Its API and rendered
diagnostic conventions are modeled on
[miette 7.6.0](https://github.com/zkat/miette/tree/7.6.0), copyright Kat Marchán; it
does not contain miette's implementation.

Both projects are distributed under the Apache License, Version 2.0. Usage takes
them under that license, reproduced in `third-party/LICENSE-APACHE-2.0`.

## heck and shell-words

`lib/src/case.rs` is a focused adaptation of the word-boundary algorithm from
[heck 0.5.0](https://github.com/withoutboats/heck/tree/0.5.0). It retains only the
snake_case, lowerCamelCase, and PascalCase conversions used by Usage.

`lib/src/shell_words.rs` is a focused adaptation of
[shell-words 1.1.1](https://github.com/tmiasko/shell-words/tree/1.1.1). It retains the
POSIX word splitting and quoting needed for mounted commands and parser output.
shell-words is copyright 2018 Tomasz Miąsko.

Both projects are distributed under the terms of either the MIT License or the Apache
License, Version 2.0. Usage takes the adapted code under the Apache License, reproduced
in `lib/third-party/LICENSE-APACHE-2.0`.

## clap

Usage's design owes a great deal to [clap](https://github.com/clap-rs/clap). No
clap source is vendored here, but clap's design is reproduced closely enough to
warrant attribution:

- `usage-derive` / `usage-rs` deliberately mirror `clap_derive`'s attribute
  vocabulary and semantics (`long`, `short`, `env`, `default_value`, `flatten`,
  `value_enum`, `rename_all`, and friends) so a clap declaration can be ported
  field by field. See [docs/rust/migrating-from-clap.md](docs/rust/migrating-from-clap.md).
- The rendered help, usage line, and diagnostic conventions follow clap's output
  shape so migrated CLIs keep their existing user-facing text.
- `clap_usage` reads a `clap::Command` through clap's public API to generate a
  spec, and the conformance suite asserts parity against clap's behavior.

clap is distributed under the terms of either the MIT license or the Apache
License, Version 2.0, at the user's option. Usage takes it under the MIT option,
reproduced verbatim below from clap's `LICENSE-MIT` (clap 4.6.6, the version in
this workspace's lockfile):

```
Copyright (c) Individual contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The Apache-2.0 option is available upstream at
<https://github.com/clap-rs/clap/blob/master/LICENSE-APACHE>.

## clap adopter probes

`benches/shadows/external-*` and `conformance/tests/external_clap_adopters.rs`
are reductions of three real clap-based CLIs, pinned to the revisions recorded in
[benches/external/README.md](benches/external/README.md). They are reduced rather
than vendored, but they do carry each upstream's declaration shapes and help text,
so each upstream's license is reproduced below.

### fd

[sharkdp/fd](https://github.com/sharkdp/fd), revision
`ee20f426ddf338ac7ead5c5f00ea49258005caaf`, dual MIT / Apache-2.0. Taken under the
MIT option:

```
MIT License

Copyright (c) 2017-present The fd developers

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### tokei

[XAMPPRocky/tokei](https://github.com/XAMPPRocky/tokei), revision
`fa44e5194060305576514d59b850353643afbfc8`, dual MIT / Apache-2.0. Taken under the
MIT option:

```
MIT License (MIT)

Copyright (c) 2016 Erin Power

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

### starship

[starship/starship](https://github.com/starship/starship), revision
`6d38f35391a8e68952a3dd4b9acd40d3d93596f6`, ISC:

```
ISC License

Copyright (c) 2019-2022, Starship Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

## heck

`derive/src/case.rs` reproduces the word-boundary rules and case conversions of
[heck](https://github.com/withoutboats/heck). No heck source is vendored, but the
algorithm is followed closely enough to warrant attribution: usage's `rename_all`
vocabulary is a clone of `clap_derive`'s, `clap_derive` uses heck, and a declaration
ported from clap has to produce the same names — so the reimplementation exists to keep
heck out of every adopter's compile, not to behave differently. `derive/src/case.rs`
tests itself against heck directly, and `usage-lib` still depends on heck as a normal
dependency.

heck is distributed under the terms of either the MIT license or the Apache License,
Version 2.0, at the user's option. Usage takes it under the MIT option, reproduced
verbatim below from heck's `LICENSE-MIT` (heck 0.5.0, the version in this workspace's
lockfile):

```
Copyright (c) 2015 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

The Apache-2.0 option is available upstream at
<https://github.com/withoutboats/heck/blob/master/LICENSE-APACHE>.
