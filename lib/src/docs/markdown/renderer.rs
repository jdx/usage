use crate::docs::markdown::tera::TERA;
use crate::docs::models::Spec;
use crate::error::UsageErr;
use itertools::Itertools;
use serde::Serialize;
use std::collections::HashMap;
use xx::regex;

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub(crate) spec: Spec,
    pub(crate) header_level: usize,
    pub(crate) multi: bool,
    tera_ctx: tera::Context,
    url_prefix: Option<String>,
    html_encode: bool,
    replace_pre_with_code_fences: bool,
}

impl MarkdownRenderer {
    pub fn new(spec: crate::Spec) -> Self {
        let mut renderer = Self {
            spec: spec.into(),
            header_level: 1,
            multi: false,
            tera_ctx: tera::Context::new(),
            url_prefix: None,
            html_encode: true,
            replace_pre_with_code_fences: false,
        };
        let mut spec = renderer.spec.clone();
        spec.render_md(&renderer);
        renderer.spec = spec;
        renderer
    }

    pub fn with_header_level(mut self, header_level: usize) -> Self {
        self.header_level = header_level;
        self
    }

    pub fn with_multi(mut self, index: bool) -> Self {
        self.multi = index;
        self
    }

    pub fn with_url_prefix<S: Into<String>>(mut self, url_prefix: S) -> Self {
        self.url_prefix = Some(url_prefix.into());
        self
    }

    pub fn with_html_encode(mut self, html_encode: bool) -> Self {
        self.html_encode = html_encode;
        self
    }

    pub fn with_replace_pre_with_code_fences(mut self, replace_pre_with_code_fences: bool) -> Self {
        self.replace_pre_with_code_fences = replace_pre_with_code_fences;
        self
    }

    pub(crate) fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) {
        self.tera_ctx.insert(key, val);
    }

    fn tera_ctx(&self) -> tera::Context {
        let mut ctx = self.tera_ctx.clone();
        ctx.insert("spec", &self.spec);
        ctx.insert("header_level", &self.header_level);
        ctx.insert("multi", &self.multi);
        ctx.insert("url_prefix", &self.url_prefix);
        ctx.insert("html_encode", &self.html_encode);
        ctx
    }

    pub(crate) fn render(&self, template_name: &str) -> Result<String, UsageErr> {
        let mut tera = TERA.clone();

        if self.html_encode {
            tera.register_filter(
                "escape_md",
                move |value: &tera::Value, _: &HashMap<String, tera::Value>| {
                    let value = value.as_str().unwrap();
                    Ok(escape_md(value).into())
                },
            );
        } else {
            tera.register_filter(
                "escape_md",
                move |value: &tera::Value, _: &HashMap<String, tera::Value>| Ok(value.clone()),
            );
        }
        let path_re =
            regex!(r"https://(github.com/[^/]+/[^/]+|gitlab.com/[^/]+/[^/]+/-)/blob/[^/]+/");
        tera.register_function("source_code_link", |args: &HashMap<String, tera::Value>| {
            let spec = args.get("spec").unwrap().as_object().unwrap();
            let cmd = args.get("cmd").unwrap().as_object().unwrap();
            let full_cmd = cmd.get("full_cmd").unwrap().as_array();
            let source_code_link_template = spec
                .get("source_code_link_template")
                .and_then(|v| v.as_str());
            if let (Some(full_cmd), Some(source_code_link_template)) =
                (full_cmd, source_code_link_template)
            {
                if full_cmd.is_empty() {
                    return Ok("".into());
                }
                let mut ctx = tera::Context::new();
                let path = full_cmd.iter().map(|v| v.as_str().unwrap()).join("/");
                ctx.insert("spec", spec);
                ctx.insert("cmd", cmd);
                ctx.insert("path", &path);
                let href = TERA.clone().render_str(source_code_link_template, &ctx)?;
                let friendly = path_re.replace_all(&href, "").to_string();
                let link = if path_re.is_match(&href) {
                    format!("[`{friendly}`]({href})")
                } else {
                    format!("[{friendly}]({href})")
                };
                Ok(link.into())
            } else {
                Ok("".into())
            }
        });

        Ok(tera.render(template_name, &self.tera_ctx())?)
    }

    pub(crate) fn replace_code_fences(&self, md: String) -> String {
        if !self.replace_pre_with_code_fences {
            return md;
        }
        // TODO: handle fences inside of <pre> or <code>
        let mut in_code_block = false;
        let mut new_md = String::new();
        for line in md.lines() {
            if let Some(line) = line.strip_prefix("    ") {
                if in_code_block {
                    new_md.push_str(&format!("{line}\n"));
                } else {
                    new_md.push_str(&format!("```\n{line}\n"));
                    in_code_block = true;
                }
            } else {
                if in_code_block {
                    new_md.push_str("```\n");
                    in_code_block = false;
                }
                new_md.push_str(&format!("{line}\n"));
            }
        }
        if in_code_block {
            new_md.push_str("```\n");
        }
        new_md.replace("```\n\n```\n", "\n")
    }
}

fn escape_md(value: &str) -> String {
    let mut segments = vec![];
    let mut current_segment = String::new();
    let mut is_current_block = false;

    // (fence_char, fence_len, fence_quote_level)
    let mut current_fence: Option<(char, usize, usize)> = None;
    let mut list_stack: Vec<(usize, usize)> = vec![]; // (quote_level, content_indent)
    let list_re = xx::regex!(r"^([-*+]|\d{1,9}[.)])(\s+|$)");
    let ordered_list_start_1_re = xx::regex!(r"^1[.)](\s+|$)");

    // Track if we are effectively in a paragraph (to decide if indented code can start)
    let mut in_paragraph = false;

    for line in value.lines() {
        // Handle Fenced Code Blocks
        if let Some((fence_char, fence_len, fence_quote_level)) = current_fence {
            let (content, level) = strip_quotes_lim(line, fence_quote_level);
            if level < fence_quote_level {
                // Fence implicitly closed by end of block quote
                current_fence = None;
            } else {
                // Check for fence end
                let indent = content.chars().take_while(|c| *c == ' ').count();
                let trimmed = &content[indent..];
                if indent < 4 && trimmed.starts_with(fence_char) {
                    let count = trimmed.chars().take_while(|c| *c == fence_char).count();
                    if count >= fence_len && trimmed[count..].trim().is_empty() {
                        current_fence = None;
                        if !is_current_block && !current_segment.is_empty() {
                            segments.push((false, std::mem::take(&mut current_segment)));
                        }
                        if !is_current_block {
                            is_current_block = true;
                        }
                        current_segment.push_str(line);
                        current_segment.push('\n');
                        in_paragraph = false;
                        continue;
                    }
                }
                if !is_current_block && !current_segment.is_empty() {
                    segments.push((false, std::mem::take(&mut current_segment)));
                }
                is_current_block = true;
                current_segment.push_str(line);
                current_segment.push('\n');
                // Inside fence, lines don't affect outer paragraph state
                continue;
            }
        }

        // Normal Processing
        let (content, level) = strip_quotes_lim(line, usize::MAX);
        let indent = content.chars().take_while(|c| *c == ' ').count();
        let trimmed = &content[indent..];
        let is_blank = trimmed.is_empty();

        // Check for fence start
        if indent < 4 {
            if let Some(first_char) = trimmed.chars().next() {
                if first_char == '`' || first_char == '~' {
                    let count = trimmed.chars().take_while(|c| *c == first_char).count();
                    if count >= 3 {
                        let info_string = &trimmed[count..];
                        if !(first_char == '`' && info_string.contains('`')) {
                            current_fence = Some((first_char, count, level));

                            if !is_current_block && !current_segment.is_empty() {
                                segments.push((false, std::mem::take(&mut current_segment)));
                            }
                            is_current_block = true;
                            current_segment.push_str(line);
                            current_segment.push('\n');
                            in_paragraph = false;
                            continue;
                        }
                    }
                }
            }
        }

        // Check for list marker
        if let Some(caps) = list_re.captures(trimmed) {
            let is_ordered = caps
                .get(1)
                .unwrap()
                .as_str()
                .chars()
                .next()
                .unwrap()
                .is_numeric();
            let can_interrupt = !is_ordered || ordered_list_start_1_re.is_match(trimmed);

            if can_interrupt || !in_paragraph {
                let m_len = caps.get(1).unwrap().len();
                let s_len = caps.get(2).unwrap().len();
                let content_indent = indent + m_len + s_len;

                while let Some(&(lvl, top)) = list_stack.last() {
                    if level < lvl || (level == lvl && indent < top) {
                        list_stack.pop();
                    } else {
                        break;
                    }
                }
                list_stack.push((level, content_indent));

                if is_current_block && !current_segment.is_empty() {
                    segments.push((true, std::mem::take(&mut current_segment)));
                }
                is_current_block = false;
                current_segment.push_str(line);
                current_segment.push('\n');
                in_paragraph = true;
                continue;
            }
        }

        // Manual Block Detection
        let mut is_block = false;

        // ATX Heading
        if indent < 4 && trimmed.starts_with('#') {
            let after_hash = trimmed.trim_start_matches('#');
            if !trimmed.starts_with("#######")
                && (after_hash.is_empty() || after_hash.starts_with(' '))
            {
                is_block = true;
            }
        }

        // Block Quote
        if !is_block && indent < 4 && trimmed.starts_with('>') {
            is_block = true;
        }

        // Setext Heading Underline
        if !is_block && indent < 4 && (trimmed.starts_with('-') || trimmed.starts_with('=')) {
            let c = trimmed.chars().next().unwrap();
            if trimmed.chars().all(|x| x == c || x == ' ') {
                is_block = true;
            }
        }

        // Thematic Break
        if !is_block && indent < 4 {
            if let Some(c) = trimmed.chars().next() {
                if c == '-' || c == '*' || c == '_' {
                    // Count non-space chars
                    let count = trimmed.chars().filter(|&x| x == c).count();
                    let other = trimmed.chars().any(|x| x != c && x != ' ');
                    if count >= 3 && !other {
                        is_block = true;
                    }
                }
            }
        }

        if is_block {
            if is_current_block && !current_segment.is_empty() {
                segments.push((true, std::mem::take(&mut current_segment)));
            }
            is_current_block = false;
            current_segment.push_str(line);
            current_segment.push('\n');
            in_paragraph = false;
            continue;
        }

        // Check for indented code block
        let active_list_item = list_stack.iter().rfind(|&&(lvl, _)| lvl == level);
        let threshold = active_list_item.map(|&(_, indent)| indent).unwrap_or(0) + 4;

        if indent >= threshold && !is_blank && !in_paragraph {
            if !is_current_block && !current_segment.is_empty() {
                segments.push((false, std::mem::take(&mut current_segment)));
            }
            is_current_block = true;
            current_segment.push_str(line);
            current_segment.push('\n');
            continue;
        }

        // Regular text line
        if is_current_block && !current_segment.is_empty() {
            segments.push((true, std::mem::take(&mut current_segment)));
        }
        is_current_block = false;
        current_segment.push_str(line);
        current_segment.push('\n');

        in_paragraph = !is_blank;
    }

    // Push final segment
    if !current_segment.is_empty() {
        segments.push((is_current_block, current_segment));
    }

    let mut output = String::with_capacity(value.len());
    for (is_block, text) in segments {
        if is_block {
            output.push_str(&text);
        } else {
            output.push_str(&process_inline(&text));
        }
    }

    if !output.is_empty() {
        output.pop();
    }
    output
}

fn strip_quotes_lim(line: &str, limit: usize) -> (&str, usize) {
    let mut remainder = line;
    let mut level = 0;
    while level < limit {
        let indent = remainder.chars().take_while(|c| *c == ' ').count();
        if indent > 3 {
            break;
        }
        let after_indent = &remainder[indent..];
        if let Some(stripped) = after_indent.strip_prefix('>') {
            let mut advance = indent + 1;
            if stripped.starts_with(' ') {
                advance += 1;
            }
            remainder = &remainder[advance..];
            level += 1;
        } else {
            break;
        }
    }
    (remainder, level)
}

fn process_inline(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    let mut in_code_span = false;

    while let Some((_i, c)) = chars.next() {
        if c == '`' {
            let mut len = 1;
            while let Some(&(_, next_c)) = chars.peek() {
                if next_c == '`' {
                    len += 1;
                    chars.next();
                } else {
                    break;
                }
            }

            if !in_code_span {
                let mut remaining_chars = chars.clone();
                let mut found_closer = false;

                while let Some((_, rc)) = remaining_chars.next() {
                    if rc == '`' {
                        let mut r_len = 1;
                        while let Some(&(_, next_rc)) = remaining_chars.peek() {
                            if next_rc == '`' {
                                r_len += 1;
                                remaining_chars.next();
                            } else {
                                break;
                            }
                        }
                        if r_len == len {
                            found_closer = true;
                            break;
                        }
                    }
                }

                if found_closer {
                    in_code_span = true;
                    for _ in 0..len {
                        output.push('`');
                    }

                    loop {
                        if let Some(&(_, pc)) = chars.peek() {
                            if pc == '`' {
                                let lookahead = chars.clone();
                                let mut run_len = 0;
                                for (_, lc) in lookahead {
                                    if lc == '`' {
                                        run_len += 1;
                                    } else {
                                        break;
                                    }
                                }

                                if run_len == len {
                                    for _ in 0..run_len {
                                        output.push('`');
                                        chars.next();
                                    }
                                    in_code_span = false;
                                    break;
                                } else {
                                    for _ in 0..run_len {
                                        output.push('`');
                                        chars.next();
                                    }
                                    continue;
                                }
                            }
                        }

                        if let Some((_, char_in_span)) = chars.next() {
                            output.push(char_in_span);
                        } else {
                            break;
                        }
                    }
                } else {
                    for _ in 0..len {
                        output.push('`');
                    }
                }
            } else {
                for _ in 0..len {
                    output.push('`');
                }
            }
        } else if c == '<' {
            if !in_code_span {
                output.push_str("&lt;");
            } else {
                output.push('<');
            }
        } else {
            output.push(c);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // https://spec.commonmark.org/0.31.2/#indented-code-blocks
    #[test]
    fn test_indented_code_blocks() {
        let cases = vec![
            // 107
            (
                "    a simple <\n      indented code < block",
                "    a simple <\n      indented code < block",
            ),
            // 108
            ("  - foo <\n\n    bar <", "  - foo &lt;\n\n    bar &lt;"),
            // 109
            ("1.  foo <\n\n    - bar <", "1.  foo &lt;\n\n    - bar &lt;"),
            // 110
            (
                "    <a/>\n    *hi*\n\n    - one",
                "    <a/>\n    *hi*\n\n    - one",
            ),
            // 111
            (
                "    chunk1 <\n\n    chunk2 <\n  \n \n \n    chunk3 <",
                "    chunk1 <\n\n    chunk2 <\n  \n \n \n    chunk3 <",
            ),
            // 112
            (
                "    chunk1 <\n      \n      chunk2 <",
                "    chunk1 <\n      \n      chunk2 <",
            ),
            // 113
            ("Foo <\n    bar <", "Foo &lt;\n    bar &lt;"),
            // 114
            ("    foo <\nbar <", "    foo <\nbar &lt;"),
            // 115
            (
                "# Heading <\n    foo <\nHeading <\n------\n    foo <\n----",
                "# Heading &lt;\n    foo <\nHeading &lt;\n------\n    foo <\n----",
            ),
            // 116
            ("        foo <\n    bar <", "        foo <\n    bar <"),
            // 117
            ("\n    \n    foo <\n    ", "\n    \n    foo <\n    "),
            // 118
            ("    foo <  ", "    foo <  "),
        ];

        for (input, expected) in cases {
            assert_eq!(
                escape_md(input),
                expected,
                "Failed on input:\n---\n{}\n---",
                input
            );
        }
    }

    // https://spec.commonmark.org/0.31.2/#fenced-code-blocks
    #[test]
    fn test_fenced_code_blocks() {
        let cases = vec![
            // 119
            ("```\n<\n >\n```", "```\n<\n >\n```"),
            // 120
            (
                "~~~
<\n >\n~~~",
                "~~~
<\n >\n~~~",
            ),
            // 121
            ("``\nfoo <\n``", "``\nfoo <\n``"),
            // 122
            (
                "```\naaa <\n~~~
```",
                "```\naaa <\n~~~
```",
            ),
            // 123
            (
                "~~~
aaa <\n```\n~~~",
                "~~~
aaa <\n```\n~~~",
            ),
            // 124
            ("````\naaa <\n```\n``````", "````\naaa <\n```\n``````"),
            // 125
            (
                "~~~~
aaa <\n~~~
~~~~",
                "~~~~
aaa <\n~~~
~~~~",
            ),
            // 126
            ("```", "```"),
            // 127
            ("`````\n\n```\naaa <", "`````\n\n```\naaa <"),
            // 128
            ("> ```\n> aaa <\n\nbbb <", "> ```\n> aaa <\n\nbbb &lt;"),
            // 129
            ("```\n\n  \n```", "```\n\n  \n```"),
            // 130
            ("```\n```", "```\n```"),
            // 131
            (" ```\n aaa <\naaa <\n```", " ```\n aaa <\naaa <\n```"),
            // 132
            (
                "  ```\naaa <\n  aaa <\naaa <\n  ```",
                "  ```\naaa <\n  aaa <\naaa <\n  ```",
            ),
            // 133
            (
                "   ```\n   aaa <\n    aaa <\n  aaa <\n   ```",
                "   ```\n   aaa <\n    aaa <\n  aaa <\n   ```",
            ),
            // 134
            ("    ```\n    aaa <\n    ```", "    ```\n    aaa <\n    ```"),
            // 135
            ("```\naaa <\n  ```", "```\naaa <\n  ```"),
            // 136
            ("   ```\naaa <\n  ```", "   ```\naaa <\n  ```"),
            // 137
            ("```\naaa <\n    ```", "```\naaa <\n    ```"),
            // 138
            ("``` ```\naaa <", "``` ```\naaa &lt;"),
            // 139
            (
                "~~~~~~
aaa <\n~~~ ~~",
                "~~~~~~
aaa <\n~~~ ~~",
            ),
            // 140
            (
                "foo <\n```\nbar <\n```\nbaz <",
                "foo &lt;\n```\nbar <\n```\nbaz &lt;",
            ),
            // 141
            (
                "foo <\n---\n~~~
bar <\n~~~
# baz <",
                "foo &lt;\n---\n~~~
bar <\n~~~
# baz &lt;",
            ),
            // 142
            (
                "```ruby\ndef foo(x) <\n  return 3\nend\n```",
                "```ruby\ndef foo(x) <\n  return 3\nend\n```",
            ),
            // 143
            (
                "~~~~    ruby startline=3 $%@#$
def foo(x) <\n  return 3\nend\n~~~~~~~",
                "~~~~    ruby startline=3 $%@#$
def foo(x) <\n  return 3\nend\n~~~~~~~",
            ),
            // 144
            ("````;\n````", "````;\n````"),
            // 145
            ("``` aa ```\nfoo <", "``` aa ```\nfoo &lt;"),
            // 146
            (
                "~~~ aa ``` ~~~
foo <\n~~~",
                "~~~ aa ``` ~~~
foo <\n~~~",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(
                escape_md(input),
                expected,
                "Failed on input:\n---\n{}\n---",
                input
            );
        }
    }

    // https://spec.commonmark.org/0.31.2/#code-spans
    #[test]
    fn test_code_spans() {
        let cases = vec![
            // 328
            ("`foo <`", "`foo <`"),
            ("`foo` <", "`foo` &lt;"),
            // 329
            ("`` foo ` bar < ``", "`` foo ` bar < ``"),
            // 330
            ("` `` < `", "` `` < `"),
            // 331
            ("`  `` <  `", "`  `` <  `"),
            // 332
            ("` a <`", "` a <`"),
            // 333 (using normal space since non-breaking might be tricky in string literal)
            ("` b < `", "` b < `"),
            // 336
            ("``\nfoo <\nbar <\n``", "``\nfoo <\nbar <\n``"),
            // 337
            ("``\nfoo < \n``", "``\nfoo < \n``"),
            // 338
            ("`foo <   bar < \nbaz <`", "`foo <   bar < \nbaz <`"),
            // 339
            (r"`foo\`bar <`", r"`foo\`bar &lt;`"),
            // 340
            ("``foo`bar <``", "``foo`bar <``"),
            // 341
            ("` foo `` bar < `", "` foo `` bar < `"),
            // 342
            ("*foo`*` <", "*foo`*` &lt;"),
            // 343
            ("[not a `link <](/foo`)", "[not a `link <](/foo`)"),
            // 344
            (r#"`<a href=\"``\">`"#, r#"`<a href=\"``\">`"#),
            // 345
            (r#"<a href=\"``\">`"#, r#"&lt;a href=\"``\">`"#),
            // 346
            ("`<https://foo.bar.`baz>`", "`<https://foo.bar.`baz>`"),
            // 347
            ("<https://foo.bar.`baz>`", "&lt;https://foo.bar.`baz>`"),
            // 348
            ("```foo`` <", "```foo`` &lt;"),
            // 349
            ("`foo <", "`foo &lt;"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                escape_md(input),
                expected,
                "Failed on input:\n---\n{}\n---",
                input
            );
        }
    }
}
