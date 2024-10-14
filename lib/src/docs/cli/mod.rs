use crate::{Spec, SpecCommand};
use once_cell::sync::Lazy;
use tera::Tera;

pub fn render_help(spec: &Spec, cmd: &SpecCommand, long: bool) -> String {
    let mut ctx = tera::Context::new();
    ctx.insert("spec", spec);
    ctx.insert("cmd", cmd);
    ctx.insert("long", &long);
    let template = if long {
        "spec_template_long.tera"
    } else {
        "spec_template_short.tera"
    };
    TERA.render(template, &ctx).unwrap().trim().to_string() + "\n"
}

static TERA: Lazy<Tera> = Lazy::new(|| {
    let mut tera = Tera::default();

    #[rustfmt::skip]
    tera.add_raw_templates([
        ("spec_template_short.tera", include_str!("templates/spec_template_short.tera")),
        ("spec_template_long.tera", include_str!("templates/spec_template_long.tera")),
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
