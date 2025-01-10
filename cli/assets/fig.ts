const envVarGenerator = {
  script: ["sh", "-c", "env"],
  postProcess: (output: string) => {
    return output.split("\n").map((l) => ({ name: l.split("=")[0] }));
  },
};

const usageGenerateSpec = (cmds: string[]) => {
  return async (
    context: string[],
    executeCommand: Fig.ExecuteCommandFunction
  ): Promise<Fig.Spec> => {
    const promises = cmds.map(async (cmd): Promise<Fig.Subcommand[]> => {
      try {
        const args = cmd.split(" ");
        const {
          stdout,
          stderr: cmdStderr,
          status: cmdStatus,
        } = await executeCommand({
          command: args[0],
          args: args.splice(1),
        });
        if (cmdStatus !== 0) {
          return [{ name: "error", description: cmdStderr }];
        }
        const {
          stdout: figSpecOut,
          stderr: figSpecStderr,
          status: usageFigStatus,
        } = await executeCommand({
          command: "usage",
          args: ["g", "fig", "--spec", stdout],
        });
        if (usageFigStatus !== 0) {
          return [{ name: "error", description: figSpecStderr }];
        }

        const start_of_json = figSpecOut.indexOf("{");
        const j = figSpecOut.slice(start_of_json);
        return JSON.parse(j).subcommands as Fig.Subcommand[];
      } catch (e) {
        return [{ name: "error", description: e }] as Fig.Subcommand[];
      }
    });

    // eslint-disable-next-line compat/compat
    const results = await Promise.allSettled(promises);
    const subcommands = results
      .filter((p) => p.status === "fulfilled")
      .map((p) => p.value);
    const failed = results
      .filter((p) => p.status === "rejected")
      .map((p) => ({ name: "error", description: p.reason }));

    return { subcommands: [...subcommands.flat(), ...failed] } as Fig.Spec;
  };
};

const completionGeneratorTemplate = (
  argSuggestionBash: string
): Fig.Generator => {
  return {
    custom: async (tokens: string[], executeCommand) => {
      let arg = argSuggestionBash;
      if (tokens.length >= 1) {
        arg = argSuggestionBash.replace(
          "{{words[CURRENT]}}",
          tokens[tokens.length - 1]
        );
      }

      if (tokens.length >= 2) {
        arg = arg.replace(`{{words[PREV]}}`, tokens[tokens.length - 2]);
      }
      const { stdout: text } = await executeCommand({
        command: "sh",
        args: ["-c", arg],
      });
      if (text.trim().length == 0) return [];
      return text.split("\n").map((elm) => ({ name: elm }));
    },
  };
};

const completionSpec: Fig.Spec = {
  name: "usage",
  subcommands: [
    {
      name: "bash",
      description: "Executes a bash script",
      options: [
        {
          name: "-h",
          description: "Show help",
          isRepeatable: false,
        },
        {
          name: "--help",
          description: "Show help",
          isRepeatable: false,
        },
      ],
      args: [
        {
          name: "script",
        },
        {
          name: "args",
          description: "Arguments to pass to script",
          isOptional: true,
          isVariadic: true,
        },
      ],
    },
    {
      name: ["complete-word", "cw"],
      options: [
        {
          name: "--shell",
          isRepeatable: false,
          args: {
            name: "shell",
            suggestions: ["bash", "fish", "zsh"],
          },
        },
        {
          name: ["-f", "--file"],
          description: "Usage spec file or script with usage shebang",
          isRepeatable: false,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-s", "--spec"],
          description: "Raw string spec input",
          isRepeatable: false,
          args: {
            name: "spec",
          },
        },
        {
          name: "--cword",
          description: "Current word index",
          isRepeatable: false,
          args: {
            name: "cword",
          },
        },
      ],
      args: {
        name: "words",
        description: "User's input from the command line",
        isOptional: true,
        isVariadic: true,
      },
    },
    {
      name: ["generate", "g"],
      subcommands: [
        {
          name: ["completion", "c"],
          options: [
            {
              name: "--cache-key",
              description:
                "A cache key to use for storing the results of calling the CLI with --usage-cmd",
              isRepeatable: false,
              args: {
                name: "cache_key",
              },
            },
            {
              name: ["-f", "--file"],
              description:
                "A .usage.kdl spec file to use for generating completions",
              isRepeatable: false,
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: "--usage-bin",
              description:
                "Override the bin used for calling back to usage-cli",
              isRepeatable: false,
              args: {
                name: "usage_bin",
              },
            },
            {
              name: "--usage-cmd",
              description:
                'A command which generates a usage spec e.g.: `mycli --usage` or `mycli completion usage` Defaults to "$bin --usage"',
              isRepeatable: false,
              args: {
                name: "usage_cmd",
              },
            },
            {
              name: "--include-bash-completion-lib",
              description: "Include https://github.com/scop/bash-completion",
              isRepeatable: false,
            },
          ],
          args: [
            {
              name: "shell",
              suggestions: ["bash", "fish", "zsh"],
            },
            {
              name: "bin",
              description: "The CLI which we're generates completions for",
            },
          ],
        },
        {
          name: "fig",
          options: [
            {
              name: ["-f", "--file"],
              description: "A usage spec taken in as a file",
              isRepeatable: false,
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: "--spec",
              description: "Raw string spec input",
              isRepeatable: false,
              args: {
                name: "spec",
              },
            },
            {
              name: "--out-file",
              description: "File on where to save the generated Fig spec",
              isRepeatable: false,
              args: {
                name: "out_file",
                template: "filepaths",
              },
            },
          ],
        },
        {
          name: "json",
          description: "Outputs a usage spec in json format",
          options: [
            {
              name: ["-f", "--file"],
              description: "A usage spec taken in as a file",
              isRepeatable: false,
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: "--spec",
              description: "Raw string spec input",
              isRepeatable: false,
              args: {
                name: "spec",
              },
            },
          ],
        },
        {
          name: ["markdown", "md"],
          options: [
            {
              name: ["-f", "--file"],
              description: "A usage spec taken in as a file",
              isRepeatable: false,
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: ["-m", "--multi"],
              description: "Render each subcommand as a separate markdown file",
              isRepeatable: false,
            },
            {
              name: "--url-prefix",
              description: "Prefix to add to all URLs",
              isRepeatable: false,
              args: {
                name: "url_prefix",
              },
            },
            {
              name: "--html-encode",
              description: "Escape HTML in markdown",
              isRepeatable: false,
            },
            {
              name: "--replace-pre-with-code-fences",
              isRepeatable: false,
            },
            {
              name: "--out-dir",
              description: "Output markdown files to this directory",
              isRepeatable: false,
              args: {
                name: "out_dir",
                template: "folders",
              },
            },
            {
              name: "--out-file",
              isRepeatable: false,
              args: {
                name: "out_file",
                template: "filepaths",
              },
            },
          ],
        },
      ],
    },
  ],
  options: [
    {
      name: "--usage-spec",
      description: "Outputs a `usage.kdl` spec for this CLI itself",
      isRepeatable: false,
    },
  ],
  args: {
    name: "completions",
    description:
      "Outputs completions for the specified shell for completing the `usage` CLI itself",
    isOptional: true,
  },
};

export default completionSpec;
