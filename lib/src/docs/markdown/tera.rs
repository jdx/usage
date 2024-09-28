use once_cell::sync::Lazy;
use std::collections::HashMap;
use tera::Tera;

pub(crate) static TERA: Lazy<Tera> = Lazy::new(|| {
    let mut tera = Tera::default();

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("arg_template.md.tera", include_str!("templates/arg_template.md.tera")),
        ("cmd_template.md.tera", include_str!("templates/cmd_template.md.tera")),
        ("flag_template.md.tera", include_str!("templates/flag_template.md.tera")),
        ("spec_template.md.tera", include_str!("templates/spec_template.md.tera")),
        ("index_template.md.tera", include_str!("templates/index_template.md.tera")),
    ]).unwrap();

    tera.register_filter(
        "repeat",
        move |value: &tera::Value, args: &HashMap<String, tera::Value>| {
            let value = value.as_str().unwrap();
            let count = args.get("count").unwrap().as_u64().unwrap();
            Ok(value.repeat(count as usize).into())
        },
    );

    tera.register_filter(
        "escape_md",
        move |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            let value = value.as_str().unwrap();
            Ok(value.replace("<", "&lt;").replace(">", "&gt;").into())
        },
    );

    tera
});
