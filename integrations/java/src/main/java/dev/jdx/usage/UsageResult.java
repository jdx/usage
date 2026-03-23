package dev.jdx.usage;

import java.nio.charset.StandardCharsets;

public class UsageResult {
    private final byte[] stdout;
    private final byte[] stderr;
    private final int exitCode;

    public UsageResult(byte[] stdout, byte[] stderr, int exitCode) {
        this.stdout = stdout;
        this.stderr = stderr;
        this.exitCode = exitCode;
    }

    public byte[] stdout() {
        return stdout;
    }

    public byte[] stderr() {
        return stderr;
    }

    public String stdoutAsString() {
        return new String(stdout, StandardCharsets.UTF_8);
    }

    public String stderrAsString() {
        return new String(stderr, StandardCharsets.UTF_8);
    }

    public int exitCode() {
        return exitCode;
    }

    public boolean success() {
        return exitCode == 0;
    }
}
