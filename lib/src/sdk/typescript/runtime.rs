pub const RUNTIME_TS: &str = r#"// Runtime module for usage-generated SDK clients. Do not edit manually.
import { execFileSync } from "node:child_process";

export class CliResult {
  constructor(
    public readonly stdout: string,
    public readonly stderr: string,
    public readonly exitCode: number,
  ) {}

  get ok(): boolean {
    return this.exitCode === 0;
  }
}

export class CliRunner {
  constructor(private binPath: string) {}

  run(args: string[], flags?: Record<string, unknown>): CliResult {
    const flagArgs: string[] = [];
    if (flags) {
      for (const [key, value] of Object.entries(flags)) {
        if (value === undefined || value === null) continue;
        if (typeof value === "boolean") {
          if (value) flagArgs.push(`--${key}`);
        } else {
          flagArgs.push(`--${key}`, String(value));
        }
      }
    }

    try {
      const stdout = execFileSync(this.binPath, [...args, ...flagArgs], {
        encoding: "utf-8",
      });
      return new CliResult(stdout, "", 0);
    } catch (e: unknown) {
      const err = e as { stdout?: string; stderr?: string; status?: number };
      return new CliResult(err.stdout ?? "", err.stderr ?? "", err.status ?? 1);
    }
  }
}
"#;
