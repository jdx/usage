const envVarGenerator = {
  script: ["sh", "-c", "env"],
  postProcess: (output: string) => {
    return output.split("\n").map((l) => ({ name: l.split("=")[0] }));
  },
};

const usageGenerateSpec = (cmds: string[]) => {
  return async (
    context: string[],
    executeCommand: Fig.ExecuteCommandFunction,
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
  argSuggestionBash: string,
): Fig.Generator => {
  return {
    custom: async (tokens: string[], executeCommand) => {
      let arg = argSuggestionBash;
      if (tokens.length >= 1) {
        arg = argSuggestionBash.replace(
          "{{words[CURRENT]}}",
          tokens[tokens.length - 1],
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
