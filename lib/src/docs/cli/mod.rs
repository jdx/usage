use crate::Spec;
use once_cell::sync::Lazy;
use tera::Tera;

pub fn render_help(spec: &Spec) -> String {
    let mut ctx = tera::Context::new();
    ctx.insert("spec", spec);
    TERA.render("spec_template.md.tera", &ctx)
        .unwrap()
        .trim()
        .to_string()
        + "\n"
}

static TERA: Lazy<Tera> = Lazy::new(|| {
    let mut tera = Tera::default();

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("spec_template.md.tera", include_str!("templates/spec_template.tera")),
    ]).unwrap();

    // tera.register_filter(
    //     "repeat",
    //     move |value: &tera::Value, args: &HashMap<String, tera::Value>| {
    //         let value = value.as_str().unwrap();
    //         let count = args.get("count").unwrap().as_u64().unwrap();
    //         Ok(value.repeat(count as usize).into())
    //     },
    // );

    tera
});
