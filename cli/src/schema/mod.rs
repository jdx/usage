//! A JSON Schema for a CLI's config file, from its `config` block.
//!
//! Every CLI in the fleet hand-writes one of these — mise generates `schema/mise.json` from
//! its own registry with a TypeScript script. Generating it from the spec means the schema,
//! the documentation and the resolver cannot disagree about what a setting is.
//!
//! Draft 2020-12, with `unevaluatedProperties: false` on every object so an unknown key in
//! a config file is reported by an editor rather than ignored.

use serde_json::{json, Map, Value};

use usage::spec::config::{SpecConfig, SpecConfigProp, SpecConfigScope};
use usage::spec::config_type::{Base, SpecConfigType};

/// What to call the schema, and where it claims to live.
#[derive(Debug, Default, Clone)]
pub struct SchemaOptions {
    pub title: Option<String>,
    /// The `$id` — where the schema is published, if anywhere.
    pub url: Option<String>,
}

/// The schema for the config file a spec describes.
pub fn config_schema(config: &SpecConfig, options: &SchemaOptions) -> Value {
    let mut schema = Map::new();
    schema.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    if let Some(url) = &options.url {
        schema.insert("$id".into(), json!(url));
    }
    if let Some(title) = &options.title {
        schema.insert("title".into(), json!(title));
    }
    schema.insert("type".into(), json!("object"));
    schema.insert("unevaluatedProperties".into(), json!(false));
    schema.insert("properties".into(), Value::Object(nest(config)));
    Value::Object(schema)
}

/// Dotted keys become nested objects: `task.output` is `task: { output: … }`.
///
/// This is what the dotted spelling buys — one canonical key in the spec, and the shape a
/// config file actually has reconstructed from it here.
fn nest(config: &SpecConfig) -> Map<String, Value> {
    let mut root = Map::new();
    for (key, prop) in &config.props {
        // Hidden settings are still settable, so they stay in the schema — with
        // `unevaluatedProperties: false`, leaving one out turns an editor red on a config
        // file that is perfectly legal. `hide` is a documentation and completion concern.
        //
        // `scope="env"` is different in kind: such a setting cannot come from a file at all,
        // so listing it would advertise a key the CLI will ignore there. mise's own
        // schema.ts does list its five `env_only` settings, and that is the drift this is
        // generated from a spec to avoid — a deliberate divergence from the parity target.
        if prop.scope == SpecConfigScope::Env {
            continue;
        }
        let mut segments: Vec<&str> = key.split('.').collect();
        let leaf = segments.pop().unwrap_or(key.as_str());
        let mut table = &mut root;
        for segment in segments {
            // A key that is both a value and a group — `prop "a"` beside `prop "a.b"` — is
            // a contradiction a schema cannot express. The group wins, because the keys
            // under it would otherwise have nowhere to live; a scalar `type` left beside
            // `properties` would make every write of `a` invalid instead of just that one.
            //
            // Replacing it wholesale was too blunt, though: a `map` or an `object` parent has
            // no `properties` either, only `additionalProperties` or a bare `type: object` —
            // so overwriting it dropped exactly the keyword that let the map hold anything,
            // and `unevaluatedProperties: false` then rejected every key it was declared to
            // accept. Only a parent that cannot hold keys at all is replaced.
            let entry = table
                .entry(segment.to_string())
                .or_insert_with(|| json!({"type": "object", "unevaluatedProperties": false}));
            let holds_keys = entry.get("type").is_some_and(|ty| ty == "object");
            if !holds_keys {
                *entry = json!({"type": "object", "unevaluatedProperties": false});
            }
            table = entry
                .as_object_mut()
                .expect("just inserted an object")
                .entry("properties")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("properties is an object");
        }
        // No guard needed the other way round: props is sorted, and a key sorts before
        // every key that extends it, so `a` is always written before `a.b` promotes it.
        table.insert(leaf.to_string(), prop_schema(prop));
    }
    root
}

fn prop_schema(prop: &SpecConfigProp) -> Value {
    let ty = prop.value_type.clone().unwrap_or_default();
    let mut schema = type_schema(&ty);
    let object = schema.as_object_mut().expect("a schema is an object");

    // The long form when there is one: an editor's hover is the one place a reader gets the
    // whole story without leaving the file.
    let description = prop
        .long_help
        .clone()
        .or_else(|| prop.help.clone())
        .map(|text| match &prop.deprecated {
            Some(why) => format!("{text}\n\nDeprecated: {why}"),
            None => text,
        })
        .or_else(|| {
            prop.deprecated
                .as_ref()
                .map(|why| format!("Deprecated: {why}"))
        });
    if let Some(description) = description {
        object.insert("description".into(), json!(description));
    }
    if let Some(default) = &prop.default {
        object.insert("default".into(), value_json(default));
    } else if !prop.default_list.is_empty() {
        object.insert("default".into(), json!(prop.default_list));
    }
    if prop.deprecated.is_some() {
        object.insert("deprecated".into(), json!(true));
    }
    if !prop.choices.is_empty() {
        // Onto every position a *value* can appear in, recursively: `choices` says what one
        // value may be, and where that value sits depends on the type. A list keeps it on
        // `items`, a map on `additionalProperties`, a union on each of its branches — and a
        // `string|list<string>` needs both, which is why this recurses rather than picking one
        // place. An `enum` beside `type: array`, or beside an `anyOf` it was not distributed
        // into, is AND-combined with it and nothing can satisfy the pair.
        let allowed = Value::Array(prop.choices.iter().map(|c| value_json(&c.value)).collect());
        constrain(&mut schema, &allowed);
    }
    schema
}

/// Apply an `enum` to every position a single value can occupy in `schema`.
///
/// Down through containers and across a union's branches, because that is where the values are.
/// `mise`'s `python.uv_venv_auto` is a `bool|string` listing all four of its values, and each
/// branch has to carry them or the branch that does not match rejects the value outright.
fn constrain(schema: &mut Value, allowed: &Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if let Some(branches) = object.get_mut("anyOf").and_then(Value::as_array_mut) {
        for branch in branches {
            constrain(branch, allowed);
        }
        return;
    }
    for key in ["items", "additionalProperties"] {
        if object.get(key).is_some_and(Value::is_object) {
            let inner = object.get_mut(key).expect("just checked");
            constrain(inner, allowed);
            return;
        }
    }
    object.insert("enum".into(), allowed.clone());
}

fn type_schema(ty: &SpecConfigType) -> Value {
    match ty {
        SpecConfigType::Base(base) => base_schema(base),
        SpecConfigType::List(inner) | SpecConfigType::Set(inner) => {
            let mut schema = json!({"type": "array", "items": type_schema(inner)});
            // A set is an array that does not repeat, which JSON Schema can say.
            if matches!(ty, SpecConfigType::Set(_)) {
                schema
                    .as_object_mut()
                    .expect("an object")
                    .insert("uniqueItems".into(), json!(true));
            }
            schema
        }
        SpecConfigType::Map(_, value) => json!({
            "type": "object",
            "additionalProperties": type_schema(value),
        }),
        // Absent is legitimate, which in a schema is simply not being required — nothing
        // here requires anything, so the inner type is the whole answer.
        SpecConfigType::Option(inner) => type_schema(inner),
        SpecConfigType::Union(members) => {
            json!({"anyOf": members.iter().map(type_schema).collect::<Vec<_>>()})
        }
    }
}

fn base_schema(base: &Base) -> Value {
    match base {
        Base::Bool => json!({"type": "boolean"}),
        Base::Int => json!({"type": "integer"}),
        Base::Uint => json!({"type": "integer", "minimum": 0}),
        Base::Float => json!({"type": "number"}),
        Base::Path => json!({"type": "string"}),
        Base::Url => json!({"type": "string", "format": "uri"}),
        Base::Duration => json!({"type": "string"}),
        Base::Object => json!({"type": "object"}),
        // A type only the tool understands. A string accepts what it is written as in a
        // config file, which is the most a schema can say without knowing more.
        Base::String | Base::Custom(_) => json!({"type": "string"}),
    }
}

fn value_json(value: &usage::spec::config::SpecConfigValue) -> Value {
    use usage::spec::config::SpecConfigValue as V;
    match value {
        V::Bool(b) => json!(b),
        V::Int(i) => json!(i),
        V::Float(f) => json!(f),
        V::String(s) => json!(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usage::Spec;

    fn schema_of(src: &str) -> Value {
        let spec: Spec = src.parse().unwrap();
        config_schema(
            &spec.config,
            &SchemaOptions {
                title: Some("Example config".into()),
                url: Some("https://example.com/schema.json".into()),
            },
        )
    }

    #[test]
    fn every_type_becomes_something_a_validator_understands() {
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "flag" type="bool"
    prop "count" type="int"
    prop "jobs" type="uint"
    prop "rate" type="float"
    prop "dir" type="path"
    prop "site" type="url"
    prop "wait" type="duration"
    prop "loose" type="object"
    prop "names" type="list<string>"
    prop "uniq" type="set<string>"
    prop "vars" type="map<string, string>"
    prop "maybe" type="option<int>"
    prop "either" type="bool|string"
    prop "exotic" type="crate::Weird"
}
"##,
        );
        let props = &schema["properties"];
        assert_eq!(props["flag"]["type"], "boolean");
        assert_eq!(props["count"]["type"], "integer");
        assert_eq!(props["jobs"]["minimum"], 0);
        assert_eq!(props["rate"]["type"], "number");
        assert_eq!(props["dir"]["type"], "string");
        assert_eq!(props["site"]["format"], "uri");
        assert_eq!(props["wait"]["type"], "string");
        assert_eq!(props["loose"]["type"], "object");
        assert_eq!(props["names"]["items"]["type"], "string");
        assert_eq!(props["uniq"]["uniqueItems"], true);
        assert_eq!(props["vars"]["additionalProperties"]["type"], "string");
        // Optional is "not required", and nothing here is required.
        assert_eq!(props["maybe"]["type"], "integer");
        assert_eq!(props["either"]["anyOf"][0]["type"], "boolean");
        assert_eq!(props["either"]["anyOf"][1]["type"], "string");
        // A type only the tool knows still validates as what a file can hold.
        assert_eq!(props["exotic"]["type"], "string");
    }

    #[test]
    fn dotted_keys_become_the_shape_a_file_has() {
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "task.output" type="string" help="How to print task output"
    prop "task.cache.remote_mode" type="string"
    prop "jobs" type="uint"
}
"##,
        );
        let task = &schema["properties"]["task"];
        assert_eq!(task["type"], "object");
        assert_eq!(task["unevaluatedProperties"], false);
        assert_eq!(task["properties"]["output"]["type"], "string");
        assert_eq!(
            task["properties"]["cache"]["properties"]["remote_mode"]["type"],
            "string"
        );
        assert_eq!(schema["properties"]["jobs"]["type"], "integer");
    }

    #[test]
    fn what_an_editor_shows_comes_from_the_spec() {
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "shell" type="string" default="bash" help="Which shell" {
        choices {
            choice "bash"
            choice "zsh"
        }
    }
    prop "exclude" type="list<string>" {
        default "target" "node_modules"
    }
    prop "old" type="bool" deprecated="Use new instead." help="Old setting"
    prop "verbose" type="bool" default=#false help="Say more" \
        long_help="Say more.\n\nIncluding about things you did not ask about."
}
"##,
        );
        let props = &schema["properties"];
        assert_eq!(props["shell"]["default"], "bash");
        assert_eq!(props["shell"]["enum"], json!(["bash", "zsh"]));
        assert_eq!(props["shell"]["description"], "Which shell");
        assert_eq!(
            props["exclude"]["default"],
            json!(["target", "node_modules"])
        );
        assert_eq!(props["old"]["deprecated"], true);
        assert_eq!(
            props["old"]["description"],
            "Old setting\n\nDeprecated: Use new instead."
        );
        // The long form when there is one: hover is where a reader gets the whole story.
        assert!(props["verbose"]["description"]
            .as_str()
            .unwrap()
            .contains("did not ask about"));
        assert_eq!(props["verbose"]["default"], false);
    }

    #[test]
    fn a_schema_lists_what_a_file_can_actually_hold() {
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "internal" type="bool" hide=#true help="Not documented, still settable"
    prop "config_file" type="path" scope="env" help="Read from the environment only"
    prop "trusted" type="bool" scope="global" help="Not from a project file"
    prop "normal" type="bool"
}
"##,
        );
        let props = schema["properties"].as_object().expect("an object");
        // Hidden but writable: omitting it would make `unevaluatedProperties: false` reject
        // a legal file.
        assert!(props.contains_key("internal"));
        // A file is still a file, whatever its scope, so a global-scoped setting belongs.
        assert!(props.contains_key("trusted"));
        assert!(props.contains_key("normal"));
        // Never readable from a file, so advertising it would be a lie.
        assert!(
            !props.contains_key("config_file"),
            "an env-only setting is not a config file key: {props:?}"
        );
    }

    #[test]
    fn a_group_that_is_also_a_map_keeps_what_makes_it_one() {
        // A map or an object parent has no `properties` either — only `additionalProperties`,
        // or a bare `type: object`. Replacing it because of that dropped the very keyword that
        // let it hold anything, and `unevaluatedProperties: false` then rejected every key the
        // map was declared to accept.
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "vars" type="map<string, string>" help="Free-form"
    prop "vars.known" type="bool" help="And one we know about"
    prop "loose" type="object"
    prop "loose.known" type="bool"
}
"##,
        );
        let vars = &schema["properties"]["vars"];
        assert_eq!(
            vars["additionalProperties"]["type"], "string",
            "the map lost what makes it a map: {vars}"
        );
        assert_eq!(vars["properties"]["known"]["type"], "boolean");
        // A free-form object keeps being one, and gains the key it declared.
        let loose = &schema["properties"]["loose"];
        assert_eq!(loose["type"], "object");
        assert_eq!(loose["properties"]["known"]["type"], "boolean");
        assert!(
            loose.get("unevaluatedProperties").is_none(),
            "a free-form object should not have been made strict: {loose}"
        );
    }

    #[test]
    fn a_key_that_is_both_a_value_and_a_group_stays_valid_json_schema() {
        // Contradictory to declare, but a schema that says `type: string` *and* lists
        // properties rejects every possible value, which is worse than picking one.
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "a" type="string" help="Also a group, which cannot be"
    prop "a.b" type="bool"
    prop "z.y" type="bool"
    prop "z" type="string"
}
"##,
        );
        // Both orders: the scalar declared before its group, and after it.
        for (parent, child) in [("a", "b"), ("z", "y")] {
            let group = &schema["properties"][parent];
            assert_eq!(group["type"], "object", "{parent}");
            assert_eq!(group["properties"][child]["type"], "boolean", "{parent}");
            assert!(
                group.get("enum").is_none(),
                "{parent} kept scalar facts beside its properties: {group}"
            );
        }
    }

    #[test]
    fn choices_constrain_the_values_not_the_container() {
        // `choices` says what one value may be. On a list that means the items: an `enum` at
        // the top of an array schema is matched against the whole array, so no array can
        // satisfy it and the schema rejects every config file that sets the setting at all.
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "tools" type="list<string>" {
        choices {
            choice "node"
            choice "python"
        }
    }
    prop "levels" type="map<string, string>" {
        choices {
            choice "warn"
            choice "error"
        }
    }
    prop "shell" type="string" {
        choices {
            choice "bash"
        }
    }
}
"##,
        );
        let props = &schema["properties"];
        assert_eq!(props["tools"]["type"], "array");
        assert!(
            props["tools"].get("enum").is_none(),
            "an array cannot equal one of its items: {}",
            props["tools"]
        );
        assert_eq!(props["tools"]["items"]["enum"], json!(["node", "python"]));
        // Same for a map: the values are what is constrained.
        assert_eq!(
            props["levels"]["additionalProperties"]["enum"],
            json!(["warn", "error"])
        );
        assert!(props["levels"].get("enum").is_none());
        // A scalar keeps it where it always was.
        assert_eq!(props["shell"]["enum"], json!(["bash"]));
    }

    #[test]
    fn a_union_with_choices_is_described_by_its_choices() {
        // `anyOf` and `enum` are combined with AND, so string-only choices on a `bool|string`
        // setting rejected `true` — which the declared type plainly allows. A spec that lists
        // its choices has said what the accepted values are, and each carries its own type.
        let schema = schema_of(
            r##"
name "ex"
bin "ex"
config {
    prop "venv" type="bool|string" {
        choices {
            choice #false help="off"
            choice "source" help="source an existing venv"
            choice #true help="create and source"
        }
    }
    prop "plain" type="bool|string"
    prop "mixed" type="string|list<string>" {
        choices {
            choice "a"
            choice "b"
        }
    }
    prop "nested" type="map<string, list<string>>" {
        choices {
            choice "a"
            choice "b"
        }
    }
}
"##,
        );
        let nested = &schema["properties"]["nested"];
        // Down to the scalars: one level put the enum on the array under
        // `additionalProperties`, where it was AND-combined with `type: array`.
        assert_eq!(
            nested["additionalProperties"]["items"]["enum"],
            json!(["a", "b"])
        );
        assert!(nested["additionalProperties"].get("enum").is_none());

        // Into each branch, not on top of the union: an `enum` beside an `anyOf` is
        // AND-combined with it, so string-only choices on a `bool|string` rejected `true`.
        // Distributed, each branch accepts the choices of its own type and the union still
        // means "either".
        let venv = &schema["properties"]["venv"];
        assert!(
            venv.get("enum").is_none(),
            "the enum should be inside the branches: {venv}"
        );
        assert_eq!(venv["anyOf"][0]["type"], "boolean");
        assert_eq!(venv["anyOf"][0]["enum"], json!([false, "source", true]));
        assert_eq!(venv["anyOf"][1]["type"], "string");
        assert_eq!(venv["anyOf"][1]["enum"], json!([false, "source", true]));

        // And a union with a container branch needs both positions at once, which is why the
        // walk recurses rather than picking one place: dropping the `anyOf` here would have
        // rejected every list form the declared type allows.
        let mixed = &schema["properties"]["mixed"];
        assert_eq!(mixed["anyOf"][0]["enum"], json!(["a", "b"]));
        assert_eq!(mixed["anyOf"][1]["items"]["enum"], json!(["a", "b"]));
        // A union with no choices keeps describing itself as a union.
        let plain = &schema["properties"]["plain"];
        assert_eq!(plain["anyOf"][0]["type"], "boolean");
        assert!(plain.get("enum").is_none());
    }

    #[test]
    fn the_envelope_says_what_it_is() {
        let schema = schema_of("name \"ex\"\nbin \"ex\"\nconfig {\n  prop \"a\"\n}\n");
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["$id"], "https://example.com/schema.json");
        assert_eq!(schema["title"], "Example config");
        // An unknown key in a config file should be reported, not ignored.
        assert_eq!(schema["unevaluatedProperties"], false);
        // No type declared means a string, which is what a bare `prop "a"` can hold.
        assert_eq!(schema["properties"]["a"]["type"], "string");
    }
}
