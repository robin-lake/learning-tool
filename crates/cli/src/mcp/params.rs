//! Tool parameter types. Doc comments become the JSON schema descriptions Claude reads.

use learning_core::guide::NewExample;
use learning_core::questions::{LinkKind, NewQuestion, QuestionPatch, QuestionStatus, RatingClass};
use learning_core::references::AttachFiles;
use learning_core::session::ReviewInput;
use learning_core::topics::TopicStatus;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionId {
    pub session_id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkipQuestion {
    pub session_id: i64,
    /// The item_id from next_question.
    pub item_id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordReview {
    pub session_id: i64,
    /// Your grading of the learner's answer.
    pub review: ReviewInput,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RateQuestion {
    pub question_id: i64,
    /// How useful the question is, 1 (useless) to 5 (very useful).
    pub usefulness: u8,
    /// The learner's own words on why.
    #[serde(default)]
    pub feedback: Option<String>,
    /// question_quality (badly made: wording, trivial, wrong level), knowledge_value (not worth knowing), or positive.
    #[serde(default)]
    pub classification: Option<RatingClass>,
    #[serde(default)]
    pub review_id: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Id {
    pub id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTopics {
    /// Show only this topic's subtree.
    #[serde(default)]
    pub root_id: Option<i64>,
    /// How many levels of children to include (counts always cover the whole subtree).
    #[serde(default)]
    pub depth: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTopic {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub target_level: Option<u8>,
    /// "active" or "archived". Archived topics' questions are not scheduled.
    #[serde(default)]
    pub status: Option<TopicStatus>,
    #[serde(default)]
    pub position: Option<i64>,
    /// Move under this parent topic.
    #[serde(default)]
    pub parent_id: Option<i64>,
    /// Move to the top level (ignored if parent_id is given).
    #[serde(default)]
    pub move_to_top_level: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Link {
    pub from: i64,
    pub to: i64,
    /// related (undirected), prerequisite (from must come first), or applies (from is used in to).
    pub kind: LinkKind,
    /// How closely related, 0-1.
    pub weight: f32,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddQuestions {
    pub topic_id: i64,
    pub questions: Vec<NewQuestion>,
    /// "active" (default: scheduled for study) or "proposed" (held for review).
    #[serde(default)]
    pub status: Option<QuestionStatus>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateQuestion {
    pub id: i64,
    /// Changing prompt, answer, rubric, pass_threshold or level creates a new revision (the old wording is retired, its schedule carries over).
    #[serde(flatten)]
    pub patch: QuestionPatch,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetQuestionStatus {
    pub id: i64,
    /// proposed, active, suspended, or retired.
    pub status: QuestionStatus,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachReference {
    /// The reference to add files to.
    pub id: i64,
    #[serde(flatten)]
    pub files: AttachFiles,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchReferences {
    pub query: String,
    /// Limit to references linked to this topic or its subtopics.
    #[serde(default)]
    pub topic_id: Option<i64>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadReference {
    pub id: i64,
    /// Start at this locator, e.g. "p. 12" or "Horns > Throat".
    #[serde(default)]
    pub from_locator: Option<String>,
    #[serde(default)]
    pub max_chunks: Option<u32>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRatings {
    /// Only ratings not yet used by a guide version (default true).
    #[serde(default = "yes")]
    pub unprocessed_only: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClassifyRating {
    pub rating_id: i64,
    pub classification: RatingClass,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveGuideVersion {
    /// The full new guide (Markdown).
    pub content: String,
    /// What changed and why.
    pub change_summary: String,
    #[serde(default)]
    pub examples: Vec<NewExample>,
    /// Ratings this version accounts for.
    #[serde(default)]
    pub processed_rating_ids: Vec<i64>,
    /// Questions whose examples should no longer be carried forward (current examples are kept by default).
    #[serde(default)]
    pub removed_example_question_ids: Vec<i64>,
}
