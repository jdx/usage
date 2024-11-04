const usageGeneratorTemplate = (usage_cmd: string): Fig.Generator => {
  return {
    custom: async (tokens: string[], executeCommand) => {
      const { stdout: spec } = await executeCommand({
        command: "sh",
        args: ["-c", usage_cmd],
      });

      const { stdout: completes } = await executeCommand({
        command: "usage",
        args: ["complete-word", "--shell", "bash", "-s", spec],
      });

      return completes
        .split("\n")
        .map((l) => ({ name: l.trim(), type: "special" }));
    },
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
