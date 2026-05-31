//! Registry ACL: org ownership, package ownership, and collaborator grants.

use std::collections::HashMap;

use crate::manifest::{OrgEntry, RegistryEntry};

/// Parse `@org/package` into (org_slug, package_name).
pub fn parse_scoped_package_name(full_name: &str) -> Result<(String, String), String> {
    let rest = full_name
        .strip_prefix('@')
        .ok_or_else(|| format!(
            "Package name must be scoped as @org/name (got '{}'). \
             Create an org with: epm org create <org>",
            full_name
        ))?;
    let slash = rest.find('/').ok_or_else(|| format!(
        "Package name must be scoped as @org/name (got '{}')",
        full_name
    ))?;
    let org = rest[..slash].to_string();
    let pkg = rest[slash + 1..].to_string();
    if org.is_empty() || pkg.is_empty() {
        return Err(format!("Invalid scoped name '{}'", full_name));
    }
    validate_slug(&org, "org")?;
    validate_slug(&pkg, "package")?;
    Ok((org, pkg))
}

pub fn format_scoped_name(org: &str, package: &str) -> String {
    format!("@{}/{}", org, package)
}

pub fn validate_slug(slug: &str, kind: &str) -> Result<(), String> {
    if slug.len() < 1 || slug.len() > 64 {
        return Err(format!("{} name must be 1–64 characters", kind));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "{} name '{}' may only contain letters, numbers, hyphens, and underscores",
            kind, slug
        ));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(format!("{} name '{}' cannot start or end with '-'", kind, slug));
    }
    Ok(())
}

/// Whether `login` owns the org.
pub fn is_org_owner(orgs: &HashMap<String, OrgEntry>, org: &str, login: &str) -> bool {
    orgs.get(org)
        .map(|o| o.owner.eq_ignore_ascii_case(login))
        .unwrap_or(false)
}

/// Whether `login` may publish this package version.
pub fn can_publish(login: &str, entry: &RegistryEntry) -> bool {
    if entry.owner.eq_ignore_ascii_case(login) {
        return true;
    }
    entry
        .collaborators
        .get(login)
        .map(|r| r == "publish")
        .unwrap_or(false)
}

/// Only the package owner may add or change collaborators.
pub fn can_grant(login: &str, entry: &RegistryEntry) -> bool {
    !entry.owner.is_empty() && entry.owner.eq_ignore_ascii_case(login)
}

/// First-time publish: caller must own the org; becomes package owner.
pub fn can_create_package(
    login: &str,
    org: &str,
    orgs: &HashMap<String, OrgEntry>,
) -> Result<(), String> {
    let org_entry = orgs
        .get(org)
        .ok_or_else(|| format!("Org '@{}' does not exist. Create it with: epm org create {}", org, org))?;
    if !org_entry.owner.eq_ignore_ascii_case(login) {
        return Err(format!(
            "Only @{} owner @{} can publish new packages under this org (you are @{}).",
            org, org_entry.owner, login
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_scoped() {
        let (o, p) = parse_scoped_package_name("@acme/widget").unwrap();
        assert_eq!(o, "acme");
        assert_eq!(p, "widget");
    }

    #[test]
    fn grant_only_owner() {
        let entry = RegistryEntry {
            name: "@acme/a".into(),
            org: Some("acme".into()),
            owner: "alice".into(),
            collaborators: HashMap::new(),
            description: None,
            author: None,
            license: None,
            repository: None,
            versions: vec![],
        };
        assert!(can_grant("alice", &entry));
        assert!(!can_grant("bob", &entry));
    }
}
