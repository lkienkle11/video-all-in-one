use crate::release::set_version::{fixture::Fixture, set_version};
use std::fs;

const CARGO: &str = "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\n\n[workspace.package]\nversion = \"0.0.0\"\nedition = \"2021\"\n";

const LOCK: &str = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aaaa\"\n\n[[package]]\nname = \"b\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"c\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bbbb\"\n";

const LOCK_MEMBER_WITHOUT_VERSION: &str =
    "version = 4\n\n[[package]]\nname = \"a\"\n\n[[package]]\nname = \"b\"\nversion = \"0.0.0\"\n";

fn write_repo(root: &Fixture, cargo: &str, lock: &str) {
    fs::write(root.join("Cargo.toml"), cargo).unwrap();
    fs::write(root.join("Cargo.lock"), lock).unwrap();
}

fn read(root: &Fixture, name: &str) -> String {
    fs::read_to_string(root.join(name)).unwrap()
}

#[test]
fn set_version_updates_every_file_and_they_agree() {
    let root = Fixture::new();
    write_repo(&root, CARGO, LOCK);

    set_version(&root, "0.1.0").unwrap();

    assert_eq!(
        read(&root, "Cargo.toml"),
        CARGO.replace("version = \"0.0.0\"", "version = \"0.1.0\"")
    );
    assert_eq!(
        read(&root, "Cargo.lock"),
        LOCK.replace("version = \"0.0.0\"", "version = \"0.1.0\"")
    );
}

#[test]
fn set_version_rejects_invalid_semver() {
    for bad in ["v0.1.0", "0.1", "0.1.0-alpha.1", "0.1.0+b5", ""] {
        let root = Fixture::new();
        write_repo(&root, CARGO, LOCK);

        assert!(set_version(&root, bad).is_err(), "accepted {bad:?}");
        assert_eq!(
            read(&root, "Cargo.toml"),
            CARGO,
            "Cargo.toml changed for {bad:?}"
        );
        assert_eq!(
            read(&root, "Cargo.lock"),
            LOCK,
            "Cargo.lock changed for {bad:?}"
        );
    }
}

#[test]
fn failed_second_write_restores_the_first_file() {
    let root = Fixture::new();
    write_repo(&root, CARGO, LOCK);
    // A directory at the temporary path of Cargo.lock makes the second write fail
    // while both files can still be read.
    fs::create_dir(root.join(".Cargo.lock.tmp")).unwrap();

    let error = set_version(&root, "0.1.0").unwrap_err();

    let text = format!("{error:#}");
    assert!(text.contains("Cargo.lock"), "{text}");
    assert_eq!(read(&root, "Cargo.toml"), CARGO);
    assert_eq!(read(&root, "Cargo.lock"), LOCK);
}

#[test]
fn set_version_fails_when_a_member_block_cannot_be_updated() {
    let root = Fixture::new();
    write_repo(&root, CARGO, LOCK_MEMBER_WITHOUT_VERSION);

    assert!(set_version(&root, "0.1.0").is_err());
    assert_eq!(read(&root, "Cargo.toml"), CARGO);
    assert_eq!(read(&root, "Cargo.lock"), LOCK_MEMBER_WITHOUT_VERSION);
}
