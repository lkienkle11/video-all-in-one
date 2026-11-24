//! Pure rules deciding whether a staged app bundle may replace the running one.
use crate::update::install_error;
use std::path::Path;
use vaio_core::update::UpdateError;

/// All three paths must already be canonical. The staged bundle has to sit directly
/// in a `.vaio-update-*` folder beside the target, and the target has to be the
/// bundle this process is running from.
pub(in crate::update) fn check_paths(
    staged: &Path,
    target: &Path,
    running_bundle: &Path,
) -> Result<(), UpdateError> {
    let root = staged
        .parent()
        .ok_or_else(|| install_error("missing staging root"))?;
    let in_update_folder = root
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".vaio-update-"));
    if running_bundle != target
        || root.parent() != target.parent()
        || !in_update_folder
        || staged == target
    {
        return Err(install_error(
            "refused update paths outside this app's staging directory",
        ));
    }
    Ok(())
}

/// The new bundle must carry the same bundle identifier as the installed one.
pub(in crate::update) fn check_identifiers(
    staged: &[u8],
    target: &[u8],
) -> Result<(), UpdateError> {
    if staged.is_empty() || staged != target {
        return Err(install_error("bundle identifiers differ"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_bundle_beside_the_running_target_is_accepted() {
        check_paths(
            Path::new("/Apps/.vaio-update-1-0/New.app"),
            Path::new("/Apps/Video.app"),
            Path::new("/Apps/Video.app"),
        )
        .unwrap();
    }

    #[test]
    fn target_that_is_not_the_running_bundle_is_refused() {
        assert!(check_paths(
            Path::new("/Apps/.vaio-update-1-0/New.app"),
            Path::new("/Apps/Other.app"),
            Path::new("/Apps/Video.app"),
        )
        .is_err());
    }

    #[test]
    fn staged_path_outside_the_update_folder_is_refused() {
        for staged in [
            "/Apps/staging/New.app",
            "/Apps/New.app",
            "/Elsewhere/.vaio-update-1-0/New.app",
            "/Apps/Video.app",
            "/Apps/.vaio-update-1-0/nested/New.app",
        ] {
            assert!(
                check_paths(
                    Path::new(staged),
                    Path::new("/Apps/Video.app"),
                    Path::new("/Apps/Video.app")
                )
                .is_err(),
                "{staged}"
            );
        }
    }

    #[test]
    fn mismatched_bundle_identifier_is_refused() {
        check_identifiers(b"app.videoallinone\n", b"app.videoallinone\n").unwrap();
        assert!(check_identifiers(b"app.evil\n", b"app.videoallinone\n").is_err());
        assert!(check_identifiers(b"", b"").is_err());
    }
}
