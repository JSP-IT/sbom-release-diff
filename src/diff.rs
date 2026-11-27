//! Deterministic pairing of components between two documents.
//!
//! Components are grouped by their version-independent identity key. Within
//! a group, entries are paired by exact version first. Whatever remains is
//! paired only when each side had a single entry for that identity;
//! otherwise the group is reported as ambiguous instead of guessed.

use std::collections::{BTreeMap, BTreeSet};

use crate::cyclonedx::{Bom, Component, IdentityBasis, NoteKind};
use crate::report::{
    Ambiguity, AmbiguityReason, ComponentRef, LicenseChange, Side, VersionChange, Warning,
    WarningKind,
};

#[derive(Debug, Default)]
pub struct Changes {
    pub added: Vec<ComponentRef>,
    pub removed: Vec<ComponentRef>,
    pub version_changed: Vec<VersionChange>,
    pub license_changed: Vec<LicenseChange>,
    pub ambiguous: Vec<Ambiguity>,
    pub warnings: Vec<Warning>,
    /// Paired components with neither a version nor a license change.
    pub unchanged: usize,
}

/// Compares two documents. The result does not depend on the order of
/// components, license entries or object keys in either input.
pub fn compare(old: &Bom, new: &Bom) -> Changes {
    let mut work = Work::default();
    for (side, bom) in [(Side::Old, old), (Side::New, new)] {
        for note in &bom.notes {
            work.warnings.push(Warning {
                kind: match note.kind {
                    NoteKind::PurlVersionMismatch => WarningKind::PurlVersionMismatch,
                    NoteKind::UnparsedLicenseExpression => WarningKind::UnparsedLicenseExpression,
                },
                side,
                identity: note.identity.clone(),
                version: note.version.clone(),
                count: None,
            });
        }
    }

    let old_groups = group(old, Side::Old, &mut work.warnings);
    let new_groups = group(new, Side::New, &mut work.warnings);
    let keys: BTreeSet<&str> = old_groups
        .keys()
        .chain(new_groups.keys())
        .copied()
        .collect();
    let empty = Vec::new();
    for key in keys {
        let old_entries = old_groups.get(key).unwrap_or(&empty);
        let new_entries = new_groups.get(key).unwrap_or(&empty);
        work.compare_group(key, old_entries, new_entries);
    }
    work.flag_identity_basis_changes();
    work.finish()
}

type SortKey<'a> = (
    Option<&'a str>,
    Vec<&'a str>,
    &'a str,
    Option<&'a str>,
    &'a str,
);

fn sort_key(c: &Component) -> SortKey<'_> {
    (
        c.version.as_deref(),
        c.license_keys(),
        c.name.as_str(),
        c.group.as_deref(),
        c.component_type.as_str(),
    )
}

/// Groups components by identity key, sorted, with identical entries merged.
fn group<'a>(
    bom: &'a Bom,
    side: Side,
    warnings: &mut Vec<Warning>,
) -> BTreeMap<&'a str, Vec<&'a Component>> {
    let mut groups: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
    for component in &bom.components {
        groups
            .entry(component.identity.key.as_str())
            .or_default()
            .push(component);
    }
    for (key, entries) in &mut groups {
        entries.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
        let mut merged: Vec<(&Component, usize)> = Vec::with_capacity(entries.len());
        for entry in entries.drain(..) {
            match merged.last_mut() {
                Some((last, count)) if sort_key(last) == sort_key(entry) => *count += 1,
                _ => merged.push((entry, 1)),
            }
        }
        for (entry, count) in &merged {
            if *count > 1 {
                warnings.push(Warning {
                    kind: WarningKind::DuplicateComponent,
                    side,
                    identity: (*key).to_owned(),
                    version: entry.version.clone(),
                    count: Some(*count),
                });
            }
        }
        *entries = merged.into_iter().map(|(entry, _)| entry).collect();
    }
    groups
}

fn by_version<'a>(entries: &[&'a Component]) -> BTreeMap<Option<&'a str>, Vec<&'a Component>> {
    let mut map: BTreeMap<Option<&str>, Vec<&Component>> = BTreeMap::new();
    for entry in entries {
        map.entry(entry.version.as_deref()).or_default().push(entry);
    }
    map
}

fn refs(entries: &[&Component]) -> Vec<ComponentRef> {
    let mut refs: Vec<ComponentRef> = entries.iter().map(|c| ComponentRef::from(*c)).collect();
    refs.sort();
    refs
}

#[derive(Default)]
struct Work<'a> {
    added: Vec<&'a Component>,
    removed: Vec<&'a Component>,
    version_changed: Vec<VersionChange>,
    license_changed: Vec<LicenseChange>,
    ambiguous: Vec<Ambiguity>,
    warnings: Vec<Warning>,
    unchanged: usize,
}

impl<'a> Work<'a> {
    fn compare_group(&mut self, key: &str, old: &[&'a Component], new: &[&'a Component]) {
        if old.is_empty() {
            self.added.extend_from_slice(new);
            return;
        }
        if new.is_empty() {
            self.removed.extend_from_slice(old);
            return;
        }

        let several_entries = old.len() > 1 || new.len() > 1;
        let old_versions = by_version(old);
        let new_versions = by_version(new);
        let versions: BTreeSet<Option<&str>> = old_versions
            .keys()
            .chain(new_versions.keys())
            .copied()
            .collect();

        let mut old_left = Vec::new();
        let mut new_left = Vec::new();
        for version in versions {
            match (old_versions.get(&version), new_versions.get(&version)) {
                (Some(o), Some(n)) if o.len() == 1 && n.len() == 1 => self.pair(o[0], n[0]),
                (Some(o), Some(n)) => self.ambiguous.push(Ambiguity {
                    identity: key.to_owned(),
                    reason: AmbiguityReason::DuplicateVersionDiffers,
                    old: refs(o),
                    new: refs(n),
                }),
                (Some(o), None) => old_left.extend_from_slice(o),
                (None, Some(n)) => new_left.extend_from_slice(n),
                (None, None) => {}
            }
        }

        match (old_left.len(), new_left.len()) {
            (0, _) => self.added.extend(new_left),
            (_, 0) => self.removed.extend(old_left),
            (1, 1) if !several_entries => self.pair(old_left[0], new_left[0]),
            _ => self.ambiguous.push(Ambiguity {
                identity: key.to_owned(),
                reason: AmbiguityReason::MultipleCandidates,
                old: refs(&old_left),
                new: refs(&new_left),
            }),
        }
    }

    fn pair(&mut self, old: &Component, new: &Component) {
        let version_changed = old.version != new.version;
        let license_changed = old.license_keys() != new.license_keys();
        if version_changed {
            self.version_changed.push(VersionChange {
                identity: new.identity.key.clone(),
                identity_basis: new.identity.basis,
                component_type: new.component_type.clone(),
                name: new.name.clone(),
                group: new.group.clone(),
                old_version: old.version.clone(),
                new_version: new.version.clone(),
            });
        }
        if license_changed {
            self.license_changed.push(LicenseChange {
                identity: new.identity.key.clone(),
                identity_basis: new.identity.basis,
                component_type: new.component_type.clone(),
                name: new.name.clone(),
                group: new.group.clone(),
                old_version: old.version.clone(),
                new_version: new.version.clone(),
                old_licenses: old.licenses.iter().map(|l| l.display.clone()).collect(),
                new_licenses: new.licenses.iter().map(|l| l.display.clone()).collect(),
            });
        }
        if !version_changed && !license_changed {
            self.unchanged += 1;
        }
    }

    /// A component that has a purl in one document but not in the other
    /// gets different identity keys and would show up as removed plus
    /// added. When a removed and an added component share a group and name
    /// and at least one of them lacks a purl, report them as ambiguous.
    fn flag_identity_basis_changes(&mut self) {
        let mut by_loose: BTreeMap<&str, (Vec<&'a Component>, Vec<&'a Component>)> =
            BTreeMap::new();
        for c in &self.removed {
            by_loose
                .entry(c.identity.loose.as_str())
                .or_default()
                .0
                .push(c);
        }
        for c in &self.added {
            by_loose
                .entry(c.identity.loose.as_str())
                .or_default()
                .1
                .push(c);
        }

        let mut flagged = BTreeSet::new();
        for (loose, (removed, added)) in &by_loose {
            let has_fallback = removed
                .iter()
                .chain(added)
                .any(|c| c.identity.basis == IdentityBasis::Fallback);
            if !removed.is_empty() && !added.is_empty() && has_fallback {
                self.ambiguous.push(Ambiguity {
                    identity: format!("name:{loose}"),
                    reason: AmbiguityReason::IdentityBasisDiffers,
                    old: refs(removed),
                    new: refs(added),
                });
                flagged.insert(loose.to_string());
            }
        }
        self.removed
            .retain(|c| !flagged.contains(c.identity.loose.as_str()));
        self.added
            .retain(|c| !flagged.contains(c.identity.loose.as_str()));
    }

    fn finish(self) -> Changes {
        let mut changes = Changes {
            added: refs(&self.added),
            removed: refs(&self.removed),
            version_changed: self.version_changed,
            license_changed: self.license_changed,
            ambiguous: self.ambiguous,
            warnings: self.warnings,
            unchanged: self.unchanged,
        };
        changes.version_changed.sort();
        changes.license_changed.sort();
        changes.ambiguous.sort();
        changes.warnings.sort();
        // Merged duplicates can repeat the same parse note.
        changes.warnings.dedup();
        changes
    }
}
