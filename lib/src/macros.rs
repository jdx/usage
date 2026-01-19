//! Convenience macros for creating specs with minimal boilerplate.
//!
//! # Examples
//!
//! ```
//! use usage::{spec_flag, spec_arg, spec_cmd};
//!
//! // Create a flag
//! let verbose = spec_flag!("-v", "--verbose");
//! let output = spec_flag!("--output" => "<file>"; help = "Output file");
//!
//! // Create an argument
//! let file = spec_arg!("file"; required = true);
//! let files = spec_arg!("files"; var = true, var_min = 1);
//!
//! // Create a command
//! let cmd = spec_cmd!("install";
//!     help = "Install packages",
//!     aliases = ["i", "add"]
//! );
//! ```

/// Create a SpecFlag with minimal boilerplate.
///
/// # Syntax
///
/// ```text
/// spec_flag!("-s", "--long")
/// spec_flag!("-s", "--long"; help = "description")
/// spec_flag!("--long")
/// spec_flag!("--long"; help = "description", var = true)
/// spec_flag!("--long" => "<arg>"; help = "description")
/// ```
///
/// # Examples
///
/// ```
/// use usage::spec_flag;
///
/// // Simple short and long flag
/// let f = spec_flag!("-v", "--verbose");
///
/// // Flag with help text
/// let f = spec_flag!("-f", "--force"; help = "Force operation");
///
/// // Long flag only
/// let f = spec_flag!("--all");
///
/// // Flag with an argument
/// let f = spec_flag!("--output" => "<file>"; help = "Output file");
/// ```
#[macro_export]
macro_rules! spec_flag {
    // Pattern: spec_flag!("-s", "--long")
    ($short:literal, $long:literal) => {{
        $crate::SpecFlagBuilder::new()
            .short($short.chars().nth(1).expect("short flag must be -X format"))
            .long(&$long[2..])
            .build()
    }};

    // Pattern: spec_flag!("-s", "--long"; key = value, ...)
    ($short:literal, $long:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let mut builder = $crate::SpecFlagBuilder::new()
            .short($short.chars().nth(1).expect("short flag must be -X format"))
            .long(&$long[2..]);
        $(builder = $crate::__spec_flag_attr!(builder, $key, $value);)*
        builder.build()
    }};

    // Pattern: spec_flag!("--long")
    ($long:literal) => {{
        $crate::SpecFlagBuilder::new()
            .long(&$long[2..])
            .build()
    }};

    // Pattern: spec_flag!("--long"; key = value, ...)
    ($long:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let mut builder = $crate::SpecFlagBuilder::new()
            .long(&$long[2..]);
        $(builder = $crate::__spec_flag_attr!(builder, $key, $value);)*
        builder.build()
    }};

    // Pattern: spec_flag!("--long" => "<arg>")
    ($long:literal => $arg:literal) => {{
        let arg: $crate::SpecArg = $arg.parse().expect("invalid arg format");
        $crate::SpecFlagBuilder::new()
            .long(&$long[2..])
            .arg(arg)
            .build()
    }};

    // Pattern: spec_flag!("--long" => "<arg>"; key = value, ...)
    ($long:literal => $arg:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let arg: $crate::SpecArg = $arg.parse().expect("invalid arg format");
        let mut builder = $crate::SpecFlagBuilder::new()
            .long(&$long[2..])
            .arg(arg);
        $(builder = $crate::__spec_flag_attr!(builder, $key, $value);)*
        builder.build()
    }};

    // Pattern: spec_flag!("-s", "--long" => "<arg>")
    ($short:literal, $long:literal => $arg:literal) => {{
        let arg: $crate::SpecArg = $arg.parse().expect("invalid arg format");
        $crate::SpecFlagBuilder::new()
            .short($short.chars().nth(1).expect("short flag must be -X format"))
            .long(&$long[2..])
            .arg(arg)
            .build()
    }};

    // Pattern: spec_flag!("-s", "--long" => "<arg>"; key = value, ...)
    ($short:literal, $long:literal => $arg:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let arg: $crate::SpecArg = $arg.parse().expect("invalid arg format");
        let mut builder = $crate::SpecFlagBuilder::new()
            .short($short.chars().nth(1).expect("short flag must be -X format"))
            .long(&$long[2..])
            .arg(arg);
        $(builder = $crate::__spec_flag_attr!(builder, $key, $value);)*
        builder.build()
    }};
}

/// Internal macro for setting flag attributes
#[macro_export]
#[doc(hidden)]
macro_rules! __spec_flag_attr {
    ($builder:expr, help, $value:expr) => {
        $builder.help($value)
    };
    ($builder:expr, help_long, $value:expr) => {
        $builder.help_long($value)
    };
    ($builder:expr, var, $value:expr) => {
        $builder.var($value)
    };
    ($builder:expr, var_min, $value:expr) => {
        $builder.var_min($value)
    };
    ($builder:expr, var_max, $value:expr) => {
        $builder.var_max($value)
    };
    ($builder:expr, required, $value:expr) => {
        $builder.required($value)
    };
    ($builder:expr, global, $value:expr) => {
        $builder.global($value)
    };
    ($builder:expr, hide, $value:expr) => {
        $builder.hide($value)
    };
    ($builder:expr, count, $value:expr) => {
        $builder.count($value)
    };
    ($builder:expr, env, $value:expr) => {
        $builder.env($value)
    };
}

/// Create a SpecArg with minimal boilerplate.
///
/// # Syntax
///
/// ```text
/// spec_arg!("name")
/// spec_arg!("name"; required = true)
/// spec_arg!("name"; var = true, var_min = 1, var_max = 10)
/// ```
///
/// # Examples
///
/// ```
/// use usage::spec_arg;
///
/// // Simple argument
/// let a = spec_arg!("file");
///
/// // Required argument
/// let a = spec_arg!("file"; required = true);
///
/// // Variadic argument with constraints
/// let a = spec_arg!("files"; var = true, var_min = 1, help = "Input files");
/// ```
#[macro_export]
macro_rules! spec_arg {
    // Pattern: spec_arg!("name")
    ($name:literal) => {{
        $crate::SpecArgBuilder::new()
            .name($name)
            .build()
    }};

    // Pattern: spec_arg!("name"; key = value, ...)
    ($name:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let mut builder = $crate::SpecArgBuilder::new()
            .name($name);
        $(builder = $crate::__spec_arg_attr!(builder, $key, $value);)*
        builder.build()
    }};
}

/// Internal macro for setting arg attributes
#[macro_export]
#[doc(hidden)]
macro_rules! __spec_arg_attr {
    ($builder:expr, help, $value:expr) => {
        $builder.help($value)
    };
    ($builder:expr, help_long, $value:expr) => {
        $builder.help_long($value)
    };
    ($builder:expr, var, $value:expr) => {
        $builder.var($value)
    };
    ($builder:expr, var_min, $value:expr) => {
        $builder.var_min($value)
    };
    ($builder:expr, var_max, $value:expr) => {
        $builder.var_max($value)
    };
    ($builder:expr, required, $value:expr) => {
        $builder.required($value)
    };
    ($builder:expr, hide, $value:expr) => {
        $builder.hide($value)
    };
    ($builder:expr, env, $value:expr) => {
        $builder.env($value)
    };
}

/// Create a SpecCommand with minimal boilerplate.
///
/// # Syntax
///
/// ```text
/// spec_cmd!("name")
/// spec_cmd!("name"; help = "description")
/// spec_cmd!("name"; help = "description", aliases = ["a", "b"])
/// ```
///
/// # Examples
///
/// ```
/// use usage::spec_cmd;
///
/// // Simple command
/// let c = spec_cmd!("install");
///
/// // Command with help
/// let c = spec_cmd!("install"; help = "Install packages");
///
/// // Command with aliases
/// let c = spec_cmd!("install"; help = "Install packages", aliases = ["i", "add"]);
/// ```
#[macro_export]
macro_rules! spec_cmd {
    // Pattern: spec_cmd!("name")
    ($name:literal) => {{
        $crate::SpecCommandBuilder::new()
            .name($name)
            .build()
    }};

    // Pattern: spec_cmd!("name"; key = value, ...)
    ($name:literal; $($key:ident = $value:expr),* $(,)?) => {{
        let mut builder = $crate::SpecCommandBuilder::new()
            .name($name);
        $(builder = $crate::__spec_cmd_attr!(builder, $key, $value);)*
        builder.build()
    }};
}

/// Internal macro for setting command attributes
#[macro_export]
#[doc(hidden)]
macro_rules! __spec_cmd_attr {
    ($builder:expr, help, $value:expr) => {
        $builder.help($value)
    };
    ($builder:expr, help_long, $value:expr) => {
        $builder.help_long($value)
    };
    ($builder:expr, hide, $value:expr) => {
        $builder.hide($value)
    };
    ($builder:expr, subcommand_required, $value:expr) => {
        $builder.subcommand_required($value)
    };
    ($builder:expr, aliases, $value:expr) => {
        $builder.aliases($value)
    };
    ($builder:expr, hidden_aliases, $value:expr) => {
        $builder.hidden_aliases($value)
    };
}

/// Create a `Vec<String>` from string literals.
///
/// # Examples
///
/// ```
/// use usage::defaults;
///
/// let values = defaults!["value1", "value2", "value3"];
/// assert_eq!(values, vec!["value1".to_string(), "value2".to_string(), "value3".to_string()]);
/// ```
#[macro_export]
macro_rules! defaults {
    [$($value:expr),* $(,)?] => {
        vec![$($value.to_string()),*]
    };
}

/// Create a `Vec<char>` for short flags.
///
/// # Examples
///
/// ```
/// use usage::shorts;
///
/// let chars = shorts!['v', 'V', 'd'];
/// assert_eq!(chars, vec!['v', 'V', 'd']);
/// ```
#[macro_export]
macro_rules! shorts {
    [$($char:literal),* $(,)?] => {
        vec![$($char),*]
    };
}

/// Create a `Vec<String>` for long flags.
///
/// # Examples
///
/// ```
/// use usage::longs;
///
/// let names = longs!["verbose", "debug"];
/// assert_eq!(names, vec!["verbose".to_string(), "debug".to_string()]);
/// ```
#[macro_export]
macro_rules! longs {
    [$($name:literal),* $(,)?] => {
        vec![$($name.to_string()),*]
    };
}

/// Create a `Vec<String>` for command aliases.
///
/// # Examples
///
/// ```
/// use usage::aliases;
///
/// let als = aliases!["i", "inst", "add"];
/// assert_eq!(als, vec!["i".to_string(), "inst".to_string(), "add".to_string()]);
/// ```
#[macro_export]
macro_rules! aliases {
    [$($name:literal),* $(,)?] => {
        vec![$($name.to_string()),*]
    };
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_spec_flag_simple() {
        let f = spec_flag!("-v", "--verbose");
        assert_eq!(f.short, vec!['v']);
        assert_eq!(f.long, vec!["verbose".to_string()]);
    }

    #[test]
    fn test_spec_flag_with_help() {
        let f = spec_flag!("-f", "--force"; help = "Force operation");
        assert_eq!(f.short, vec!['f']);
        assert_eq!(f.long, vec!["force".to_string()]);
        assert_eq!(f.help, Some("Force operation".to_string()));
    }

    #[test]
    fn test_spec_flag_long_only() {
        let f = spec_flag!("--all");
        assert!(f.short.is_empty());
        assert_eq!(f.long, vec!["all".to_string()]);
    }

    #[test]
    fn test_spec_flag_variadic() {
        let f = spec_flag!("--file"; var = true, var_min = 1, var_max = 10);
        assert!(f.var);
        assert_eq!(f.var_min, Some(1));
        assert_eq!(f.var_max, Some(10));
    }

    #[test]
    fn test_spec_flag_with_arg() {
        let f = spec_flag!("--output" => "<file>"; help = "Output file");
        assert!(f.arg.is_some());
        assert_eq!(f.arg.as_ref().unwrap().name, "file");
        assert_eq!(f.help, Some("Output file".to_string()));
    }

    #[test]
    fn test_spec_arg_simple() {
        let a = spec_arg!("file");
        assert_eq!(a.name, "file");
    }

    #[test]
    fn test_spec_arg_with_options() {
        let a = spec_arg!("files"; var = true, var_min = 1, help = "Input files");
        assert_eq!(a.name, "files");
        assert!(a.var);
        assert_eq!(a.var_min, Some(1));
        assert_eq!(a.help, Some("Input files".to_string()));
    }

    #[test]
    fn test_spec_cmd_simple() {
        let c = spec_cmd!("install");
        assert_eq!(c.name, "install");
    }

    #[test]
    fn test_spec_cmd_with_options() {
        let c = spec_cmd!("install"; help = "Install packages", aliases = ["i", "add"]);
        assert_eq!(c.name, "install");
        assert_eq!(c.help, Some("Install packages".to_string()));
        assert_eq!(c.aliases, vec!["i".to_string(), "add".to_string()]);
    }

    #[test]
    fn test_defaults_macro() {
        let d = defaults!["a", "b", "c"];
        assert_eq!(d, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_shorts_macro() {
        let s = shorts!['a', 'b', 'c'];
        assert_eq!(s, vec!['a', 'b', 'c']);
    }

    #[test]
    fn test_longs_macro() {
        let l = longs!["verbose", "debug"];
        assert_eq!(l, vec!["verbose".to_string(), "debug".to_string()]);
    }

    #[test]
    fn test_aliases_macro() {
        let a = aliases!["i", "inst"];
        assert_eq!(a, vec!["i".to_string(), "inst".to_string()]);
    }
}
