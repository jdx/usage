# usage-java

Run the [usage](https://usage.jdx.dev) CLI from Java via [Chicory](https://github.com/nicholasgasior/chicory), a pure-Java WebAssembly runtime.

The `usage` CLI is compiled to WebAssembly (WASI) and executed at runtime through Chicory with build-time AOT compilation for fast execution.

## Prerequisites

Build the WASM binary (requires wasi-sdk):

```bash
mise run wasm:build
```

## Build

```bash
cd integrations/java
mvn compile
```

## Quick Start

```java
import dev.jdx.usage.Usage;
import dev.jdx.usage.UsageResult;

String spec =
    "name \"mycli\"\n"
    + "bin \"mycli\"\n"
    + "version \"1.0.0\"\n"
    + "flag \"-v --verbose\" help=\"Verbose output\"\n"
    + "cmd \"run\" help=\"Run the thing\" {\n"
    + "    arg <target>\n"
    + "}\n";

// Generate bash completions
UsageResult result = Usage.builder()
    .withStdin(spec)
    .withArgs("generate", "completion", "bash", "mycli", "-f", "-")
    .run();

System.out.println(result.stdoutAsString());
```

## API

### `Usage.builder()`

Creates a builder for running usage CLI commands.

| Method | Description |
| --- | --- |
| `.withStdin(String)` | Pipe a string to stdin (typically a usage spec) |
| `.withStdin(byte[])` | Pipe bytes to stdin |
| `.withArgs(String...)` | Append CLI arguments (the `usage` program name is added automatically) |
| `.withDirectory(Path)` | Pre-open a directory for WASI filesystem access (needed for `--out-file`) |
| `.run()` | Execute and return a `UsageResult` |

### `UsageResult`

| Method | Description |
| --- | --- |
| `.stdout()` | Raw stdout bytes |
| `.stderr()` | Raw stderr bytes |
| `.stdoutAsString()` | Stdout as UTF-8 string |
| `.stderrAsString()` | Stderr as UTF-8 string |
| `.exitCode()` | Process exit code |
| `.success()` | `true` if exit code is 0 |

## Common Commands

```java
// Generate shell completions
Usage.builder().withStdin(spec).withArgs("generate", "completion", "bash", "mybinary", "-f", "-").run();
Usage.builder().withStdin(spec).withArgs("generate", "completion", "zsh", "mybinary", "-f", "-").run();
Usage.builder().withStdin(spec).withArgs("generate", "completion", "fish", "mybinary", "-f", "-").run();

// Generate man page
Usage.builder().withStdin(spec).withArgs("generate", "manpage", "-f", "-").run();

// Generate JSON spec
Usage.builder().withStdin(spec).withArgs("generate", "json", "-f", "-").run();

// Generate markdown (requires a directory for --out-file, use ZeroFs for in-memory)
FileSystem fs = ZeroFs.newFileSystem(Configuration.unix().toBuilder().setAttributeViews("unix").build());
Path outDir = fs.getPath("out");
Files.createDirectory(outDir);
Usage.builder()
    .withStdin(spec)
    .withArgs("generate", "markdown", "-f", "-", "--out-file", outDir.resolve("docs.md").toString())
    .withDirectory(outDir)
    .run();
String markdown = new String(Files.readAllBytes(outDir.resolve("docs.md")));
```

## Example

See [`example/Main.java`](example/Main.java) for a complete example mirroring the [cobra integration example](../cobra/example/main.go).

Run it with [jbang](https://www.jbang.dev/):

```bash
mise run wasm:build
cd integrations/java
mvn install
jbang example/Main.java
```
