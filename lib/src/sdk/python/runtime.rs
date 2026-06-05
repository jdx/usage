pub const RUNTIME_PY: &str = r#"# Runtime module for usage-generated SDK clients. Do not edit manually.
from __future__ import annotations

import subprocess


class CliResult:
    """Result of a CLI invocation."""

    def __init__(self, stdout: str, stderr: str, exit_code: int) -> None:
        self.stdout = stdout
        self.stderr = stderr
        self.exit_code = exit_code

    @property
    def ok(self) -> bool:
        return self.exit_code == 0


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
            raise RuntimeError(f"CLI binary not found: {self.bin_path}")
"#;
