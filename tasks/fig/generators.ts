const usageGenerateSpec = (cmds: string[]) => {
  return async (tokens: string[], executeCommand: Fig.ExecuteCommandFunction): Promise<Fig.Spec> => {
    const promises = cmds.map(async (cmd) => {
      try {
        const { stdout } = await executeCommand({
          command: 'sh', args: ['-c', cmd]
        });
        const { stdout: figSpecOut } = await executeCommand({
          command: 'usage', args: ['g', 'fig', '--spec', stdout]
        })
        const start_of_json = figSpecOut.indexOf("{")
        const j = figSpecOut.slice(start_of_json)
        return JSON.parse(j).subcommands as Fig.Subcommand[]
      }
      catch (e){
        throw e;
      }
    })

    const subcommands = (await Promise.allSettled(promises)).filter(p => p.status === 'fulfilled').map(p => p.value);
    
    return { subcommands: subcommands.flat() } as Fig.Spec
  }
}

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
