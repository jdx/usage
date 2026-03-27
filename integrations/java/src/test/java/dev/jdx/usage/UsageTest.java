package dev.jdx.usage;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.roastedroot.zerofs.Configuration;
import io.roastedroot.zerofs.ZeroFs;
import java.nio.file.FileSystem;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

class UsageTest {

    private static final String SPEC =
            "name \"test-cli\"\n"
                    + "bin \"test-cli\"\n"
                    + "version \"1.0.0\"\n"
                    + "flag \"-v --verbose\" help=\"Enable verbose output\" global=#true\n"
                    + "arg \"<input>\" help=\"Input file\"\n"
                    + "cmd \"sub\" help=\"A subcommand\" {\n"
                    + "    flag \"--force\" help=\"Force operation\"\n"
                    + "}\n";

    @Test
    void generateJson() {
        UsageResult result =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs("generate", "json", "-f", "-")
                        .run();

        assertTrue(result.success(), "stderr: " + result.stderrAsString());
        String output = result.stdoutAsString();
        assertTrue(output.contains("\"name\": \"test-cli\""));
        assertTrue(output.contains("\"version\": \"1.0.0\""));
    }

    @Test
    void generateManpage() {
        UsageResult result =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs("generate", "manpage", "-f", "-")
                        .run();

        assertTrue(result.success(), "stderr: " + result.stderrAsString());
        String output = result.stdoutAsString();
        assertTrue(output.contains("TEST-CLI"));
    }

    @Test
    void generateMarkdown() throws Exception {
        try (FileSystem fs =
                ZeroFs.newFileSystem(
                        Configuration.unix().toBuilder().setAttributeViews("unix").build())) {
            Path outDir = fs.getPath("out");
            Files.createDirectory(outDir);
            Path outFile = outDir.resolve("docs.md");

            UsageResult result =
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

            assertTrue(result.success(), "stderr: " + result.stderrAsString());
            String markdown = new String(Files.readAllBytes(outFile));
            assertTrue(markdown.contains("test-cli"));
        }
    }

    @Test
    void generateCompletionBash() {
        UsageResult result =
                Usage.builder()
                        .withStdin(SPEC)
                        .withArgs("generate", "completion", "bash", "test-cli", "-f", "-")
                        .run();

        assertTrue(result.success(), "stderr: " + result.stderrAsString());
        String output = result.stdoutAsString();
        assertTrue(output.contains("test-cli"));
    }

    @Test
    void invalidArgs() {
        UsageResult result = Usage.builder().withArgs("--nonexistent-flag").run();

        assertEquals(2, result.exitCode());
    }
}
