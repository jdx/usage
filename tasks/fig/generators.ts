export const envVarGenerator = {
  script: ['sh', '-c', 'env'],
  postProcess: (output: string) => {
    return output.split('\n').map(l => ({name: l.split('=')[0]}))
  }
}

export const singleCmdNewLineGenerator = (completion_cmd: string): Fig.Generator => ({
  script: completion_cmd.split(' '),
  splitOn: '\n'
})

export const singleCmdJsonGenerator = (cmd: string): Fig.Generator => ({
  script: cmd.split(' '),
  postProcess: (out) => (JSON.parse(out).map((r: any) => ({name: r.name, description: r.description})))
})

export const contextualGeneratorLastWord = (cmd: string): Fig.Generator => ({
  script: (context) => {
    if (context.length < 2) {
      return []
    }
    
    const prev = context[context.length - 2] // -1 is the current word
    return ['sh', '-c', [cmd, prev].join(' ')]
  }
})


export const usageGeneratorTemplate = (usage_cmd: string) : Fig.Generator => {
  return {
    custom: async (tokens: string[], executeCommand) => {
      const { stdout: spec } = await executeCommand({
        command: 'sh', args: ['-c', usage_cmd]
      });

      const { stdout: completes } = await executeCommand({
        command: 'usage', args: ['complete-word', '--shell', 'bash', '-s', spec]
      })

      return completes.split('\n').map(l => ({name: l.trim(), type: 'special'}))
      
    }
  }
}

export const completionGeneratorTemplate = (argSuggestionBash: string): Fig.Generator => {
  return {
    custom: async (tokens: string[], executeCommand) => {
      let arg = argSuggestionBash;
      if (tokens.length >= 1) {
        arg = argSuggestionBash.replace("{{words[CURRENT]}}", tokens[tokens.length - 1])
      }

      if (tokens.length >= 2) {
        arg = arg.replace(`{{words[PREV]}}`, tokens[tokens.length - 2])
      }
      const {stdout: text} = await executeCommand({
        command: 'sh', args: ['-c', arg]
      });
      if (text.trim().length == 0) return []
      return text.split("\n").map((elm) => ({ name: elm }));
    }
  }
}


// Dynamically generate fig specs on "mount run" commands
export const usageGenerateSpec = (cmds: string[]) => {
  return async (tokens: string[], executeCommand: Fig.ExecuteCommandFunction): Promise<Fig.Spec> => {
    const promises = cmds.map(async (cmd) => {
      try {
        const { stdout } = await executeCommand({
          command: 'sh', args: ['-c', cmd]
        });
        const { stdout: figSpecOut } = await executeCommand({
          command: 'usage', args: ['g', 'fig', '--spec', stdout, '--stdout']
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


export const pluginGenerator: Fig.Generator = singleCmdNewLineGenerator('mise plugins --core --user')
export const allPluginsGenerator: Fig.Generator = singleCmdNewLineGenerator('mise plugins --all')
export const simpleTaskGenerator = singleCmdJsonGenerator('mise tasks -J')
export const settingsGenerator = singleCmdNewLineGenerator(`mise settings --keys`)

export const aliasGenerator: Fig.Generator = {
  ...contextualGeneratorLastWord('mise alias ls'),
  postProcess: (out) => {
    //return [{name: out}]
    //return out.split('\t').map(l => ({name: l}))
    //return [{name: "test", "description": out}]
    const tokens = out.split(/\s+/)
    if (tokens.length == 0)
      return []

    return tokens.flatMap((_, i) => {
      if ((i % 3) == 0) {
        return [tokens[i+1]]
      }
      return []
    }).filter(l => l.trim().length > 0).map(l => ({name: l.trim()}))
  }
}

export const pluginWithAlias: Fig.Generator = {
  script: 'mise alias ls'.split(' '),
  postProcess: (output: string) => {
    const plugins = output.split('\n').map((line) => {
      const tokens = line.split(/\s+/)
      return tokens[0]
    })
    return [... new Set(plugins)].map((p) => ({name: p}))
  }
}
export const configPathGenerator: Fig.Generator = {
  ...singleCmdJsonGenerator('mise config ls -J'),
  postProcess: (out) => JSON.parse(out).map((r: any) => ({name: r.path, description: r.path}))
}