// Generate a usage spec (KDL) from an Effect v4 `effect/cli` command tree.
//
// Effect's public `Command` type exposes names, descriptions, aliases, examples
// and subcommands, but not flags or arguments. Those are read from the runtime
// `config` field the command carries, the same data Effect's own help and
// shell completions use. `readConfig` fails loudly when that shape changes.
import type { Command } from "effect/cli";

export interface GenerateOptions {
  /** Override the spec `name` and `bin`; defaults to the root command's name. */
  name?: string;
  /** Written as the spec `version`. */
  version?: string;
}

export interface GenerateResult {
  kdl: string;
  /** Constructs that could not be represented exactly, with where they occur. */
  unsupported: string[];
}

type Any = Command.Command<any, any, any, any, any>;

interface Single {
  _tag: "Single";
  kind: "flag" | "argument";
  name: string;
  aliases: readonly string[];
  hidden: boolean;
  description: { _tag: "Some" | "None"; value?: string };
  primitiveType: {
    _tag: string;
    choiceKeys?: readonly string[];
    pathType?: string;
  };
  typeName?: string;
  optionalValue?: string;
}

interface Param {
  _tag: string;
  param?: Param;
  alternatives?: readonly (Param | (() => Param))[];
  f?: (value: unknown) => unknown;
  min?: { _tag: string; value?: number };
  max?: { _tag: string; value?: number };
  [key: string]: unknown;
}

interface Shape {
  required: boolean;
  variadic: boolean;
  min?: number;
  max?: number;
  single: Single;
  default?: string;
}

class Report {
  readonly items: string[] = [];
  note(where: string, what: string): void {
    this.items.push(`${where}: ${what}`);
  }
}

/** Render the command tree as a usage spec in KDL. */
export function generate(root: Any, options: GenerateOptions = {}): string {
  return generateWithReport(root, options).kdl;
}

/** Like {@link generate}, but also reports what could not be represented. */
export function generateWithReport(
  root: Any,
  options: GenerateOptions = {}
): GenerateResult {
  const report = new Report();
  const name = options.name ?? root.name;
  const out: string[] = [`name ${q(name)}`, `bin ${q(name)}`];
  if (options.version) out.push(`version ${q(options.version)}`);
  const summary = root.shortDescription ?? root.description;
  if (summary) out.push(`about ${q(firstLine(summary))}`);
  if (
    root.description &&
    (root.description !== summary || root.description.includes("\n"))
  ) {
    out.push(`long_about ${q(root.description)}`);
  }
  body(root, 0, out, report, name);
  return { kdl: out.join("\n") + "\n", unsupported: report.items };
}

function body(
  cmd: Any,
  depth: number,
  out: string[],
  report: Report,
  path: string
): void {
  const pad = "    ".repeat(depth);
  const cfg = readConfig(cmd);

  // Shared flags appear in both lists; emit them once, as global.
  const shared = new Set<Param>(cfg.contextFlags);
  for (const p of cfg.flags) {
    if (shared.has(p)) continue;
    out.push(...renderFlag(p, false, pad, report, path));
  }
  for (const p of cfg.arguments) out.push(...renderArg(p, pad, report, path));
  for (const p of cfg.contextFlags)
    out.push(...renderFlag(p, true, pad, report, path));
  for (const ex of cmd.examples) {
    out.push(
      `${pad}example ${q(ex.command)}${ex.description ? ` help=${q(ex.description)}` : ""}`
    );
  }
  for (const group of cmd.subcommands) {
    for (const sub of group.commands)
      emitCommand(sub, depth, out, report, `${path} ${sub.name}`);
  }
}

function emitCommand(
  cmd: Any,
  depth: number,
  out: string[],
  report: Report,
  path: string
): void {
  const pad = "    ".repeat(depth);
  const props: string[] = [];
  const summary = cmd.shortDescription ?? cmd.description;
  if (summary) props.push(`help=${q(firstLine(summary))}`);
  if (cmd.unlisted) props.push("hide=#true");
  const head = `${pad}cmd ${q(cmd.name)}${props.length ? " " + props.join(" ") : ""}`;
  const inner: string[] = [];
  const ipad = "    ".repeat(depth + 1);
  if (cmd.alias && cmd.alias !== cmd.name)
    inner.push(`${ipad}alias ${q(cmd.alias)}`);
  if (
    cmd.description &&
    (cmd.description !== summary || cmd.description.includes("\n"))
  ) {
    inner.push(`${ipad}long_help ${q(cmd.description)}`);
  }
  body(cmd, depth + 1, inner, report, path);
  out.push(inner.length ? `${head} {\n${inner.join("\n")}\n${pad}}` : head);
}

function renderFlag(
  param: Param,
  shared: boolean,
  pad: string,
  report: Report,
  path: string
): string[] {
  const shape = shapeOf(param, "flag", report, path);
  if (!shape) return [];
  const s = shape.single;
  const shorts = [s.name, ...s.aliases]
    .filter((n) => n.length === 1)
    .map((n) => `-${n}`);
  const longs = [s.name, ...s.aliases]
    .filter((n) => n.length > 1)
    .map((n) => `--${n}`);
  const props: string[] = [];
  const help = Option(s.description);
  if (help) props.push(`help=${q(firstLine(help))}`);
  const isBool = s.primitiveType._tag === "Boolean";
  if (shape.required) props.push("required=#true");
  if (shape.variadic) {
    props.push("var=#true");
    if (shape.min) props.push(`var_min=${shape.min}`);
    if (shape.max !== undefined) props.push(`var_max=${shape.max}`);
  }
  if (s.hidden) props.push("hide=#true");
  if (shared) props.push("global=#true");
  if (shape.default !== undefined) props.push(`default=${q(shape.default)}`);
  const lines = [
    `${pad}flag ${q([...shorts, ...longs].join(" "))}${props.length ? " " + props.join(" ") : ""}`,
  ];
  if (s.optionalValue !== undefined) {
    report.note(
      `${path} --${s.name}`,
      "optional flag value is exported as a required value"
    );
  }
  const children: string[] = [];
  if (help && help.includes("\n")) {
    children.push(`${pad}    long_help ${q(help)}`);
  }
  if (!isBool) {
    const arg = `<${s.typeName ?? typeLabel(s)}>`;
    const choices = choicesOf(s);
    if (choices.length) {
      children.push(
        `${pad}    arg ${q(arg)} {`,
        `${pad}        choices ${choices.map(q).join(" ")}`,
        `${pad}    }`
      );
    } else {
      children.push(`${pad}    arg ${q(arg)}`);
    }
  }
  if (!children.length) return lines;
  lines[0] += " {";
  lines.push(...children, `${pad}}`);
  return lines;
}

function renderArg(
  param: Param,
  pad: string,
  report: Report,
  path: string
): string[] {
  const shape = shapeOf(param, "argument", report, path);
  if (!shape) return [];
  const s = shape.single;
  const label = `${shape.required ? "<" + s.name + ">" : "[" + s.name + "]"}${shape.variadic ? "…" : ""}`;
  const props: string[] = [];
  const help = Option(s.description);
  if (help) props.push(`help=${q(firstLine(help))}`);
  if (help && help.includes("\n")) props.push(`long_help=${q(help)}`);
  if (!shape.required) props.push("required=#false");
  if (shape.variadic) {
    props.push("var=#true");
    if (shape.min) props.push(`var_min=${shape.min}`);
    if (shape.max !== undefined) props.push(`var_max=${shape.max}`);
  }
  if (s.hidden) props.push("hide=#true");
  if (shape.default !== undefined) props.push(`default=${q(shape.default)}`);
  const head = `${pad}arg ${q(label)}${props.length ? " " + props.join(" ") : ""}`;
  const choices = choicesOf(s);
  if (!choices.length) return [head];
  return [
    `${head} {`,
    `${pad}    choices ${choices.map(q).join(" ")}`,
    `${pad}}`,
  ];
}

function shapeOf(
  param: Param,
  kind: "flag" | "argument",
  report: Report,
  path: string
): Shape | undefined {
  const all = singles(param);
  const first = all[0];
  if (!first) return undefined;
  const where = `${path} ${kind === "flag" ? "--" : ""}${first.name}`;
  if (all.length > 1)
    report.note(
      where,
      "alternatives (orElse) are exported as the first one only"
    );
  let required = true;
  let variadic = false;
  let min: number | undefined;
  let max: number | undefined;
  let dflt: string | undefined;
  for (let p: Param | undefined = param; p; p = p.param) {
    if (p._tag === "Optional") required = false;
    if (p._tag === "Variadic") {
      variadic = true;
      min = p.min?.value;
      max = p.max?.value;
    }
    if (p._tag === "Map" && p.param?._tag === "Optional" && p.f) {
      // withDefault wraps the optional in a map that supplies the value.
      try {
        const v = p.f({ _tag: "None", _id: "Option" });
        if (
          typeof v === "string" ||
          typeof v === "number" ||
          typeof v === "boolean"
        )
          dflt = String(v);
        else if (v !== undefined)
          report.note(
            where,
            "default value is not a string, number or boolean"
          );
      } catch {
        report.note(where, "default value could not be evaluated");
      }
    }
  }
  const tag = first.primitiveType._tag;
  if (["FileParse", "FileSchema", "Redacted", "KeyValuePair"].includes(tag)) {
    report.note(where, `${tag} values are exported as plain strings`);
  }
  // A variadic without a positive minimum accepts zero values.
  if (variadic && !(min !== undefined && min > 0)) required = false;
  return {
    required: required && dflt === undefined,
    variadic,
    min,
    max,
    single: first,
    default: dflt,
  };
}

function singles(param: Param): Single[] {
  switch (param._tag) {
    case "Single":
      return [param as unknown as Single];
    case "Map":
    case "Optional":
    case "Variadic":
      return singles(param.param!);
    case "Transform":
      return [
        ...singles(param.param!),
        ...(param.alternatives ?? []).flatMap((a) =>
          singles(typeof a === "function" ? a() : a)
        ),
      ];
    default:
      throw new Error(
        `usage-spec-effect: unknown Effect param kind "${param._tag}"; this effect version is not supported`
      );
  }
}

function readConfig(cmd: Any): {
  flags: Param[];
  arguments: Param[];
  contextFlags: Param[];
} {
  const c = cmd as unknown as {
    config?: { flags?: Param[]; arguments?: Param[] };
    contextConfig?: { flags?: Param[] };
  };
  if (
    !c.config ||
    !Array.isArray(c.config.flags) ||
    !Array.isArray(c.config.arguments)
  ) {
    throw new Error(
      `usage-spec-effect: cannot read the flags of command "${cmd.name}"; this effect version is not supported`
    );
  }
  return {
    flags: c.config.flags,
    arguments: c.config.arguments,
    contextFlags: c.contextConfig?.flags ?? [],
  };
}

function choicesOf(s: Single): string[] {
  return s.primitiveType._tag === "Choice"
    ? [...(s.primitiveType.choiceKeys ?? [])]
    : [];
}

function typeLabel(s: Single): string {
  switch (s.primitiveType._tag) {
    case "Int":
      return "int";
    case "Finite":
      return "number";
    case "Date":
      return "date";
    case "Path":
    case "FileText":
    case "FileParse":
    case "FileSchema":
      return "path";
    case "Choice":
      return "choice";
    default:
      return "value";
  }
}

function Option(o: { _tag: string; value?: string }): string | undefined {
  return o._tag === "Some" ? o.value : undefined;
}

function firstLine(s: string): string {
  return s.split("\n")[0]!.trim();
}

/** Quote a string as a KDL string. */
function q(s: string): string {
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n").replace(/\r/g, "\\r").replace(/\t/g, "\\t")}"`;
}
