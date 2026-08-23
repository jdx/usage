//! Record and replay a CLI's black-box behavior during a migration.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

const FORMAT_VERSION: u32 = 1;

/// One invocation whose observable behavior belongs to a CLI's compatibility contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub name: String,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_remove: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(with = "bytes")]
    pub stdin: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl Case {
    pub fn new<I, S>(name: impl Into<String>, argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            argv: argv.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
            env_remove: Vec::new(),
            stdin: Vec::new(),
            cwd: None,
        }
    }

    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    pub fn env_remove(mut self, name: impl Into<String>) -> Self {
        self.env_remove.push(name.into());
        self
    }

    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = bytes.into();
        self
    }

    pub fn cwd(mut self, path: impl Into<String>) -> Self {
        self.cwd = Some(path.into());
        self
    }
}

/// The exact process result recorded for one case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub success: bool,
    pub code: Option<i32>,
    #[serde(with = "bytes")]
    pub stdout: Vec<u8>,
    #[serde(with = "bytes")]
    pub stderr: Vec<u8>,
}

/// One named input and the result produced by the reference binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedCase {
    #[serde(flatten)]
    pub input: Case,
    pub expected: Observation,
}

/// A versioned, portable set of black-box CLI expectations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub format_version: u32,
    pub cases: Vec<RecordedCase>,
}

impl Contract {
    /// Run every case against a reference binary and capture its observable contract.
    pub fn record<I>(program: impl AsRef<Path>, cases: I) -> std::io::Result<Self>
    where
        I: IntoIterator<Item = Case>,
    {
        let program = program.as_ref();
        let cases = cases
            .into_iter()
            .map(|input| {
                let expected = run(program, &input)?;
                Ok(RecordedCase { input, expected })
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(Self {
            format_version: FORMAT_VERSION,
            cases,
        })
    }

    /// Write a stable, reviewable JSON fixture.
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(invalid_data)?;
        bytes.push(b'\n');
        std::fs::write(path, bytes)
    }

    /// Read a fixture and reject formats this version does not understand.
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let contract: Self = serde_json::from_slice(&std::fs::read(path)?).map_err(invalid_data)?;
        if contract.format_version != FORMAT_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unsupported CLI contract format {}; expected {FORMAT_VERSION}",
                    contract.format_version
                ),
            ));
        }
        Ok(contract)
    }

    /// Run the recorded inputs against another binary and return every mismatch.
    pub fn replay(&self, program: impl AsRef<Path>) -> std::io::Result<Replay> {
        let program = program.as_ref();
        let mut mismatches = Vec::new();
        for recorded in &self.cases {
            let actual = run(program, &recorded.input)?;
            if actual != recorded.expected {
                mismatches.push(Mismatch {
                    name: recorded.input.name.clone(),
                    expected: recorded.expected.clone(),
                    actual,
                });
            }
        }
        Ok(Replay { mismatches })
    }
}

/// The result of replaying a contract against a candidate binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub mismatches: Vec<Mismatch>,
}

impl Replay {
    pub fn is_match(&self) -> bool {
        self.mismatches.is_empty()
    }

    /// Assert every case matched, with a compact text diff of changed process surfaces.
    #[track_caller]
    pub fn assert_match(self) {
        assert!(self.is_match(), "{self}");
    }
}

impl fmt::Display for Replay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_match() {
            return f.write_str("all CLI contract cases matched");
        }
        writeln!(f, "{} CLI contract case(s) changed:", self.mismatches.len())?;
        for mismatch in &self.mismatches {
            writeln!(f, "- {}", mismatch.name)?;
            if (mismatch.expected.success, mismatch.expected.code)
                != (mismatch.actual.success, mismatch.actual.code)
            {
                writeln!(
                    f,
                    "  status: expected {:?}, got {:?}",
                    mismatch.expected.code, mismatch.actual.code
                )?;
            }
            if mismatch.expected.stdout != mismatch.actual.stdout {
                writeln!(
                    f,
                    "  stdout:\n    expected: {}\n    actual:   {}",
                    Stream(&mismatch.expected.stdout),
                    Stream(&mismatch.actual.stdout)
                )?;
            }
            if mismatch.expected.stderr != mismatch.actual.stderr {
                writeln!(
                    f,
                    "  stderr:\n    expected: {}\n    actual:   {}",
                    Stream(&mismatch.expected.stderr),
                    Stream(&mismatch.actual.stderr)
                )?;
            }
        }
        Ok(())
    }
}

struct Stream<'a>(&'a [u8]);

impl fmt::Display for Stream<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(self.0) {
            Ok(text) => write!(f, "{text:?}"),
            Err(_) => write!(f, "{:02x?}", self.0),
        }
    }
}

/// One case whose status or output changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub name: String,
    pub expected: Observation,
    pub actual: Observation,
}

fn run(program: &Path, case: &Case) -> std::io::Result<Observation> {
    let mut command = Command::new(program);
    command.args(&case.argv).envs(&case.env);
    for name in &case.env_remove {
        command.env_remove(name);
    }
    if let Some(cwd) = &case.cwd {
        command.current_dir(cwd);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let input = case.stdin.clone();
    let stdin_writer = child
        .stdin
        .take()
        .map(|mut stdin| {
            std::thread::Builder::new()
                .name("usage-contract-stdin".into())
                .spawn(move || stdin.write_all(&input))
        })
        .transpose()?;
    let output = child.wait_with_output()?;
    if let Some(stdin_writer) = stdin_writer {
        let result = stdin_writer
            .join()
            .map_err(|_| std::io::Error::other("CLI contract stdin writer panicked"))?;
        if let Err(error) = result {
            if error.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(error);
            }
        }
    }
    Ok(Observation {
        success: output.status.success(),
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
}

mod bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match std::str::from_utf8(bytes) {
            Ok(text) => serializer.serialize_str(text),
            Err(_) => bytes.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TextOrBytes {
            Text(String),
            Bytes(Vec<u8>),
        }
        Ok(match TextOrBytes::deserialize(deserializer)? {
            TextOrBytes::Text(text) => text.into_bytes(),
            TextOrBytes::Bytes(bytes) => bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_contract_round_trips_and_replays() {
        let program = std::env::current_exe().unwrap();
        let contract = Contract::record(&program, [Case::new("help", ["--help"])]).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("contract.json");
        contract.save(&path).unwrap();
        let fixture = std::fs::read_to_string(&path).unwrap();
        assert!(fixture.contains(r#""stdout": "#), "{fixture}");
        let loaded = Contract::load(path).unwrap();
        assert_eq!(loaded, contract);
        loaded.replay(program).unwrap().assert_match();
    }

    #[test]
    fn replay_reports_the_surface_that_changed() {
        let program = std::env::current_exe().unwrap();
        let mut contract = Contract::record(&program, [Case::new("help", ["--help"])]).unwrap();
        contract.cases[0].expected.stdout = b"old help\n".to_vec();
        let replay = contract.replay(program).unwrap();
        assert_eq!(replay.mismatches.len(), 1);
        let rendered = replay.to_string();
        assert!(rendered.contains("help"), "{rendered}");
        assert!(rendered.contains("stdout"), "{rendered}");
    }

    #[test]
    fn a_program_may_exit_without_consuming_stdin() {
        let program = std::env::current_exe().unwrap();
        let contract = Contract::record(
            program,
            [Case::new("help", ["--help"]).stdin(vec![b'x'; 1024 * 1024])],
        )
        .unwrap();
        assert_eq!(contract.cases.len(), 1);
    }

    #[test]
    fn binary_stream_diffs_show_the_exact_bytes() {
        let observation = |stdout| Observation {
            success: true,
            code: Some(0),
            stdout,
            stderr: Vec::new(),
        };
        let replay = Replay {
            mismatches: vec![Mismatch {
                name: "binary".into(),
                expected: observation(vec![0xff]),
                actual: observation(vec![0xfe]),
            }],
        };

        let rendered = replay.to_string();
        assert!(rendered.contains("[ff]"), "{rendered}");
        assert!(rendered.contains("[fe]"), "{rendered}");
    }
}
