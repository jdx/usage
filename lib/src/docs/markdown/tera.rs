use itertools::Itertools;
use std::sync::LazyLock;
use tera::Tera;
use xx::regex;

pub(crate) static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();

    tera.register_filter(
        "repeat",
        move |value: &tera::Value,
              args: tera::Kwargs,
              _: &tera::State|
              -> tera::TeraResult<String> {
            let value = value.as_str().unwrap();
            let count = args.must_get::<u64>("count")?;
            Ok(value.repeat(count as usize))
        },
    );
    tera.register_filter(
        "default",
        |value: &tera::Value,
         kwargs: tera::Kwargs,
         _: &tera::State|
         -> tera::TeraResult<tera::Value> {
            let default_val = kwargs.must_get::<tera::Value>("value")?;
            let boolean = kwargs.get::<bool>("boolean")?.unwrap_or_default();
            if value.is_undefined() || value.is_none() || (boolean && !value.is_truthy()) {
                Ok(default_val)
            } else {
                Ok(value.clone())
            }
        },
    );
    tera.register_filter(
        "filter",
        |value: &[tera::Value],
         args: tera::Kwargs,
         _: &tera::State|
         -> tera::TeraResult<tera::Value> {
            let attr = args.must_get::<String>("attribute")?;
            let expected = args
                .get::<tera::Value>("value")?
                .unwrap_or_else(tera::Value::none);
            let values = value
                .iter()
                .filter(|item| {
                    let Some(actual) = item.get_from_path(&attr) else {
                        return false;
                    };
                    if expected.is_none() {
                        !actual.is_none()
                    } else {
                        actual == &expected
                    }
                })
                .cloned()
                .collect_vec();
            Ok(tera::Value::from(values))
        },
    );
    tera.register_filter(
        "slice",
        |value: &[tera::Value],
         args: tera::Kwargs,
         _: &tera::State|
         -> tera::TeraResult<tera::Value> {
            fn index(idx: i64, len: usize) -> usize {
                if idx < 0 {
                    len.saturating_sub(idx.unsigned_abs() as usize)
                } else {
                    idx as usize
                }
            }

            let start = args
                .get::<i64>("start")?
                .map(|idx| index(idx, value.len()))
                .unwrap_or_default();
            let end = args
                .get::<i64>("end")?
                .map(|idx| index(idx, value.len()))
                .unwrap_or(value.len())
                .min(value.len());
            if start >= end {
                return Ok(tera::Value::from(Vec::<tera::Value>::new()));
            }
            Ok(tera::Value::from(value[start..end].to_vec()))
        },
    );
    tera.register_filter(
        "concat",
        |value: &[tera::Value],
         args: tera::Kwargs,
         _: &tera::State|
         -> tera::TeraResult<tera::Value> {
            let extra = args.must_get::<tera::Value>("with")?;
            let mut values = value.to_vec();
            if let Some(extra_values) = extra.as_array() {
                values.extend(extra_values.iter().cloned());
            } else {
                values.push(extra);
            }
            Ok(tera::Value::from(values))
        },
    );
    tera.register_filter(
        "escape_md",
        |value: &tera::Value, _: tera::Kwargs, _: &tera::State| -> tera::TeraResult<tera::Value> {
            Ok(value.clone())
        },
    );
    let path_re = regex!(r"https://(github.com/[^/]+/[^/]+|gitlab.com/[^/]+/[^/]+/-)/blob/[^/]+/");
    tera.register_function(
        "source_code_link",
        move |args: tera::Kwargs, _: &tera::State| -> tera::TeraResult<String> {
            let spec = args.must_get::<&tera::Value>("spec")?;
            let cmd = args.must_get::<&tera::Value>("cmd")?;
            let full_cmd = cmd.get_from_path("full_cmd").and_then(|v| v.as_array());
            let source_code_link_template = spec
                .get_from_path("source_code_link_template")
                .and_then(|v| v.as_str());
            if let (Some(full_cmd), Some(source_code_link_template)) =
                (full_cmd, source_code_link_template)
            {
                if full_cmd.is_empty() {
                    return Ok(String::new());
                }
                let mut ctx = tera::Context::new();
                let path = full_cmd.iter().map(|v| v.as_str().unwrap()).join("/");
                ctx.insert_value("spec", spec.clone());
                ctx.insert_value("cmd", cmd.clone());
                ctx.insert("path", &path);
                let href = TERA
                    .clone()
                    .render_str(source_code_link_template, &ctx, false)?;
                let friendly = path_re.replace_all(&href, "").to_string();
                let link = if path_re.is_match(&href) {
                    format!("[`{friendly}`]({href})")
                } else {
                    format!("[{friendly}]({href})")
                };
                Ok(link)
            } else {
                Ok(String::new())
            }
        },
    );

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("arg_template.md.tera", include_str!("templates/arg_template.md.tera")),
        ("cmd_template.md.tera", include_str!("templates/cmd_template.md.tera")),
        ("flag_template.md.tera", include_str!("templates/flag_template.md.tera")),
        ("spec_template.md.tera", include_str!("templates/spec_template.md.tera")),
        ("index_template.md.tera", include_str!("templates/index_template.md.tera")),
    ]).unwrap();

    tera
});
