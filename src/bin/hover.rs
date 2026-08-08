use crate::drawing::layer::hover::{HoverInfo, HoverItem, Tag};
use osm::css::{CSSValue, RulesCache, Selector};
use osm::object::Object;

/// The styled fill color (0-255 rgb) for a feature's selector, for the legend dot.
fn layer_color(css: &RulesCache, selector: &Selector, zoom: f32) -> Option<[u8; 3]> {
    let sel = selector
        .clone()
        .with_any("zoom".to_string(), (zoom.floor() as u32).to_string());
    let color = css
        .get_matching_rules(&sel)
        .iter()
        .filter_map(|r| r.kvs.get("background-color"))
        .next_back()?;
    match color {
        CSSValue::Color(c) => Some([
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
        ]),
        _ => None,
    }
}

/// Bridge the app's hovered `Object`s to the layer-facing [`HoverInfo`] contract.
///
/// Builds borrowed views (no string cloning) into short-lived scratch buffers and
/// hands them to `f`, whose scope bounds the borrows. This closure shape is what
/// lets `HoverInfo` stay fully borrowed. `None` when nothing is hovered.
pub fn with_hover_info<R>(
    objects: &[Object],
    css: &RulesCache,
    zoom: f32,
    cursor: (f32, f32),
    pixels_per_point: f32,
    f: impl FnOnce(Option<HoverInfo>) -> R,
) -> R {
    if objects.is_empty() {
        return f(None);
    }

    struct Head<'a> {
        layer: Option<&'a str>,
        class: Option<&'a str>,
        title: Option<&'a str>,
        color: Option<[u8; 3]>,
        range: std::ops::Range<usize>,
    }

    // All items' tags laid out contiguously, with a slice range recorded per item.
    let mut tags: Vec<Tag> = Vec::new();
    let mut heads: Vec<Head> = Vec::new();
    for object in objects {
        let selector = object.selector();
        // Skip the background pseudo-feature; it carries no useful info.
        if selector.typ.as_deref() == Some("background") {
            continue;
        }
        let start = tags.len();
        for (name, value) in object.tags() {
            tags.push(Tag {
                name: name.as_str(),
                value: value.as_str(),
            });
        }
        heads.push(Head {
            layer: selector.any.get("name").map(String::as_str),
            class: selector.classes.first().map(String::as_str),
            title: object.title.as_deref(),
            color: layer_color(css, selector, zoom),
            range: start..tags.len(),
        });
    }
    if heads.is_empty() {
        return f(None);
    }
    let items: Vec<HoverItem> = heads
        .iter()
        .map(|h| HoverItem {
            layer: h.layer,
            class: h.class,
            title: h.title,
            color: h.color,
            tags: &tags[h.range.clone()],
        })
        .collect();

    f(Some(HoverInfo {
        cursor,
        pixels_per_point,
        items: &items,
    }))
}
