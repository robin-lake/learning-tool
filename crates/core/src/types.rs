//! Small shared types: string-backed enums stored as lowercase text, and timestamp helpers.

use chrono::{DateTime, NaiveDate, Utc};

use crate::{Error, Result};

/// Defines an enum stored as lowercase text in SQLite and serialized the same way in JSON.
macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        pub enum $name {
            $(#[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }

            pub fn parse(s: &str) -> $crate::Result<$name> {
                match s {
                    $($text => Ok($name::$variant),)+
                    other => Err($crate::Error::Invalid(format!(
                        concat!("invalid ", stringify!($name), " {:?}"), other
                    ))),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}
pub(crate) use str_enum;

str_enum!(
    /// Whether an item belongs to the long-term main program or to a project.
    Scope { Main => "main", Project => "project" }
);

str_enum!(
    /// How two topics (or two questions) relate.
    LinkKind { Related => "related", Prerequisite => "prerequisite", Applies => "applies" }
);

pub(crate) fn ts(now: DateTime<Utc>) -> String {
    now.to_rfc3339()
}

pub(crate) fn parse_ts(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| Error::Invalid(format!("bad timestamp {s:?}: {e}")))
}

pub(crate) fn parse_day(s: &str) -> Result<NaiveDate> {
    s.parse().map_err(|e| Error::Invalid(format!("bad date {s:?}: {e}")))
}

/// Maps a unique-constraint violation to `Error::Conflict`; other errors pass through.
pub(crate) fn conflict_on_unique(e: sqlx::Error, msg: impl FnOnce() -> String) -> Error {
    match e.as_database_error() {
        Some(d) if d.is_unique_violation() => Error::Conflict(msg()),
        _ => Error::Db(e),
    }
}

pub(crate) fn validate_level(level: u8) -> Result<()> {
    if level > 3 {
        return Err(Error::Invalid(format!("level must be 0-3, got {level}")));
    }
    Ok(())
}

pub(crate) fn validate_weight(weight: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&weight) {
        return Err(Error::Invalid(format!("link weight must be between 0 and 1, got {weight}")));
    }
    Ok(())
}
