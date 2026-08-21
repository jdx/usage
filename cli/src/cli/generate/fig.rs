use std::path::PathBuf;
use std::vec;

use indexmap::IndexMap;
use itertools::Itertools;
use usage::{SpecArg, SpecCommand, SpecComplete, SpecFlag};
use usage_rs::Args;

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
        let s: &str = match description.as_ref() {
            Some(s) => s,
            None => return serializer.serialize_str(""),
        };
        if s.is_empty() {
            return serializer.serialize_str("");
        }
        let mut v: Vec<char> = s.chars().collect();
        if let Some(first_upper) = v[0].to_uppercase().next() {
            v[0] = first_upper;
        }
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

/// Generate Fig completion spec for Amazon Q / Fig
#[derive(Args)]
#[usage(effect = "read")]
pub struct Fig {
    /// A usage spec taken in as a file, use "-" to read from stdin
    #[usage(short, long)]
    file: Option<PathBuf>,

    /// File path where the generated Fig spec will be saved, or "-" for stdout
    #[usage(
        long,
        value_hint = usage_rs::ValueHint::FilePath,
        effect = "write"
    )]
    out_file: Option<PathBuf>,

    /// Raw string spec input
    #[usage(long, required_unless = "--file", overrides = "--file")]
    spec: Option<String>,
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
    /// Whether [`Self::template`] came from a `complete … type=` declaration rather than
    /// from the name guess `get_template` makes. Not part of a Fig spec — it only decides
    /// which of several declarations wins while one is being built.
    #[serde(skip)]
    typed_template: bool,
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

/// The Fig template for a portable `complete … type=`, where there is one.
///
/// `executable` is filesystem completion in Fig's vocabulary too; the narrower kinds
/// (`command`, `user`, `host`, `none`) have no template and are left alone rather than
/// approximated with the wrong one.
fn template_for_type(type_: &str) -> Option<&'static str> {
    match type_ {
        "path" | "file" | "executable" => Some("filepaths"),
        "dir" => Some("folders"),
        _ => None,
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
            typed_template: false,
        }
    }

    pub fn update_from_complete(&mut self, spec: SpecComplete) {
        let name = spec.name;

        // Nearest declaration wins, which is the rule the generator already followed:
        // `or_else` kept whatever a command's own completer had put there when the root
        // spec's completers were applied over the whole tree afterwards. The template
        // obeys the same rule, so an argument cannot come out carrying both a template
        // and a generator for a reader to have to choose between. A name-inferred
        // template is a guess rather than a declaration, so it does not count as one.
        if self.typed_template || self.generators.is_some() {
            return;
        }

        // A typed completer is a policy Fig has its own name for, so it becomes a
        // template rather than a generator. `get_template` above guesses from the
        // argument's name — `out_file` gets paths because it contains "file" — which was
        // all there was before `type=` existed.
        if let Some(template) = spec.type_.as_deref().and_then(template_for_type) {
            self.template = Some(template.to_string());
            self.typed_template = true;
            // Fig answers a template itself: nothing runs, so there is nothing to
            // debounce.
            self.debounce = None;
            return;
        }

        // Only a completer with something to run becomes a generator. The rest — `none`,
        // and the kinds a shell answers for itself — have no command to post-process, and
        // building one anyway emitted a generator whose script was the empty string.
        let Some(run) = spec.run else {
            return;
        };
        self.generators = Some(FigGenerator {
            type_: GeneratorType::Complete,
            post_process: run,
            template_str: format!("${name}$"),
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
            .filter_map(|a| a.generators.clone())
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

    /// This command's own arguments — its positionals and its flags' values — without
    /// descending into subcommands.
    ///
    /// What a `complete` node inside a `cmd` block is about. [`Self::get_args`] gathers
    /// the whole subtree, which is right for the root spec's completers because those are
    /// inherited, and wrong for a command's own.
    pub fn get_own_args(&mut self) -> Vec<&mut FigArg> {
        let mut own: Vec<&mut FigArg> = self.options.iter_mut().map(|o| o.get_args()).concat();
        own.extend(self.args.iter_mut());
        own
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
        let mut command = Self::parse_declarations(cmd)?;
        // Each command's own completers, at its own level. Only the root spec's `complete`
        // nodes used to reach any argument, so a `complete` inside a `cmd` block — which
        // is where a typed value hint on a subcommand's argument lands — was dropped.
        Fig::fill_args_complete(command.get_own_args(), cmd.complete.clone());
        Some(command)
    }

    fn parse_declarations(cmd: &SpecCommand) -> Option<Self> {
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
    fn get_prescript() -> String {
        format!(
            "// @generated by usage-cli from usage spec\n{}",
            include_str!("../../../assets/fig/generators.ts")
        )
    }

    fn get_postscript() -> String {
        "export default completionSpec;".to_string()
    }

    fn fill_args_complete(args: Vec<&mut FigArg>, completes: IndexMap<String, SpecComplete>) {
        args.into_iter()
            .filter_map(|a| completes.get(&a.name).map(|v| (a, v.clone())))
            .for_each(|(arg, complete)| arg.update_from_complete(complete));
    }
}

impl usage_rs::Run for Fig {
    type Output = miette::Result<()>;

    fn run(self) -> Self::Output {
        let write = |path: &PathBuf, md: &str| -> miette::Result<()> {
            generate::write_or_stdout(Some(path), &format!("{}\n", md.trim()))?;
            Ok(())
        };
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let mut main_command = FigCommand::parse_from_spec(&spec.cmd).ok_or_else(|| {
            miette::miette!("Failed to parse command spec (command may be hidden)")
        })?;
        let args = main_command.get_args();
        let completes = spec.complete;
        Fig::fill_args_complete(args, completes);
        let j = serde_json::to_string_pretty(&main_command)
            .map_err(|e| miette::miette!("Failed to serialize Fig spec: {}", e))?;
        let mut result = format!("const completionSpec: Fig.Spec = {j}");

        let generators = main_command.get_generators();
        generators.iter().for_each(|g| {
            let template_str = g.template_str.clone();
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
            .filter_map(|cmd| cmd.generate_spec.as_ref().map(|spec| (cmd, spec)))
            .for_each(|(_, call_template_str)| {
                let args = call_template_str.replace("$", "");
                let replace_str = call_template_str.replace("\"", "\\\"");
                result = result.replace(
                    format!("\"{replace_str}\"").as_str(),
                    format!("usageGenerateSpec([{args}])").as_str(),
                )
            });

        // Only the file path wraps the spec in the prescript/postscript that make it usable
        // on its own; bare `usage g fig` prints the spec object alone. `--out-file -` follows
        // the file path, since it means "the bytes a file would have received". Whether the
        // two should agree is a separate question, left alone here.
        if let Some(path) = &self.out_file {
            let prescript = if let Some(source_file) = &self.file {
                let source_label = if source_file.as_os_str() == "-" {
                    "stdin".to_string()
                } else {
                    source_file.display().to_string()
                };
                format!(
                    "// @generated by usage-cli from {}\n{}",
                    source_label,
                    include_str!("../../../assets/fig/generators.ts")
                )
            } else {
                Fig::get_prescript()
            };
            result = [prescript, result, Fig::get_postscript()].join("\n\n");
            write(path, result.as_str())?;
        } else {
            print!("{result}");
        }

        Ok(())
    }
}
