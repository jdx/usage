pub const RUNTIME_TS: &str = r#"// Runtime module for usage-generated SDK clients. Do not edit manually.
import { spawn, ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";

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

/**
 * A result whose stdout was declared `framing="json"`, already parsed.
 *
 * `data` is null when the command printed nothing. The exit code is still here: a
 * declared non-zero code such as "a check failed" is an outcome rather than an error,
 * so reading one of those never throws.
 */
export class CliJsonResult<T = unknown> extends CliResult {
  constructor(
    stdout: string,
    stderr: string,
    exitCode: number,
    public readonly data: T | null,
  ) {
    super(stdout, stderr, exitCode);
  }
}

export class CliError extends Error {
  constructor(
    public readonly binPath: string,
    message: string,
    public readonly exitCode?: number,
    public readonly stderr?: string,
  ) {
    super(message);
    this.name = "CliError";
  }
}

/**
 * A `framing="jsonl"` stream: one parsed object per line, as they arrive.
 *
 * `for await` consumes stdout lazily, so a command that never ends is fine as long as
 * the consumer stops reading — breaking out of the loop kills the child. `exitCode` is
 * null until the stream ends.
 */
export class CliStream<T = unknown> implements AsyncIterable<T> {
  exitCode: number | null = null;
  private chunks: string[] = [];
  private exited: Promise<number>;

  constructor(
    private child: ChildProcess,
    private binPath: string,
  ) {
    child.stderr?.setEncoding("utf-8").on("data", (chunk: string) => { this.chunks.push(chunk); });
    this.exited = new Promise<number>((resolve, reject) => {
      child.on("error", (err: NodeJS.ErrnoException) => {
        reject(
          err.code === "ENOENT"
            ? new CliError(this.binPath, `CLI binary not found: ${this.binPath}`)
            : err,
        );
      });
      child.on("close", (code: number | null) => {
        this.exitCode = code ?? 1;
        resolve(this.exitCode);
      });
    });
    // A stream that is constructed and never iterated must not take the process down
    // with an unhandled rejection when the binary is missing. `wait()` still sees it.
    void this.exited.catch(() => {});
  }

  get stderr(): string {
    return this.chunks.join("");
  }

  /** Resolves with the exit code once the child has ended. */
  wait(): Promise<number> {
    return this.exited;
  }

  /**
   * Stop the child without reading the rest of its output.
   *
   * Signalling is all this can do synchronously — `exitCode` is set when the process
   * actually ends, so `await wait()` afterwards if you need it.
   */
  close(): void {
    if (this.exitCode === null) this.child.kill();
  }

  async *[Symbol.asyncIterator](): AsyncGenerator<T, void, unknown> {
    if (!this.child.stdout) return;
    const lines = createInterface({ input: this.child.stdout, crlfDelay: Infinity });
    try {
      for await (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === "") continue;
        try {
          yield JSON.parse(trimmed) as T;
        } catch (e) {
          throw new CliError(
            this.binPath,
            `invalid JSON on a jsonl line: ${String(e)}`,
            this.exitCode ?? undefined,
            this.stderr,
          );
        }
      }
      await this.exited;
    } finally {
      lines.close();
      this.close();
    }
  }
}

export class CliRunner {
  constructor(private binPath: string) {}

  async run(args: string[]): Promise<CliResult> {
    return new Promise<CliResult>((resolve, reject) => {
      const child: ChildProcess = spawn(this.binPath, args, {
        stdio: ["pipe", "pipe", "pipe"],
      });
      let stdout = "";
      let stderr = "";
      child.stdout?.setEncoding("utf-8").on("data", (chunk: string) => { stdout += chunk; });
      child.stderr?.setEncoding("utf-8").on("data", (chunk: string) => { stderr += chunk; });
      child.on("error", (err: NodeJS.ErrnoException) => {
        if (err.code === "ENOENT") {
          reject(new CliError(this.binPath, `CLI binary not found: ${this.binPath}`));
        } else {
          reject(err);
        }
      });
      child.on("close", (code: number | null) => {
        resolve(new CliResult(stdout, stderr, code ?? 1));
      });
    });
  }

  async runJson<T = unknown>(args: string[]): Promise<CliJsonResult<T>> {
    const result = await this.run(args);
    const text = result.stdout.trim();
    if (text === "") {
      return new CliJsonResult<T>(result.stdout, result.stderr, result.exitCode, null);
    }
    try {
      return new CliJsonResult<T>(
        result.stdout,
        result.stderr,
        result.exitCode,
        JSON.parse(text) as T,
      );
    } catch (e) {
      throw new CliError(
        this.binPath,
        `expected JSON on stdout: ${String(e)}`,
        result.exitCode,
        result.stderr,
      );
    }
  }

  runJsonl<T = unknown>(args: string[]): CliStream<T> {
    return new CliStream<T>(
      spawn(this.binPath, args, { stdio: ["pipe", "pipe", "pipe"] }),
      this.binPath,
    );
  }
}
"#;
