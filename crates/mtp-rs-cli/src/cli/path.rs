use super::error::{CliError, CliErrorKind};
use mtp_rs::{ObjectHandle, ObjectInfo, Storage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePath {
    raw: String,
    components: Vec<String>,
    trailing_slash: bool,
}

#[derive(Debug, Clone)]
pub enum ExistingRemote {
    Root,
    Object(ObjectInfo),
}

#[derive(Debug, Clone)]
pub struct UploadTarget {
    pub parent: Option<ObjectHandle>,
    pub filename: String,
    pub existing: Option<ObjectInfo>,
}

impl RemotePath {
    pub fn parse(path: &str) -> Result<Self, CliError> {
        if path.is_empty() || !path.starts_with('/') {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "remote path must be absolute, for example /Music/song.mp3",
            ));
        }

        let trailing_slash = path != "/" && path.ends_with('/');
        let mut components = Vec::new();
        for component in path.split('/').filter(|part| !part.is_empty()) {
            validate_component(component)?;
            components.push(component.to_string());
        }

        Ok(Self {
            raw: path.to_string(),
            components,
            trailing_slash,
        })
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    #[must_use]
    pub fn trailing_slash(&self) -> bool {
        self.trailing_slash
    }
}

pub fn validate_component(component: &str) -> Result<(), CliError> {
    if component.is_empty() || component == "." || component == ".." {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            format!("invalid remote path component '{component}'"),
        ));
    }

    if component.contains('/') || component.contains('\\') || component.contains('\0') {
        return Err(CliError::new(
            CliErrorKind::RemotePath,
            format!("invalid remote filename '{component}'"),
        ));
    }

    Ok(())
}

pub async fn resolve_existing(
    storage: &Storage,
    path: &RemotePath,
    verbose: bool,
) -> Result<ExistingRemote, CliError> {
    if path.is_root() {
        return Ok(ExistingRemote::Root);
    }

    let mut parent = None;
    let mut found = None;
    for (index, component) in path.components().iter().enumerate() {
        let objects = storage
            .list_objects(parent)
            .await
            .map_err(|e| CliError::from_mtp("list remote folder", e, verbose))?;
        let object = objects
            .into_iter()
            .find(|object| object.filename == component.as_str())
            .ok_or_else(|| {
                CliError::new(
                    CliErrorKind::RemotePath,
                    format!("remote path not found: {}", path.raw()),
                )
            })?;

        let is_last = index + 1 == path.components().len();
        if !is_last && !object.is_folder() {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                format!("remote parent is not a folder: {}", object.filename),
            ));
        }

        parent = Some(object.handle);
        found = Some(object);
    }

    Ok(ExistingRemote::Object(
        found.expect("non-root path has an object"),
    ))
}

pub async fn resolve_upload_target(
    storage: &Storage,
    remote_path: &RemotePath,
    local_filename: &str,
    verbose: bool,
) -> Result<UploadTarget, CliError> {
    if remote_path.is_root() {
        return Ok(UploadTarget {
            parent: None,
            filename: local_filename.to_string(),
            existing: None,
        });
    }

    let mut components = remote_path.components().to_vec();
    let explicit_folder = remote_path.trailing_slash();
    let final_name = if explicit_folder {
        local_filename.to_string()
    } else {
        components.pop().expect("non-root path has final component")
    };
    validate_component(&final_name)?;

    let parent_path = RemotePath {
        raw: parent_raw_from_components(&components),
        components,
        trailing_slash: false,
    };
    let parent = match resolve_existing(storage, &parent_path, verbose).await? {
        ExistingRemote::Root => None,
        ExistingRemote::Object(object) if object.is_folder() => Some(object.handle),
        ExistingRemote::Object(_) => {
            return Err(CliError::new(
                CliErrorKind::RemotePath,
                "remote parent is not a folder",
            ));
        }
    };

    let objects = storage
        .list_objects(parent)
        .await
        .map_err(|e| CliError::from_mtp("list remote folder", e, verbose))?;
    if let Some(existing_folder) = objects
        .iter()
        .find(|object| object.filename == final_name.as_str() && object.is_folder())
    {
        return Ok(UploadTarget {
            parent: Some(existing_folder.handle),
            filename: local_filename.to_string(),
            existing: None,
        });
    }

    let existing = objects
        .into_iter()
        .find(|object| object.filename == final_name.as_str() && object.is_file());

    Ok(UploadTarget {
        parent,
        filename: final_name,
        existing,
    })
}

fn parent_raw_from_components(components: &[String]) -> String {
    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root() {
        let path = RemotePath::parse("/").unwrap();
        assert!(path.is_root());
        assert!(!path.trailing_slash());
    }

    #[test]
    fn rejects_relative_paths() {
        assert!(RemotePath::parse("Music/song.mp3").is_err());
    }

    #[test]
    fn tracks_trailing_slash_destination() {
        let path = RemotePath::parse("/Music/").unwrap();
        assert_eq!(path.components(), vec!["Music".to_string()].as_slice());
        assert!(path.trailing_slash());
    }

    #[test]
    fn rejects_invalid_components() {
        assert!(RemotePath::parse("/../x").is_err());
        assert!(RemotePath::parse("/bad\\name").is_err());
        assert!(RemotePath::parse("/bad\0name").is_err());
    }

    #[test]
    fn validates_upload_filename() {
        assert!(validate_component("app.prg").is_ok());
        assert!(validate_component("../app.prg").is_err());
        assert!(validate_component("bad\\app.prg").is_err());
    }
}
