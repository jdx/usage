use std::path::PathBuf;
use std::vec;

use clap::Args;
use indexmap::IndexMap;
use itertools::Itertools;
use usage::{SpecArg, SpecCommand, SpecComplete, SpecFlag};

use crate::cli::generate;
use serde::{Deserialize, Serialize, Serializer};
use serde_with::{serde_as, OneOrMany};

fn is_false(value: &bool) -> bool {
    !*value
}

mod description_format {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(description: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: &str = description.as_ref().unwrap();
        let mut v: Vec<char> = s.chars().collect();
        v[0] = v[0].to_uppercase().next().unwrap();
        serializer.serialize_str(v.iter().collect::<String>().as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Some(s))
    }
}

#[derive(Args)]
#[clap()]
pub struct Fig {
    /// A usage spec taken in as a file
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// raw string spec input
    #[clap(long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,

    /// File on where to save the generated Fig spec
    #[clap(long, value_hint = clap::ValueHint::FilePath)]
    out_file: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
enum GeneratorType {
    EnvVar,
    Complete,
}

#[derive(Deserialize, Clone)]
struct FigGenerator {
    type_: GeneratorType,
    post_process: String,
    template_str: String,
}

impl Serialize for FigGenerator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.template_str)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FigArg {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "description_format")]
    description: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    is_optional: bool,
    #[serde(skip_serializing_if = "is_false")]
    is_variadic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generators: Option<FigGenerator>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggestions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debounce: Option<bool>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone)]
struct FigOption {
    #[serde_as(as = "OneOrMany<_>")]
    name: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "description_format")]
    description: Option<String>,
    #[serde(rename(serialize = "isRepeatable"))]
    is_repeatable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<FigArg>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FigCommand {
    #[serde_as(as = "OneOrMany<_>")]
    name: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    subcommands: Vec<FigCommand>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<FigOption>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "OneOrMany<_>")]
    args: Vec<FigArg>,

    #[serde(skip_serializing_if = "Option::is_none")]
    generate_spec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache: Option<bool>,
}

impl FigGenerator {
    pub fn create_simple_generator(type_: GeneratorType) -> Self {
        Self {
            type_: type_.clone(),
            template_str: FigGenerator::get_generator_name(type_).to_uppercase(),
            post_process: "".to_string(),
        }
    }

    fn get_generator_name(type_: GeneratorType) -> String {
        match type_.clone() {
            GeneratorType::EnvVar => "envVarGenerator".to_string(),
            GeneratorType::Complete => "completionGeneratorTemplate".to_string(),
        }
    }

    fn get_generator_arg(&self) -> String {
        match self.type_ {
            GeneratorType::Complete => {
                let postprocess = self.post_process.clone();
                format!("(`{postprocess}`)")
            }
            _ => "".to_string(),
        }
    }

    pub fn get_generator_text(&self) -> String {
        let generator_name = FigGenerator::get_generator_name(self.type_.clone());
        let arg = self.get_generator_arg();

        format!("{generator_name}{arg}")
    }
}

impl FigArg {
    fn get_template(name: &str) -> Option<String> {
        name.to_lowercase()
            .contains("file")
            .then(|| "filepaths".to_string())
            .or(name
                .to_lowercase()
                .contains("dir")
                .then(|| "folders".to_string()))
            .or(name
                .to_lowercase()
                .contains("path")
                .then(|| "filepaths".to_string()))
    }

    fn get_generator(name: &str) -> Option<FigGenerator> {
        name.to_lowercase()
            .contains("env_vars")
            .then(|| FigGenerator::create_simple_generator(GeneratorType::EnvVar))
            .or(name
                .to_lowercase()
                .contains("env_var")
                .then(|| FigGenerator::create_simple_generator(GeneratorType::EnvVar)))
    }

    pub fn get_generators(&self) -> Vec<FigGenerator> {
        match self.generators.clone() {
            Some(a) => vec![a],
            None => vec![],
        }
    }

    fn get_name(name: &str) -> String {
        name.replace("<", "")
            .replace(">", "")
            .replace("[", "")
            .replace("]", "")
            .to_ascii_lowercase()
    }

    pub fn parse_from_spec(arg: &SpecArg) -> Self {
        Self {
            name: FigArg::get_name(&arg.name),
            description: arg.help.clone(),
            is_variadic: arg.var,
            is_optional: !arg.required,
            template: FigArg::get_template(&arg.name),
            generators: FigArg::get_generator(&arg.name),
            suggestions: arg.choices.clone().map(|c| c.choices).unwrap_or_default(),
            debounce: FigArg::get_generator(&arg.name).map(|_| true),
        }
    }

    pub fn update_from_complete(&mut self, spec: SpecComplete) {
        let name = spec.name;

        self.generators = self.generators.clone().or_else(|| {
            Some(FigGenerator {
                type_: GeneratorType::Complete,
                post_process: spec.run.unwrap_or("".to_string()),
                template_str: format!("${name}$"),
            })
        })
    }
}

impl FigOption {
    fn get_names(flag: &SpecFlag) -> Vec<String> {
        let mut n: Vec<String> = flag.short.iter().map(|c| format!("-{c}")).collect();
        n.extend(flag.long.iter().map(|l| format!("--{l}")));
        n
    }

    pub fn get_generators(&self) -> Vec<FigGenerator> {
        self.args
            .iter()
            .filter(|&a| a.generators.is_some())
            .cloned()
            .map(|a| a.generators.unwrap())
            .collect()
    }

    pub fn get_args(&mut self) -> Vec<&mut FigArg> {
        self.args.as_mut().map(|a| vec![a]).unwrap_or_default()
    }

    pub fn parse_from_spec(flag: &SpecFlag) -> Self {
        Self {
            name: FigOption::get_names(flag),
            description: flag.help.clone(),
            is_repeatable: flag.var,
            args: flag.arg.clone().map(|arg| FigArg::parse_from_spec(&arg)),
        }
    }
}
impl FigCommand {
    fn get_names(cmd: &SpecCommand) -> Vec<String> {
        let mut r = vec![cmd.name.clone()];
        r.extend(cmd.aliases.clone());
        r
    }

    pub fn get_generators(&self) -> Vec<FigGenerator> {
        let sub = self
            .subcommands
            .iter()
            .map(|s| s.get_generators())
            .collect_vec()
            .concat();
        let opt = self
            .options
            .iter()
            .map(|o| o.get_generators())
            .collect_vec()
            .concat();
        let args = self
            .args
            .iter()
            .map(|a| a.get_generators())
            .collect_vec()
            .concat();
        [sub, opt, args].concat()
    }

    pub fn get_commands(&self) -> Vec<FigCommand> {
        let subcmds = self.subcommands.iter().map(|s| s.get_commands()).concat();
        [subcmds, vec![self.clone()]].concat()
    }

    pub fn get_args(&mut self) -> Vec<&mut FigArg> {
        let opt_args = self.options.iter_mut().map(|o| o.get_args()).concat();
        let sub_args = self.subcommands.iter_mut().map(|c| c.get_args()).concat();

        let args = self.args.iter_mut().collect_vec();
        let mut result = Vec::new();
        for vec in [opt_args, sub_args, args] {
            result.extend(vec);
        }
        result
    }

    pub fn parse_from_spec(cmd: &SpecCommand) -> Option<Self> {
        (!cmd.hide).then(|| Self {
            name: FigCommand::get_names(cmd),
            description: cmd.help.clone(),
            subcommands: cmd
                .subcommands
                .iter()
                .filter(|(_, v)| !v.hide)
                .filter_map(|(_, v)| FigCommand::parse_from_spec(v))
                .collect(),
            options: cmd
                .flags
                .iter()
                .filter(|f| !f.hide)
                .map(FigOption::parse_from_spec)
                .collect(),
            args: cmd
                .args
                .iter()
                .filter(|a| !a.hide)
                .map(FigArg::parse_from_spec)
                .collect(),
            generate_spec: (!cmd.mounts.is_empty()).then(|| {
                let calls = cmd
                    .mounts
                    .iter()
                    .cloned()
                    .map(|m| {
                        let run = m.run;
                        format!("\"{run}\"")
                    })
                    .join(",");
                format!("${calls}$")
            }),
            cache: (!cmd.mounts.is_empty()).then_some(false),
        })
    }
}

impl Fig {
    pub fn run(&self) -> miette::Result<()> {
        let write = |path: &PathBuf, md: &str| -> miette::Result<()> {
            println!("writing to {}", path.display());
            xx::file::write(path, format!("{}\n", md.trim()))?;
            Ok(())
        };
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let mut main_command = FigCommand::parse_from_spec(&spec.cmd).unwrap();
        let args = main_command.get_args();
        let completes = spec.complete;
        Fig::fill_args_complete(args, completes);
        let j = serde_json::to_string_pretty(&main_command).unwrap();
        let mut result = format!("const completionSpec: Fig.Spec = {j}");

        let generators = main_command.get_generators();
        generators.iter().cloned().for_each(|g| {
            let template_str = g.clone().template_str;
            let generator_call_text = g.get_generator_text();
            result = result.replace(
                format!("\"{template_str}\"").as_str(),
                generator_call_text.as_str(),
            )
        });

        // Handle mount run commands
        main_command
            .get_commands()
            .iter()
            .filter(|&cmd| cmd.generate_spec.is_some())
            .cloned()
            .for_each(|cmd| {
                let call_template_str = cmd.generate_spec.unwrap();
                let args = call_template_str.replace("$", "");
                let replace_str = call_template_str.replace("\"", "\\\"");
                result = result.replace(
                    format!("\"{replace_str}\"").as_str(),
                    format!("usageGenerateSpec([{args}])").as_str(),
                )
            });

        if let Some(path) = &self.out_file {
            result = [Fig::get_prescript(), result, Fig::get_postscript()].join("\n\n");
            write(path, result.as_str())?;
        } else {
            print!("{result}");
        }

        Ok(())
    }

    fn get_prescript() -> String {
        include_str!("../../../assets/fig/generators.ts").to_string()
    }

    fn get_postscript() -> String {
        "export default completionSpec;".to_string()
    }

    fn fill_args_complete(args: Vec<&mut FigArg>, completes: IndexMap<String, SpecComplete>) {
        let completable_args = args
            .into_iter()
            .map(|a| {
                let completekv = completes.get_key_value(&a.name);
                completekv.map(|(_, v)| (a, v.clone()))
            })
            .filter(Option::is_some);

        completable_args.for_each(|a| {
            let x = a.unwrap();
            x.0.update_from_complete(x.1)
        });
    }
}
