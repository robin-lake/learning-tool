use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

fn one() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RubricPoint {
    /// Short stable id, unique within the rubric (e.g. "r1").
    pub id: String,
    /// What a correct answer must contain or demonstrate.
    pub point: String,
    /// Relative weight (default 1).
    #[serde(default = "one")]
    pub weight: f32,
    /// If true, missing this point fails the answer regardless of score.
    #[serde(default)]
    pub essential: bool,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(transparent)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, as = "Vec<RubricPoint>"))]
pub struct Rubric(pub Vec<RubricPoint>);

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PointResult {
    /// The rubric point id.
    pub id: String,
    /// Whether the answer met this point.
    pub hit: bool,
    /// Optional note on what was right or missing.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub score: f32,
    pub essentials_met: bool,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Grade {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Grade {
    pub fn from_i64(v: i64) -> Result<Grade> {
        match v {
            1 => Ok(Grade::Again),
            2 => Ok(Grade::Hard),
            3 => Ok(Grade::Good),
            4 => Ok(Grade::Easy),
            _ => Err(Error::Invalid(format!("grade must be 1-4, got {v}"))),
        }
    }
}

pub fn validate_rubric(r: &Rubric, pass_threshold: f32) -> Result<()> {
    if r.0.is_empty() {
        return Err(Error::Invalid("rubric must have at least one point".into()));
    }
    let mut seen = HashSet::new();
    for p in &r.0 {
        if p.id.trim().is_empty() {
            return Err(Error::Invalid("rubric point ids must not be empty".into()));
        }
        if !seen.insert(p.id.as_str()) {
            return Err(Error::Invalid(format!("duplicate rubric point id {:?}", p.id)));
        }
        if p.point.trim().is_empty() {
            return Err(Error::Invalid(format!("rubric point {:?} has no text", p.id)));
        }
        if !(p.weight > 0.0) {
            return Err(Error::Invalid(format!("rubric point {:?} must have weight > 0", p.id)));
        }
    }
    if !(pass_threshold > 0.0 && pass_threshold <= 1.0) {
        return Err(Error::Invalid(format!("pass_threshold must be in (0, 1], got {pass_threshold}")));
    }
    Ok(())
}

/// Share of rubric weight hit. `results` must cover exactly the rubric's point ids.
pub fn score(r: &Rubric, results: &[PointResult]) -> Result<Score> {
    let expected: HashSet<&str> = r.0.iter().map(|p| p.id.as_str()).collect();
    let mut given = HashSet::new();
    for res in results {
        if !expected.contains(res.id.as_str()) {
            return Err(Error::Invalid(format!("unknown rubric point id {:?}", res.id)));
        }
        if !given.insert(res.id.as_str()) {
            return Err(Error::Invalid(format!("rubric point {:?} graded twice", res.id)));
        }
    }
    if let Some(missing) = r.0.iter().find(|p| !given.contains(p.id.as_str())) {
        return Err(Error::Invalid(format!("rubric point {:?} was not graded", missing.id)));
    }
    let hit = |id: &str| results.iter().any(|res| res.id == id && res.hit);
    let total: f32 = r.0.iter().map(|p| p.weight).sum();
    // `+ 0.0` turns the -0.0 that an empty float sum produces into 0.0.
    let got: f32 = r.0.iter().filter(|p| hit(&p.id)).map(|p| p.weight).sum::<f32>() + 0.0;
    let all_hit = r.0.iter().all(|p| hit(&p.id));
    Ok(Score {
        score: if all_hit { 1.0 } else { (got / total).clamp(0.0, 1.0) },
        essentials_met: r.0.iter().filter(|p| p.essential).all(|p| hit(&p.id)),
    })
}

/// Maps a rubric score to an FSRS grade. Rules apply top to bottom (see the design doc).
pub fn grade(s: &Score, pass_threshold: f32, hints_used: u32, fluent: bool) -> Grade {
    if !s.essentials_met || s.score < pass_threshold {
        Grade::Again
    } else if s.score >= 1.0 && hints_used == 0 && fluent {
        Grade::Easy
    } else if hints_used > 0 || s.score < pass_threshold + (1.0 - pass_threshold) / 2.0 {
        Grade::Hard
    } else {
        Grade::Good
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    fn rubric() -> Rubric {
        Rubric(vec![
            RubricPoint { id: "r1".into(), point: "a".into(), weight: 1.0, essential: true },
            RubricPoint { id: "r2".into(), point: "b".into(), weight: 1.0, essential: false },
            RubricPoint { id: "r3".into(), point: "c".into(), weight: 2.0, essential: false },
        ])
    }
    fn hits(h: [bool; 3]) -> Vec<PointResult> {
        ["r1", "r2", "r3"]
            .iter()
            .zip(h)
            .map(|(id, hit)| PointResult { id: id.to_string(), hit, note: String::new() })
            .collect()
    }
    fn g(h: [bool; 3], hints: u32, fluent: bool) -> Grade {
        let s = score(&rubric(), &hits(h)).unwrap();
        grade(&s, 0.6, hints, fluent)
    }

    #[test]
    fn grade_missing_essential_is_again() {
        assert_eq!(g([false, true, true], 0, true), Grade::Again);
    }
    #[test]
    fn grade_below_threshold_is_again() {
        assert_eq!(g([true, false, false], 0, false), Grade::Again);
    }
    #[test]
    fn grade_hint_is_hard() {
        assert_eq!(g([true, true, true], 1, true), Grade::Hard);
    }
    #[test]
    fn grade_low_pass_is_hard() {
        let s = score(&rubric(), &hits([true, false, true])).unwrap();
        assert_eq!(s.score, 0.75);
        assert_eq!(grade(&s, 0.6, 0, false), Grade::Hard);
    }
    #[test]
    fn grade_solid_pass_is_good() {
        assert_eq!(g([true, true, true], 0, false), Grade::Good);
    }
    #[test]
    fn grade_perfect_fluent_is_easy() {
        assert_eq!(g([true, true, true], 0, true), Grade::Easy);
    }
    #[test]
    fn grade_exactly_at_threshold_passes() {
        // "Pass at 60%" includes 60%: found by cargo-mutants (`<` → `<=` survived).
        let s = Score { score: 0.6, essentials_met: true };
        assert_eq!(grade(&s, 0.6, 0, false), Grade::Hard);
    }
    #[test]
    fn grade_boundary_at_midpoint_is_good() {
        let s = Score { score: 0.8, essentials_met: true };
        assert_eq!(grade(&s, 0.6, 0, false), Grade::Good);
    }
    #[test]
    fn perfect_score_is_exactly_one() {
        let r = Rubric(vec![
            RubricPoint { id: "a".into(), point: "a".into(), weight: 0.1, essential: false },
            RubricPoint { id: "b".into(), point: "b".into(), weight: 0.2, essential: false },
        ]);
        let res = vec![
            PointResult { id: "a".into(), hit: true, note: String::new() },
            PointResult { id: "b".into(), hit: true, note: String::new() },
        ];
        assert_eq!(score(&r, &res).unwrap().score, 1.0);
    }
    #[test]
    fn nothing_hit_scores_positive_zero() {
        // Summing an empty float iterator gives -0.0, which displays as "-0%".
        let s = score(&rubric(), &hits([false, false, false])).unwrap();
        assert!(s.score == 0.0 && s.score.is_sign_positive(), "{:?}", s.score);
    }

    #[test]
    fn score_rejects_missing_or_extra_ids() {
        let mut res = hits([true, true, true]);
        res.pop();
        assert!(matches!(score(&rubric(), &res), Err(Error::Invalid(_))));
        let mut res = hits([true, true, true]);
        res.push(PointResult { id: "zz".into(), hit: true, note: String::new() });
        assert!(matches!(score(&rubric(), &res), Err(Error::Invalid(_))));
        let mut res = hits([true, true, true]);
        res[2].id = "r1".into();
        assert!(matches!(score(&rubric(), &res), Err(Error::Invalid(_))));
    }
    #[test]
    fn validate_rubric_rejects_dupes_empty_zero_weight_bad_threshold() {
        assert!(validate_rubric(&rubric(), 0.6).is_ok());
        assert!(validate_rubric(&Rubric(vec![]), 0.6).is_err());
        let mut r = rubric();
        r.0[1].id = "r1".into();
        assert!(validate_rubric(&r, 0.6).is_err());
        let mut r = rubric();
        r.0[0].id = " ".into();
        assert!(validate_rubric(&r, 0.6).is_err());
        let mut r = rubric();
        r.0[0].point = "".into();
        assert!(validate_rubric(&r, 0.6).is_err());
        let mut r = rubric();
        r.0[0].weight = 0.0;
        assert!(validate_rubric(&r, 0.6).is_err());
        assert!(validate_rubric(&rubric(), 0.0).is_err());
        assert!(validate_rubric(&rubric(), 1.01).is_err());
        assert!(validate_rubric(&rubric(), 1.0).is_ok());
    }
    #[test]
    fn rubric_json_defaults() {
        let r: Rubric = serde_json::from_str(r#"[{"id":"r1","point":"x"}]"#).unwrap();
        assert_eq!(r.0[0].weight, 1.0);
        assert!(!r.0[0].essential);
    }
}
