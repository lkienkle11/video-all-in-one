//! Manifest parsing and version decision, tested against fixtures read at compile time.

mod update {
    use semver::Version;
    use vaio_core::update::{decide, parse_manifest, ManifestError, UpdateDecision};

    const VALID: &str = include_str!("fixtures/latest-valid.json");
    const LOWER: &str = include_str!("fixtures/latest-lower.json");
    const SCHEMA_2: &str = include_str!("fixtures/latest-schema-2.json");

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const VALID_SIGNATURE: &str =
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

    fn version(text: &str) -> Version {
        Version::parse(text).expect("test version is valid semver")
    }

    #[test]
    fn parses_schema_1_manifest() {
        let manifest = parse_manifest(VALID).expect("the valid fixture parses");
        assert_eq!(manifest.schema, 1);
        assert_eq!(manifest.version, version("0.1.1"));
        assert!(!manifest.notes_en.is_empty());
        assert!(!manifest.notes_vi.is_empty());
        let asset = &manifest.assets["macos-aarch64"];
        assert_eq!(asset.size, 12345);
        assert_eq!(asset.sha256, EMPTY_SHA256);
        assert_eq!(asset.signature, VALID_SIGNATURE);
    }

    #[test]
    fn unknown_schema_is_rejected_not_panicking() {
        assert_eq!(
            parse_manifest(SCHEMA_2),
            Err(ManifestError::UnknownSchema(2))
        );
    }

    #[test]
    fn future_schema_is_not_parsed_further() {
        // The body has no version or assets, so schema 1 parsing would fail on it.
        let text = r#"{"schema": 2, "unexpected": {"nested": true}}"#;
        assert_eq!(parse_manifest(text), Err(ManifestError::UnknownSchema(2)));
    }

    #[test]
    fn lower_version_is_up_to_date() {
        let manifest = parse_manifest(LOWER).expect("the lower fixture parses");
        let decision = decide(&version("0.1.1"), &manifest, "macos-aarch64");
        assert_eq!(decision, UpdateDecision::UpToDate);
    }

    #[test]
    fn equal_version_is_up_to_date() {
        let manifest = parse_manifest(VALID).expect("the valid fixture parses");
        let decision = decide(&version("0.1.1"), &manifest, "macos-aarch64");
        assert_eq!(decision, UpdateDecision::UpToDate);
    }

    #[test]
    fn higher_version_is_available_with_size() {
        let manifest = parse_manifest(VALID).expect("the valid fixture parses");
        let decision = decide(&version("0.1.0"), &manifest, "macos-aarch64");
        let UpdateDecision::Available {
            version: offered,
            size,
            notes_en,
            notes_vi,
            asset,
        } = decision
        else {
            panic!("expected an available update");
        };
        assert_eq!(offered, version("0.1.1"));
        assert_eq!(size, 12345);
        assert_eq!(asset, manifest.assets["macos-aarch64"]);
        assert_eq!(notes_en, manifest.notes_en);
        assert_eq!(notes_vi, manifest.notes_vi);
    }

    #[test]
    fn missing_platform_asset_is_not_for_this_platform() {
        let manifest = parse_manifest(VALID).expect("the valid fixture parses");
        let decision = decide(&version("0.1.0"), &manifest, "linux-x86_64");
        assert_eq!(decision, UpdateDecision::NotForThisPlatform);
    }

    #[test]
    fn schema_not_a_number_is_invalid() {
        let text = VALID.replace("\"schema\": 1", "\"schema\": \"1\"");
        assert!(matches!(
            parse_manifest(&text),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn not_json_is_invalid() {
        assert!(matches!(
            parse_manifest("{not json"),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn bad_hex_length_is_invalid() {
        let text = VALID.replace(EMPTY_SHA256, &EMPTY_SHA256[..63]);
        assert!(matches!(
            parse_manifest(&text),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn uppercase_sha256_is_invalid() {
        let text = VALID.replace(EMPTY_SHA256, &EMPTY_SHA256.to_uppercase());
        assert!(matches!(
            parse_manifest(&text),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn bad_signature_length_is_invalid() {
        let text = VALID.replace(VALID_SIGNATURE, "AAAA");
        assert!(matches!(
            parse_manifest(&text),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn zero_size_is_invalid() {
        let text = VALID.replace("\"size\": 12345", "\"size\": 0");
        assert!(matches!(
            parse_manifest(&text),
            Err(ManifestError::Invalid(_))
        ));
    }
}
