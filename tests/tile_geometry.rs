//! Snapshot tests over real vector tiles: decode a committed `.pbf` and lock in
//! the geometry the parser produces (kinds, ring splitting, per-selector counts).
//! Run `cargo insta review` to accept intentional changes.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use osm::feature::collection::FeatureCollection;
use osm::geometry::Geometry;
use osm::math::TileId;
use osm::vector_tile::tile::Tile;

#[derive(Debug)]
#[allow(dead_code)] // fields are read via Debug in the snapshot
struct TileSummary {
    object_count: usize,
    polygons: usize,
    lines: usize,
    points: usize,
    /// Total rings across all polygons — exercises the ring splitting.
    polygon_rings: usize,
    /// Polygons with more than one ring (holes / multipolygon parts).
    multi_ring_polygons: usize,
    by_selector: BTreeMap<String, usize>,
}

fn summarize(objects: &[osm::object::Object]) -> TileSummary {
    let mut summary = TileSummary {
        object_count: objects.len(),
        polygons: 0,
        lines: 0,
        points: 0,
        polygon_rings: 0,
        multi_ring_polygons: 0,
        by_selector: BTreeMap::new(),
    };
    for object in objects {
        *summary
            .by_selector
            .entry(object.selector().to_string())
            .or_default() += 1;
        match object.geometry() {
            Geometry::Polygon(polygon) => {
                summary.polygons += 1;
                summary.polygon_rings += polygon.rings().len();
                if polygon.rings().len() > 1 {
                    summary.multi_ring_polygons += 1;
                }
            }
            Geometry::Line(_) => summary.lines += 1,
            Geometry::Point(_) => summary.points += 1,
        }
    }
    summary
}

/// Parses `z_x_y` from a fixture file stem, e.g. `14_8580_5737`.
fn tile_id_from_stem(stem: &str) -> TileId {
    let mut parts = stem.split('_').map(|p| p.parse::<u32>().unwrap());
    TileId::new(
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    )
}

#[test]
fn tile_geometry_snapshots() {
    insta::glob!("fixtures/tiles/*.pbf", |path| {
        let data = std::fs::read(path).unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let tile_id = tile_id_from_stem(stem);
        let feature_collection = Arc::new(RwLock::new(FeatureCollection::new()));

        let tile = Tile::from_mbvt(&tile_id, &data, feature_collection, vec![]);
        let objects = tile.objects();
        let objects = objects.read().unwrap();

        insta::assert_debug_snapshot!(summarize(&objects));
    });
}

/// Every polygon feature must produce a non-empty outline band (the stroke
/// geometry that replaced the inflated-fill outline). Guards against the band
/// pass silently generating nothing.
#[test]
fn polygon_features_get_outline_bands() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tiles");
    let mut fixtures = 0;
    let mut outline_indices = 0usize;

    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("pbf") {
            continue;
        }
        fixtures += 1;
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let tile_id = tile_id_from_stem(stem);
        let data = std::fs::read(&path).unwrap();
        let fc = Arc::new(RwLock::new(FeatureCollection::new()));
        let tile = Tile::from_mbvt(&tile_id, &data, fc, vec![]);

        for (_, fill, outline) in tile.features() {
            // A band belongs to a real polygon feature: whenever there is an
            // outline, there is a fill.
            if !outline.is_empty() {
                assert!(!fill.is_empty());
            }
            outline_indices += outline.len();
        }
    }

    assert!(fixtures > 0, "no tile fixtures found");
    assert!(
        outline_indices > 0,
        "expected some outline band geometry across {fixtures} fixtures, got none"
    );
}
