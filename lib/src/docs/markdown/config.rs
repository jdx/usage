use crate::docs::markdown::renderer::MarkdownRenderer;
use crate::docs::models::SpecConfig;
use crate::error::UsageErr;

impl MarkdownRenderer {
    /// The settings reference for a spec's `config` block.
    ///
    /// Empty string when there is nothing to say, so a caller can concatenate it without
    /// checking — a CLI with no settings should not grow a blank section.
    pub fn render_config(&self) -> Result<String, UsageErr> {
        // From the raw block, not from `self.spec.config`: that one was rendered by `new`
        // before the builder options were set, and rendering is once-only, so taking it would
        // silently ignore `replace_pre_with_code_fences`. Same reason `render_cmd` converts
        // from the raw command it is given.
        let mut config = SpecConfig::from(&self.raw_config);
        if config.is_empty() {
            return Ok(String::new());
        }
        config.render_md(self);
        let mut ctx = self.clone();
        ctx.insert("config", &config);
        ctx.render("config_template.md.tera")
    }
}

#[cfg(test)]
mod tests {
    use crate::docs::markdown::renderer::MarkdownRenderer;
    use insta::assert_snapshot;

    fn rendered(src: &str) -> String {
        let spec: crate::Spec = src.parse().unwrap();
        MarkdownRenderer::new(spec)
            .with_replace_pre_with_code_fences(true)
            .render_config()
            .unwrap()
    }

    #[test]
    fn a_cli_with_no_settings_renders_nothing() {
        assert_eq!(rendered("name \"ex\"\nbin \"ex\"\n"), "");
    }

    #[test]
    fn the_facts_list_starts_on_its_own_line_whatever_its_first_item_is() {
        // The blank line that opens the list was emitted inside the `type_` branch, so a prop
        // with a default and no declared type put its first list item straight against the
        // heading — and a deprecated one put it against the admonition's closing `:::`, where
        // some renderers read it as part of the admonition rather than as a list.
        let page = rendered(
            r##"
name "ex"
bin "ex"
config {
    prop "untyped" default=1 help="No declared type"
    prop "gone" default=2 deprecated="Use untyped." help="Also no declared type"
}
"##,
        );
        assert!(
            page.contains("## `untyped`\n\n- **default**: `1`"),
            "the list item is against the heading:\n{page:?}"
        );
        assert!(
            page.contains(":::\n\n- **default**: `2`"),
            "the list item is against the admonition:\n{page:?}"
        );
    }

    #[test]
    fn the_settings_section_sits_under_the_title_on_the_single_file_page() {
        // Two things were wrong here at once, and the second hid the first: `{%- include %}`
        // stripped the blank line before the section, so its heading was glued onto the last
        // line of the command above it — `Run# Configuration` — and a `header_level` decrement
        // put that heading at level 1, beside the document's own title.
        let spec: crate::Spec = r##"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint" help="How many"
}
cmd "run" help="Run"
"##
        .parse()
        .unwrap();
        let page = MarkdownRenderer::new(spec).render_spec().unwrap();
        assert!(
            page.contains("\n\n## Configuration\n"),
            "the heading is glued to the line above it, or at the wrong level:\n{page}"
        );
        // And the settings sit below it, a level deeper than the commands' own level.
        assert!(page.contains("### `jobs`"), "{page}");
    }

    #[test]
    fn a_setting_s_long_help_gets_the_rendering_options_it_was_asked_for() {
        // `MarkdownRenderer::new` renders the whole docs model eagerly — before the builder
        // methods that set the options — and rendering marks each item done, so a second pass
        // no-ops. Taking the already-rendered config meant `replace_pre_with_code_fences` did
        // nothing at all to a setting's long help, silently, while doing its job everywhere
        // else on the same page.
        let src = r##"
name "ex"
bin "ex"
config {
    prop "shell" help="Which shell" {
        long_help "Run it like this:\n\n    ex --shell bash\n"
    }
}
"##;
        let spec: crate::Spec = src.parse().unwrap();
        let with_fences = MarkdownRenderer::new(spec.clone())
            .with_replace_pre_with_code_fences(true)
            .render_config()
            .unwrap();
        assert!(
            with_fences.contains("```"),
            "the option did not reach the setting's help:\n{with_fences}"
        );
        // And without it the block stays indented, so the assertion above is about the option
        // rather than about something else in the pipeline.
        let without = MarkdownRenderer::new(spec.clone()).render_config().unwrap();
        assert!(!without.contains("```"), "{without}");
        assert!(without.contains("    ex --shell bash"), "{without}");

        // The single-file page renders the same model by its own path, and had the same bug.
        let whole = MarkdownRenderer::new(spec)
            .with_replace_pre_with_code_fences(true)
            .render_spec()
            .unwrap();
        assert!(
            whole.contains("```"),
            "the option did not reach the settings section of the whole-spec page:\n{whole}"
        );
    }

    #[test]
    fn the_index_links_the_settings_page_beside_it() {
        // `--multi` writes settings.md next to index.md, and a reader who starts at the
        // index — which is what an index is for — has to be able to get there.
        let with_settings: crate::Spec = r##"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint"
}
cmd "run" help="Run"
"##
        .parse()
        .unwrap();
        let renderer = MarkdownRenderer::new(with_settings);
        let index = renderer.render_index().unwrap();
        assert!(
            index.contains(&format!("[Settings](/{})", renderer.config_page())),
            "{index}"
        );

        // The page name does not move when a `settings` command appears — mise has exactly that
        // command, and `settings.md` would have been its page. A name that switched would leave
        // the abandoned one behind as a stale page on the next run.
        let collides: crate::Spec = r##"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint"
}
cmd "settings" help="Manage settings"
"##
        .parse()
        .unwrap();
        let renderer = MarkdownRenderer::new(collides);
        assert_eq!(renderer.config_page(), "configuration.md");
        assert_eq!(renderer.config_page_collision(), None);

        // And the one collision left is reported rather than silent.
        let clash: crate::Spec = r##"
name "ex"
bin "ex"
config {
    prop "jobs" type="uint"
}
cmd "configuration" help="Somebody really did this"
"##
        .parse()
        .unwrap();
        assert_eq!(
            MarkdownRenderer::new(clash).config_page_collision(),
            Some("configuration")
        );

        // And a CLI with no settings gets no link, because there is no page: the two are
        // gated on the same condition so the index cannot point at a file nothing wrote.
        let without: crate::Spec = "name \"ex\"\nbin \"ex\"\ncmd \"run\" help=\"Run\"\n"
            .parse()
            .unwrap();
        let renderer = MarkdownRenderer::new(without);
        let index = renderer.render_index().unwrap();
        // Against the name the page is actually written under, not a name nothing uses any
        // more: asserting the absence of `settings.md` passed happily while a broken gate
        // emitted a link to `configuration.md`.
        assert!(
            !index.contains(&format!("[Settings](/{}", renderer.config_page())),
            "{index}"
        );
    }

    #[test]
    fn a_block_of_only_files_reaches_every_output() {
        // Where the config files live is the part a reader cannot guess, and a CLI may
        // describe the chain before it declares its first setting. Three output paths render
        // this model — its own page, the single-file page, and the manpage — and each had its
        // own idea of when there was something to render: two gated on props, so the same
        // spec documented its files in one place and not the others.
        let src = r##"
name "ex"
bin "ex"
config {
    file "/etc/ex/config.toml" scope="system"
    file "ex.toml" findup=#true
}
"##;
        let spec: crate::Spec = src.parse().unwrap();
        let renderer = MarkdownRenderer::new(spec);
        let page = renderer.render_config().unwrap();
        assert!(page.contains("ex.toml"), "{page}");
        let whole = renderer.render_spec().unwrap();
        assert!(
            whole.contains("ex.toml"),
            "the single-file page dropped the file chain:\n{whole}"
        );
    }

    #[test]
    fn every_part_of_a_prop_reaches_the_page() {
        assert_snapshot!(rendered(
            r##"
name "hk"
bin "hk"
config {
    source "git" name="git config" doc_hint="git config `{key}`"
    file "~/.config/hk/config.toml" scope="global"
    file "hk.toml" findup=#true
    prop "jobs" type="uint" default=0 default_note="0 = auto-detect" \
        help="Number of parallel jobs" since="1.0.0" help_heading="Performance" {
        cli "--jobs" "-j"
        env "HK_JOBS" "HK_JOB"
        source "git" "hk.jobs"
        example "hk check --jobs 4"
    }
    prop "exclude" type="list<string>" merge="union" help="Patterns to skip" {
        default "target" "node_modules"
        env "HK_EXCLUDE"
    }
    prop "stash" type="string" help="How to stash" {
        choices {
            choice "git" help="Use `git stash`"
            choice "none" help="No stashing"
        }
    }
    prop "trusted" type="bool" scope="global" help="Trust the config"
    prop "old" deprecated="Use jobs instead." deprecated_remove_at="2027.12.0" help="Old"
    prop "secret" hide=#true help="Not for the page"
}
"##
        ));
    }
}
