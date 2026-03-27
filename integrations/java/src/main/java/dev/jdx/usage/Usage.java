package dev.jdx.usage;

import com.dylibso.chicory.annotations.WasmModuleInterface;
import com.dylibso.chicory.runtime.ImportValues;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.wasi.WasiExitException;
import com.dylibso.chicory.wasi.WasiOptions;
import com.dylibso.chicory.wasi.WasiPreview1;
import com.dylibso.chicory.wasm.WasmModule;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

@WasmModuleInterface(WasmResource.absoluteFile)
public final class Usage {
    private static final WasmModule MODULE = UsageModule.load();

    public static Builder builder() {
        return new Builder();
    }

    private static UsageResult exec(byte[] stdin, List<String> args, Path directory) {
        try (ByteArrayOutputStream stdout = new ByteArrayOutputStream();
                ByteArrayOutputStream stderr = new ByteArrayOutputStream()) {

            WasiOptions.Builder wasiOptsBuilder =
                    WasiOptions.builder()
                            .withStdout(stdout)
                            .withStderr(stderr)
                            .withStdin(
                                    new ByteArrayInputStream(
                                            stdin != null ? stdin : new byte[0]))
                            .withArguments(args);

            if (directory != null) {
                wasiOptsBuilder.withDirectory(directory.toString(), directory);
            }

            try (WasiPreview1 wasi =
                    WasiPreview1.builder().withOptions(wasiOptsBuilder.build()).build()) {
                Instance.builder(MODULE)
                        .withMachineFactory(UsageModule::create)
                        .withImportValues(
                                ImportValues.builder()
                                        .addFunction(wasi.toHostFunctions())
                                        .build())
                        .build();
            } catch (WasiExitException e) {
                return new UsageResult(stdout.toByteArray(), stderr.toByteArray(), e.exitCode());
            }

            return new UsageResult(stdout.toByteArray(), stderr.toByteArray(), 0);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    public static final class Builder {
        private byte[] stdin;
        private final List<String> args = new ArrayList<>();
        private Path directory;

        private Builder() {
            args.add("usage");
        }

        public Builder withStdin(byte[] stdin) {
            this.stdin = stdin;
            return this;
        }

        public Builder withStdin(String stdin) {
            this.stdin = stdin.getBytes(StandardCharsets.UTF_8);
            return this;
        }

        public Builder withArgs(String... args) {
            for (String arg : args) {
                this.args.add(arg);
            }
            return this;
        }

        public Builder withDirectory(Path directory) {
            this.directory = directory;
            return this;
        }

        public UsageResult run() {
            return exec(stdin, args, directory);
        }
    }
}
