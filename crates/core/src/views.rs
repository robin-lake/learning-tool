//! Page-shaped read models for the web app. List views never carry answers or rubrics.

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct TopicRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct QuestionRef {
    pub id: i64,
    pub prompt: String,
    pub topic_path: Vec<String>,
}
