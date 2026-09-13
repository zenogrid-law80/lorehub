use std::{cmp::Ordering, fmt, str::FromStr};

use anyhow::{Result, bail, ensure};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
    pre_release: Vec<Identifier>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Identifier {
    Numeric(u64),
    Text(String),
}

impl FromStr for SemanticVersion {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        ensure!(!value.contains('+'), "build metadata is not supported");
        let (core, pre_release) = value
            .split_once('-')
            .map_or((value, None), |(core, pre)| (core, Some(pre)));
        let mut parts = core.split('.');
        let major = parse_number(parts.next(), "major")?;
        let minor = parse_number(parts.next(), "minor")?;
        let patch = parse_number(parts.next(), "patch")?;
        ensure!(
            parts.next().is_none(),
            "version must contain major.minor.patch"
        );
        let pre_release = match pre_release {
            None => Vec::new(),
            Some("") => bail!("pre-release version cannot be empty"),
            Some(value) => value
                .split('.')
                .map(|identifier| {
                    ensure!(
                        !identifier.is_empty(),
                        "pre-release identifier cannot be empty"
                    );
                    ensure!(
                        identifier
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
                        "pre-release identifier contains invalid characters"
                    );
                    if identifier.bytes().all(|byte| byte.is_ascii_digit()) {
                        ensure!(
                            identifier == "0" || !identifier.starts_with('0'),
                            "numeric pre-release identifier cannot have a leading zero"
                        );
                        Ok(Identifier::Numeric(identifier.parse()?))
                    } else {
                        Ok(Identifier::Text(identifier.to_owned()))
                    }
                })
                .collect::<Result<Vec<_>>>()?,
        };
        Ok(Self {
            major,
            minor,
            patch,
            pre_release,
        })
    }
}

fn parse_number(value: Option<&str>, name: &str) -> Result<u64> {
    let value = value.ok_or_else(|| anyhow::anyhow!("version is missing {name}"))?;
    ensure!(!value.is_empty(), "version {name} cannot be empty");
    ensure!(
        value == "0" || !value.starts_with('0'),
        "version {name} cannot have a leading zero"
    );
    Ok(value.parse()?)
}

impl Ord for SemanticVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| compare_pre_release(&self.pre_release, &other.pre_release))
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_pre_release(left: &[Identifier], right: &[Identifier]) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    for (left, right) in left.iter().zip(right) {
        let ordering = match (left, right) {
            (Identifier::Numeric(left), Identifier::Numeric(right)) => left.cmp(right),
            (Identifier::Numeric(_), Identifier::Text(_)) => Ordering::Less,
            (Identifier::Text(_), Identifier::Numeric(_)) => Ordering::Greater,
            (Identifier::Text(left), Identifier::Text(right)) => left.cmp(right),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre_release.is_empty() {
            formatter.write_str("-")?;
            for (index, identifier) in self.pre_release.iter().enumerate() {
                if index > 0 {
                    formatter.write_str(".")?;
                }
                match identifier {
                    Identifier::Numeric(value) => write!(formatter, "{value}")?,
                    Identifier::Text(value) => formatter.write_str(value)?,
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_version_precedence_is_supported() {
        let versions = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in versions.windows(2) {
            assert!(
                pair[0].parse::<SemanticVersion>().unwrap()
                    < pair[1].parse::<SemanticVersion>().unwrap()
            );
        }
    }

    #[test]
    fn invalid_versions_are_rejected() {
        for value in ["1", "1.2", "01.2.3", "1.2.3-", "1.2.3-01", "1.2.3+dev"] {
            assert!(value.parse::<SemanticVersion>().is_err(), "{value}");
        }
    }
}
