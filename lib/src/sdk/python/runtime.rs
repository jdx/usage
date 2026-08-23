pub const RUNTIME_PY: &str = r#"# Runtime module for usage-generated SDK clients. Do not edit manually.
from __future__ import annotations

import json
import subprocess
import threading
from typing import Any, Iterator, Optional


class CliResult:
    """Result of a CLI invocation."""

    def __init__(self, stdout: str, stderr: str, exit_code: int) -> None:
        self.stdout = stdout
        self.stderr = stderr
        self.exit_code = exit_code

    @property
    def ok(self) -> bool:
        return self.exit_code == 0


class CliJsonResult(CliResult):
    """A result whose stdout was declared `framing=json`, already parsed.

    `data` is None when the command printed nothing. The exit code is still here: a
    declared non-zero code such as "a check failed" is an outcome rather than an
    error, so reading one of those never raises.
    """

    def __init__(self, stdout: str, stderr: str, exit_code: int, data: Any) -> None:
        super().__init__(stdout, stderr, exit_code)
        self.data = data


class CliError(RuntimeError):
    """An invocation that could not produce a result at all."""

    def __init__(
        self, message: str, exit_code: Optional[int] = None, stderr: str = ""
    ) -> None:
        super().__init__(message)
        self.exit_code = exit_code
        self.stderr = stderr


class CliStream:
    """A `framing=jsonl` stream: one parsed object per line, as they arrive.

    Iterating consumes stdout lazily, so a command that never ends is fine as long as
    the consumer stops reading. `exit_code` is None until the stream is exhausted or
    closed. Use it as a context manager, or call `close()`, to be sure the child is
    reaped when you stop early.
    """

    def __init__(self, proc: subprocess.Popen) -> None:
        self._proc = proc
        self._chunks: list[str] = []
        self._exhausted = False
        self.exit_code: Optional[int] = None
        # Drained on a thread, and not optional: with stderr piped and only stdout
        # being read, a child that writes more to stderr than the pipe buffer holds
        # blocks forever waiting for someone to empty it.
        self._pump = threading.Thread(target=self._drain, daemon=True)
        self._pump.start()

    def _drain(self) -> None:
        if self._proc.stderr is not None:
            for line in self._proc.stderr:
                self._chunks.append(line)

    @property
    def stderr(self) -> str:
        return "".join(self._chunks)

    def __iter__(self) -> Iterator[Any]:
        stdout = self._proc.stdout
        if stdout is None:
            return
        try:
            for line in stdout:
                line = line.strip()
                if not line:
                    continue
                try:
                    yield json.loads(line)
                except ValueError as e:
                    raise CliError(
                        f"invalid JSON on a jsonl line: {e}", stderr=self.stderr
                    )
            self._exhausted = True
        finally:
            self.close()

    def close(self) -> None:
        if self.exit_code is not None:
            return
        # Only signal a child that is still producing. One that reached EOF on its own
        # is already on its way out, and terminating it would report -15 for a run that
        # finished cleanly.
        if not self._exhausted and self._proc.poll() is None:
            self._proc.terminate()
        self.exit_code = self._proc.wait()
        self._pump.join(timeout=1)
        for pipe in (self._proc.stdout, self._proc.stderr):
            if pipe is not None:
                pipe.close()

    def __enter__(self) -> "CliStream":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


class CliRunner:
    """Runs a CLI binary via subprocess."""

    def __init__(self, bin_path: str) -> None:
        self.bin_path = bin_path

    def run(self, args: list[str]) -> CliResult:
        try:
            result = subprocess.run(
                [self.bin_path, *args],
                capture_output=True,
                text=True,
            )
            return CliResult(result.stdout, result.stderr, result.returncode)
        except FileNotFoundError:
            raise CliError(f"CLI binary not found: {self.bin_path}")

    def run_json(self, args: list[str]) -> CliJsonResult:
        """Run to completion and parse stdout as one JSON document."""
        result = self.run(args)
        text = result.stdout.strip()
        if not text:
            return CliJsonResult(result.stdout, result.stderr, result.exit_code, None)
        try:
            data = json.loads(text)
        except ValueError as e:
            raise CliError(
                f"expected JSON on stdout: {e}",
                exit_code=result.exit_code,
                stderr=result.stderr,
            )
        return CliJsonResult(result.stdout, result.stderr, result.exit_code, data)

    def run_jsonl(self, args: list[str]) -> CliStream:
        """Start the command and hand back stdout as parsed lines, as they arrive."""
        try:
            proc = subprocess.Popen(
                [self.bin_path, *args],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
            )
        except FileNotFoundError:
            raise CliError(f"CLI binary not found: {self.bin_path}")
        return CliStream(proc)
"#;
