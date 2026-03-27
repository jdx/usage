///usr/bin/env jbang "$0" "$@" ; exit $?
//DEPS dev.jdx:usage-chicory:0.1.0-SNAPSHOT
//DEPS io.roastedroot:zerofs:0.1.0

// Example demonstrating the usage-java integration.
//
// Run with jbang:
//   mise run wasm:build
//   cd integrations/java && mvn install
//   jbang example/Main.java
//
// See the test suite for more examples.

import dev.jdx.usage.Usage;
import dev.jdx.usage.UsageResult;
import io.roastedroot.zerofs.Configuration;
import io.roastedroot.zerofs.ZeroFs;
import java.nio.file.FileSystem;
import java.nio.file.Files;
import java.nio.file.Path;

public class Main {

    private static final String SPEC =
            "name \"deploy-tool\"\n"
                    + "bin \"deploy-tool\"\n"
                    + "version \"0.1.0\"\n"
                    + "about \"A deployment management tool\"\n"
                    + "flag \"-v --verbose\" help=\"Enable verbose output\" global=#true\n"
                    + "cmd \"deploy\" help=\"Deploy a service\" {\n"
                    + "    flag \"-e --env\" help=\"Target environment\" {\n"
                    + "        arg <ENV> default=\"staging\"\n"
                    + "    }\n"
                    + "    flag \"--force\" help=\"Force deployment without confirmation\"\n"
                    + "    flag \"--dry-run\" help=\"Show what would be deployed\"\n"
                    + "    arg <service> help=\"Service to deploy\"\n"
                    + "}\n"
                    + "cmd \"rollback\" help=\"Rollback a service to a previous version\" {\n"
                    + "    arg <service> help=\"Service to rollback\"\n"
                    + "    arg \"[version]\" help=\"Target version\" required=#false\n"
                    + "}\n"
                    + "cmd \"status\" help=\"Show deployment status\" {\n"
                    + "    alias \"st\" \"info\"\n"
                    + "    arg \"[service]\" help=\"Service to check\" required=#false\n"
                    + "}\n";

    public static void main(String[] args) throws Exception {
        // Generate JSON spec
        System.out.println("=== JSON spec ===");
        UsageResult jsonResult =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs("generate", "json", "-f", "-")
                        .run();
        System.out.println(jsonResult.stdoutAsString());

        // Generate bash completions
        System.out.println("=== Bash completions ===");
        UsageResult bashResult =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs(
                                "generate",
                                "completion",
                                "bash",
                                "deploy-tool",
                                "-f",
                                "-")
                        .run();
        System.out.println(bashResult.stdoutAsString());

        // Generate markdown documentation (requires a filesystem for --out-file)
        System.out.println("=== Markdown docs ===");
        try (FileSystem fs =
                ZeroFs.newFileSystem(
                        Configuration.unix().toBuilder().setAttributeViews("unix").build())) {
            Path outDir = fs.getPath("out");
            Files.createDirectory(outDir);
            Path outFile = outDir.resolve("docs.md");

            UsageResult mdResult =
                    Usage.builder()
                            .withStdin(SPEC)
                            .withArgs(
                                    "generate",
                                    "markdown",
                                    "-f",
                                    "-",
                                    "--out-file",
                                    outFile.toString())
                            .withDirectory(outDir)
                            .run();

            if (mdResult.success()) {
                System.out.println(new String(Files.readAllBytes(outFile)));
            } else {
                System.err.println(mdResult.stderrAsString());
            }
        }

        // Generate man page
        System.out.println("=== Man page ===");
        UsageResult manResult =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs("generate", "manpage", "-f", "-")
                        .run();
        System.out.println(manResult.stdoutAsString());
    }
}
