//! The Luau type definitions (`types/zgui.d.luau`) cover what the engine
//! accepts: every style property, widget field, event, `ui` function and
//! animatable key. Adding one to the engine without its type fails here.

const DEFS: &str = include_str!("../types/zgui.d.luau");
const NODE: &str = include_str!("../src/node.rs");
const TARGET: &str = include_str!("../src/anime/target.rs");

/// The quoted names in `source` between `start` and the first `end` after it.
fn quoted(source: &str, start: &str, end: &str) -> Vec<String> {
    let from = source
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` not found"));
    let rest = &source[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    rest[..to]
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .map(str::to_owned)
        .collect()
}
/// A type's body in the definitions: from `start` to its closing `}`.
fn body<'a>(start: &str) -> &'a str {
    let from = DEFS
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` not in the definitions"));
    let rest = &DEFS[from..];
    &rest[..rest.find("\n}").expect("a closing brace")]
}
fn missing(names: &[String], within: &str) -> Vec<String> {
    names
        .iter()
        .filter(|n| {
            !within.contains(&format!("\t{n}: ")) && !within.contains(&format!("\t[\"{n}\"]: "))
        })
        .cloned()
        .collect()
}

#[test]
fn style_properties_are_typed() {
    let props = quoted(NODE, "props! {\n", "\n}");
    assert!(
        props.len() > 90,
        "found the property table: {}",
        props.len()
    );
    let gaps = missing(&props, body("type Props = {"));
    assert!(
        gaps.is_empty(),
        "style properties without types in Props: {gaps:?}"
    );
    let gaps = missing(&props, body("type Styles = {"));
    assert!(
        gaps.is_empty(),
        "style properties without types in Styles: {gaps:?}"
    );
}

#[test]
fn widget_fields_and_events_are_typed() {
    let fields = quoted(NODE, "const WIDGET_FIELDS", "];");
    let props = body("type Props = {");
    // Presence's options and `each`'s key are typed on those functions.
    let gaps: Vec<_> = missing(&fields, props)
        .into_iter()
        .filter(|f| !["key", "enter", "exit", "mode", "initial"].contains(&f.as_str()))
        .collect();
    assert!(gaps.is_empty(), "widget fields without types: {gaps:?}");
    let events: Vec<String> = NODE
        .split('"')
        .filter(|s| s.starts_with("on_") && s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .map(str::to_owned)
        .collect();
    let gaps = missing(&events, props);
    assert!(gaps.is_empty(), "events without types: {gaps:?}");
}

#[test]
fn ui_functions_are_typed() {
    let mut names = Vec::new();
    for chunk in NODE.split("ui.set(").skip(1) {
        if let Some(name) = chunk
            .trim_start()
            .strip_prefix('"')
            .and_then(|s| s.split('"').next())
        {
            names.push(name.to_owned());
        }
    }
    for list in [
        "(\"scroll\", false), (\"scroll_x\", true)",
        "\"image\", \"svg\"",
        "\"popover\", \"menu\"",
    ] {
        assert!(NODE.contains(list), "the constructor list `{list}` moved");
        names.extend(list.split('"').skip(1).step_by(2).map(str::to_owned));
    }
    let gaps = missing(&names, body("declare ui: {"));
    assert!(gaps.is_empty(), "ui functions without types: {gaps:?}");
}

#[test]
fn animatable_keys_are_typed() {
    let mut keys = quoted(TARGET, "pub fn for_element", "pub fn unit");
    // `shaderParam0` to `shaderParam15` share a prefix in the engine.
    assert!(
        keys.iter().any(|k| k == "shaderParam"),
        "the shader parameter keys moved"
    );
    keys.retain(|k| k != "shaderParam");
    keys.extend((0..16).map(|i| format!("shaderParam{i}")));
    let gaps = missing(&keys, body("type ElementParams = {"));
    assert!(gaps.is_empty(), "animatable keys without types: {gaps:?}");
}
