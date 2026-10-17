import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { Argument, Command, Flag } from "effect/cli";
import { generate, generateWithReport } from "../src/index.ts";

const serve = Command.make("serve", {
  port: Flag.Int("port").pipe(
    Flag.withAlias("p"),
    Flag.withDefault(8080),
    Flag.withDescription("Port to bind")
  ),
  mode: Flag.Literals("mode", ["dev", "prod"]),
  files: Argument.String("file").pipe(Argument.variadic({ min: 1 })),
}).pipe(Command.withDescription("Serve files"), Command.withAlias("s"));

const root = Command.make("app", {
  verbose: Flag.Boolean("verbose").pipe(Flag.withAlias("v")),
  name: Flag.String("name").pipe(Flag.optional),
  target: Argument.String("target").pipe(Argument.optional),
}).pipe(
  Command.withDescription("My app"),
  Command.withSubcommands([
    serve,
    Command.make("secret").pipe(Command.unlisted),
  ])
);

test("exports commands, flags, args, aliases, choices and defaults", () => {
  const kdl = generate(root, { version: "1.0.0" });
  assert.match(
    kdl,
    /^name "app"\nbin "app"\nversion "1.0.0"\nabout "My app"\n/
  );
  assert.match(kdl, /flag "-v --verbose"\n/);
  assert.match(kdl, /flag "--name" \{\n {4}arg "<value>"\n\}/);
  assert.match(kdl, /arg "\[target\]" required=#false/);
  assert.match(kdl, /cmd "serve" help="Serve files" \{\n {4}alias "s"/);
  assert.match(kdl, /flag "-p --port" help="Port to bind" default="8080"/);
  assert.match(
    kdl,
    /flag "--mode" required=#true \{\n {8}arg "<choice>" \{\n {12}choices "dev" "prod"/
  );
  assert.match(kdl, /arg "<file>…" var=#true var_min=1/);
  assert.match(kdl, /cmd "secret" hide=#true/);
});

test("reports constructs it cannot represent exactly", () => {
  const cmd = Command.make("x", {
    a: Flag.String("a").pipe(Flag.orElse(Flag.String("b"))),
  });
  const { unsupported } = generateWithReport(cmd);
  assert.equal(unsupported.length, 1);
  assert.match(unsupported[0]!, /alternatives/);
});

test("output parses with the usage CLI", (t) => {
  const kdl = generate(root, { version: "1.0.0" });
  const r = spawnSync(
    process.env.USAGE_BIN ?? "usage",
    ["generate", "md", "--file", "-"],
    { input: kdl, encoding: "utf8" }
  );
  if (r.error) return t.skip("usage CLI not installed");
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /--mode/);
});
