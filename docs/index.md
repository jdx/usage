---
layout: home
title: CLI specifications and framework tooling
description: Build a typed Rust CLI or describe an existing tool in KDL. Generate shell completions, help, reference docs, man pages, and SDKs from one definition.
---

<div class="usage-home-content">

<div class="usage-home-heading">
<p class="usage-section-label">Choose your starting point</p>

## Built around the CLI you want to ship.

</div>

<div class="usage-paths">
<div>
<span class="usage-path-number" aria-hidden="true">01 / BUILD</span>

### A new Rust CLI

Declare commands with structs and enums. Get typed parsing, help, completion,
validation, and configuration support, plus a portable spec you can export.

[Start the Rust quickstart →](/rust/quickstart)

</div>
<div>
<span class="usage-path-number" aria-hidden="true">02 / CONNECT</span>

### An existing command-line tool

Export a spec from clap, Cobra, or another framework, or write KDL directly.
Generate reference docs and completions while keeping your application code.

[Find an integration →](/spec/integrations)

</div>
<div>
<span class="usage-path-number" aria-hidden="true">03 / SCRIPT</span>

### A script that needs an interface

Declare arguments and flags in comments. Usage validates the input, handles
`--help`, and passes the parsed values to your script as environment variables.

[Add parsing to a script →](/cli/scripts)

</div>
</div>

<div class="usage-toolkit">
<div>
<p class="usage-section-label">One definition, every artifact</p>

## Spend less time keeping things in sync.

Use the same commands, descriptions, choices, and defaults across the tools your
users see. Regenerate the artifacts as your interface evolves.

[Install the Usage CLI →](/cli/#installation)

</div>
<div class="usage-toolkit-links">

[**Shell completions** <span>Suggest commands, flags, files, and live values →</span>](/cli/completions)

[**Reference documentation** <span>Generate Markdown pages and Unix man pages →</span>](/cli/markdown)

[**TypeScript and Python SDKs** <span>Call your CLI through typed subprocess clients →</span>](/cli/sdk)

[**Interface checks** <span>Lint a spec and find breaking changes before a release →</span>](/cli/diff)

</div>
</div>
</div>

<UsageBenches />

<div class="usage-home-footer">

Looking for the Go framework? [Read the development preview →](/go/)

</div>
