const INDEX: &str = include_str!("../index.html");
const TRUNK: &str = include_str!("../Trunk.toml");
const APP: &str = include_str!("../src/app.rs");
const ATLAS_META: &str = include_str!("../assets/fonts/signs-rgba.json");
const CATALOG: &str = include_str!("../assets/signs/catalog.json");
const ASSET_README: &str = include_str!("../assets/README.md");
const LICENSE: &str = include_str!("../assets/licenses/NotoSansJP-OFL.txt");
const PROVENANCE: &str = include_str!("../assets/licenses/font-provenance.json");
const PNG: &[u8] = include_bytes!("../assets/fonts/signs-rgba.png");

#[test]
fn atlas_png_and_metadata_agree_on_dimensions_and_24_regions() {
    assert!(PNG.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(&PNG[12..16], b"IHDR");
    let width = u32::from_be_bytes(PNG[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(PNG[20..24].try_into().unwrap());
    assert_eq!((width, height), (2048, 1536));
    assert!(ATLAS_META.contains("\"width\": 2048"));
    assert!(ATLAS_META.contains("\"height\": 1536"));
    assert_eq!(ATLAS_META.matches("\"region_id\":").count(), 24);
    for id in 0..24 {
        assert!(ATLAS_META.contains(&format!("\"region_id\": {id}")));
    }
}

#[test]
fn catalog_has_six_signs_for_each_of_four_kinds() {
    assert_eq!(CATALOG.matches("\"region_id\":").count(), 24);
    for kind in ["Tube", "Led", "Hologram", "FaultText"] {
        assert_eq!(CATALOG.matches(&format!("\"kind\": \"{kind}\"")).count(), 6);
    }
    for text in ["夜市", "ラーメン", "OPEN 24H"] {
        assert!(CATALOG.contains(text));
    }
}

#[test]
fn trunk_copies_runtime_assets_and_asset_urls_are_document_relative() {
    assert!(INDEX.contains("data-trunk rel=\"copy-dir\" href=\"assets\""));
    assert!(TRUNK.contains("target = \"index.html\""));
    assert!(APP.contains("fetch_bytes(\"assets/fonts/signs-rgba.png\")"));
    assert!(APP.contains("fetch_bytes(\"assets/fonts/signs-rgba.json\")"));
    assert!(!APP.contains("fetch_bytes(\"/assets/"));
    assert!(ASSET_README.contains("never fetch `/assets/...`"));
}

#[test]
fn bundled_font_license_and_provenance_are_present() {
    assert!(LICENSE.contains("SIL OPEN FONT LICENSE Version 1.1"));
    assert!(PROVENANCE.contains("sha256"));
    assert!(ASSET_README.contains("Noto Sans JP"));
}
