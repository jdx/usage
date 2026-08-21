# Third-party notices

Usage itself is licensed under the MIT License; see [LICENSE](LICENSE). This file
records the third-party work that Usage vendors, derives from, or is closely
modeled on, together with the license each is used under.

## clap

Usage's design owes a great deal to [clap](https://github.com/clap-rs/clap). No
clap source is vendored here, but clap's design is reproduced closely enough to
warrant attribution:

- `usage-derive` / `usage-rs` deliberately mirror `clap_derive`'s attribute
  vocabulary and semantics (`long`, `short`, `env`, `default_value`, `flatten`,
  `value_enum`, `rename_all`, and friends) so a clap declaration can be ported
  field by field. See [docs/rust/clap-compatibility.md](docs/rust/clap-compatibility.md).
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

## bash-completion

[`lib/bash-completion/bash_completion`](lib/bash-completion/bash_completion) is a
verbatim copy of the `bash_completion` script from
[scop/bash-completion](https://github.com/scop/bash-completion) (version 2.15.0).
`lib/src/complete/bash.rs` embeds it with `include_str!` and emits it when
`usage generate completion bash --include-bash-completion-lib` is used, so the
script is redistributed both in the `usage-lib` crate and in any binary built
from it.

**bash-completion is licensed under the GNU General Public License, version 2 or
later** — not MIT. The full license text ships alongside the script in
[lib/bash-completion/COPYING](lib/bash-completion/COPYING), and the upstream
copyright header is preserved at the top of the script itself:

```
Copyright © 2006-2008, Ian Macdonald <ian@caliban.org>
          © 2009-2020, Bash Completion Maintainers
```

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
