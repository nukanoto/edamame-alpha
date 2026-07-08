//! ファイル名パース、識別子、拡張子保持、レンディションメタデータ競合の単体テスト
//! （タスク T020）。

use edamame_alpha::domain::rendition::RenditionMetadata;
use edamame_alpha::domain::segment::{
    SegmentId, parse_filename, render_filename, validate_identifier,
};

#[test]
fn parses_sequence_and_extension() {
    assert_eq!(parse_filename("101.ts").unwrap(), (101, "ts".to_owned()));
    assert_eq!(parse_filename("102.m4s").unwrap(), (102, "m4s".to_owned()));
    assert_eq!(parse_filename("7.mp4").unwrap(), (7, "mp4".to_owned()));
}

#[test]
fn rejects_invalid_filenames() {
    assert!(parse_filename("noext").is_err());
    assert!(parse_filename("101.").is_err());
    assert!(parse_filename("abc.ts").is_err());
}

#[test]
fn renders_filename_from_sequence_and_extension() {
    assert_eq!(render_filename(101, "ts"), "101.ts");
}

#[test]
fn identity_uses_sequence_not_filename() {
    let id = SegmentId::new("live", "720p", 101).unwrap();
    assert_eq!(id.sequence, 101);
    assert_eq!(id.channel, "live");
    assert_eq!(id.rendition, "720p");
}

#[test]
fn rejects_identifiers_with_separators_or_empty() {
    assert!(validate_identifier("rendition", "").is_err());
    assert!(validate_identifier("rendition", "a/b").is_err());
    assert!(validate_identifier("channel", "live").is_ok());
    assert!(SegmentId::new("live", "bad/rendition", 1).is_err());
}

#[test]
fn detects_conflicting_metadata_and_merges_compatible() {
    let mut established = RenditionMetadata {
        bandwidth: Some(3_000_000),
        resolution: Some("1280x720".to_owned()),
    };

    // 同一のメタデータは互換性がある。
    assert!(established.merge(&established.clone()).is_ok());

    // 異なる帯域幅は競合し拒否される。
    let conflicting = RenditionMetadata {
        bandwidth: Some(6_000_000),
        resolution: None,
    };
    assert!(established.merge(&conflicting).is_err());

    // 空の入力メタデータは競合せず、値をそのままにする。
    let mut fresh = RenditionMetadata::default();
    let incoming = RenditionMetadata {
        bandwidth: Some(6_000_000),
        resolution: None,
    };
    fresh.merge(&incoming).unwrap();
    assert_eq!(fresh.bandwidth, Some(6_000_000));
}
