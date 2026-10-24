use chrono::Utc;
use learning_core::questions::QuestionStatus;
use learning_core::topics::{NewTopic, TopicPatch};
use learning_core::references::NewSource;
use learning_core::{guide, questions, references, session, stats, topics};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData as McpError, tool, tool_router};

use super::params::*;
use super::{LearningServer, respond};

type ToolResult = Result<CallToolResult, McpError>;

#[tool_router(vis = "pub(crate)")]
impl LearningServer {
    // ---- Study ----

    #[tool(description = "Start (or resume) today's main study session. Returns counts and the session id; call next_question to get questions.")]
    async fn start_session(&self) -> ToolResult {
        let s = self.settings().await?;
        respond(session::start(&self.pool, &s, Utc::now()).await, |r| {
            format!(
                "Session {}: {} to answer ({} due, {} new allowed, {} carried over). Streak {}.",
                r.session_id, r.planned, r.due_total, r.new_allowed, r.backlog_carried, r.streak
            )
        })
    }

    #[tool(description = "Get the next question in a session, or null when the session is done. The rubric and answer are for grading only — never show them to the user before they answer. Calling again before record_review returns the same question.")]
    async fn next_question(&self, Parameters(p): Parameters<SessionId>) -> ToolResult {
        respond(session::next_question(&self.pool, p.session_id, Utc::now()).await, |r| match r {
            Some(q) => format!("Question {} of session ({} remaining, level L{}).", q.position, q.remaining, q.level),
            None => "Session complete: no questions left. Call end_session.".into(),
        })
    }

    #[tool(description = "Skip a question the learner doesn't want to answer now: drops it from this session without recording a review. It stays scheduled and comes back in a later session.")]
    async fn skip_question(&self, Parameters(p): Parameters<SkipQuestion>) -> ToolResult {
        respond(session::skip(&self.pool, p.session_id, p.item_id).await, |_| {
            format!("Skipped item {}; it will come back in a later session.", p.item_id)
        })
    }

    #[tool(description = "Record your grading of the learner's answer. Pass the item_id and question_id from next_question, one rubric_result entry per rubric point (hit or not), hints_used, and fluent (immediate, complete recall). The grade and next due date are computed for you. Recording the same item twice is rejected.")]
    async fn record_review(&self, Parameters(p): Parameters<RecordReview>) -> ToolResult {
        let s = self.settings().await?;
        respond(session::record_review(&self.pool, &s, p.session_id, p.review, Utc::now()).await, |r| {
            let mut line = format!("Graded {:?} (score {:.0}%), next due {}.", r.grade, r.score * 100.0, r.due_on);
            if r.requeued {
                line.push_str(" Will be asked again later this session.");
            }
            if r.is_leech {
                line.push_str(" This question is a leech: offer to rewrite it.");
            }
            line
        })
    }

    #[tool(description = "End a session and get its summary (grades, streak, tomorrow's load, leeches).")]
    async fn end_session(&self, Parameters(p): Parameters<SessionId>) -> ToolResult {
        let s = self.settings().await?;
        respond(session::end(&self.pool, &s, p.session_id, Utc::now()).await, |r| {
            format!("Answered {}. Streak {}. {} due tomorrow.", r.answered, r.streak, r.due_tomorrow)
        })
    }

    #[tool(description = "Record the learner's usefulness rating (1-5) for a question, with their feedback and your classification of it.")]
    async fn rate_question(&self, Parameters(p): Parameters<RateQuestion>) -> ToolResult {
        let r = guide::rate(&self.pool, p.question_id, p.usefulness, p.feedback, p.classification, p.review_id, Utc::now()).await;
        respond(r, |id| format!("Rating {id} recorded."))
    }

    #[tool(description = "Today's numbers: answered, remaining, new introduced, due tomorrow, streak.")]
    async fn today_stats(&self) -> ToolResult {
        let s = self.settings().await?;
        respond(stats::today(&self.pool, &s, Utc::now()).await, |r| {
            format!("Answered {} today, {} remaining, streak {}.", r.answered_today, r.remaining_today, r.streak)
        })
    }

    // ---- Topics and questions ----

    #[tool(description = "The topic tree with active-question counts by level, average recall, and target level.")]
    async fn list_topics(&self, Parameters(p): Parameters<ListTopics>) -> ToolResult {
        respond(topics::tree(&self.pool, p.root_id, p.depth, Utc::now()).await, |r| format!("{} top-level topics.", r.len()))
    }

    #[tool(description = "A topic's details: path, children, links, references and question ids.")]
    async fn get_topic(&self, Parameters(p): Parameters<Id>) -> ToolResult {
        respond(topics::get(&self.pool, p.id).await, |r| format!("Topic {}: {}.", r.topic.id, r.topic.name))
    }

    #[tool(description = "Create a topic (optionally under a parent) with a long-term target level 0-3.")]
    async fn create_topic(&self, Parameters(p): Parameters<NewTopic>) -> ToolResult {
        respond(topics::create(&self.pool, p, Utc::now()).await, |t| format!("Created topic {} ({}).", t.id, t.name))
    }

    #[tool(description = "Rename, describe, re-target, reorder, move or archive a topic.")]
    async fn update_topic(&self, Parameters(p): Parameters<UpdateTopic>) -> ToolResult {
        let parent_id = match (p.parent_id, p.move_to_top_level) {
            (Some(id), _) => Some(Some(id)),
            (None, true) => Some(None),
            (None, false) => None,
        };
        let patch = TopicPatch {
            name: p.name,
            description: p.description,
            target_level: p.target_level,
            status: p.status,
            position: p.position,
            parent_id,
        };
        respond(topics::update(&self.pool, p.id, patch, Utc::now()).await, |t| format!("Updated topic {}.", t.id))
    }

    #[tool(description = "Create or update a weighted link between two topics.")]
    async fn link_topics(&self, Parameters(p): Parameters<Link>) -> ToolResult {
        respond(topics::link(&self.pool, p.from, p.to, p.kind, p.weight, p.note, Utc::now()).await, |l| {
            format!("Linked topics {} and {} ({}).", l.from_topic_id, l.to_topic_id, l.kind)
        })
    }

    #[tool(description = "Add questions to a topic in one all-or-nothing batch. Returns ids and a load forecast (days of new-question capacity, extra daily reviews).")]
    async fn add_questions(&self, Parameters(p): Parameters<AddQuestions>) -> ToolResult {
        let s = self.settings().await?;
        let status = p.status.unwrap_or(QuestionStatus::Active);
        respond(questions::add(&self.pool, p.topic_id, p.questions, status, &s, Utc::now()).await, |r| {
            format!(
                "Added {} questions; {} active = {} days of new-question capacity, ~{:.1} extra reviews/day in the first month.",
                r.ids.len(),
                r.forecast.new_active,
                r.forecast.days_of_new_capacity,
                r.forecast.est_extra_reviews_per_day_first_month
            )
        })
    }

    #[tool(description = "Edit a question. Rewording creates a new revision and returns it; priority/topic/source changes apply in place.")]
    async fn update_question(&self, Parameters(p): Parameters<UpdateQuestion>) -> ToolResult {
        respond(questions::update(&self.pool, p.id, p.patch, Utc::now()).await, |q| match q.revision_of {
            Some(old) if old == p.id => format!("Question {} replaced by revision {}.", p.id, q.id),
            _ => format!("Updated question {}.", q.id),
        })
    }

    #[tool(description = "Set a question's status: proposed, active, suspended or retired.")]
    async fn set_question_status(&self, Parameters(p): Parameters<SetQuestionStatus>) -> ToolResult {
        respond(questions::set_status(&self.pool, p.id, p.status, Utc::now()).await, |_| {
            format!("Question {} is now {}.", p.id, p.status)
        })
    }

    #[tool(description = "Create or update a weighted link between two questions (e.g. from an elaboration question).")]
    async fn link_questions(&self, Parameters(p): Parameters<Link>) -> ToolResult {
        respond(questions::link(&self.pool, p.from, p.to, p.kind, p.weight, p.note, Utc::now()).await, |_| {
            format!("Linked questions {} and {}.", p.from, p.to)
        })
    }

    #[tool(description = "A question with its full review history, ratings, scheduling state and earlier revisions.")]
    async fn get_question(&self, Parameters(p): Parameters<Id>) -> ToolResult {
        respond(questions::get(&self.pool, p.id).await, |d| {
            format!("Question {} ({} reviews).", d.question.id, d.reviews.len())
        })
    }

    // ---- References ----

    #[tool(description = "Store a reference: copies the original file and extracted text into the library, indexes the text for search, and links it to topics. Extract PDF text first with `pdftotext -layout`.")]
    async fn add_reference(&self, Parameters(p): Parameters<NewSource>) -> ToolResult {
        respond(references::add(&self.pool, &self.dirs, p, Utc::now()).await, |(s, n)| {
            format!("Added reference {} ({}) with {} searchable chunks.", s.id, s.title, n)
        })
    }

    #[tool(description = "Add files to an existing reference, e.g. a book stored as metadata only that you now have as a PDF. Keeps the reference id, so questions citing it stay linked. A new text_file replaces the searchable text; pass replace=true to swap a file the reference already has.")]
    async fn attach_reference_file(&self, Parameters(p): Parameters<AttachReference>) -> ToolResult {
        respond(references::attach(&self.pool, &self.dirs, p.id, p.files, Utc::now()).await, |(s, n)| {
            format!("Attached files to reference {} ({}); {} searchable chunks.", s.id, s.title, n)
        })
    }

    #[tool(description = "Full-text search across stored references. Returns matching excerpts with their locators.")]
    async fn search_references(&self, Parameters(p): Parameters<SearchReferences>) -> ToolResult {
        respond(references::search(&self.pool, &p.query, p.topic_id, p.limit.unwrap_or(10)).await, |h| {
            format!("{} matches.", h.len())
        })
    }

    #[tool(description = "A reference's metadata, linked topic ids and chunk count.")]
    async fn get_reference(&self, Parameters(p): Parameters<Id>) -> ToolResult {
        respond(references::get(&self.pool, p.id).await, |(s, _, n)| format!("Reference {}: {} ({n} chunks).", s.id, s.title))
    }

    #[tool(description = "Read a reference's text in order, starting at a locator.")]
    async fn read_reference(&self, Parameters(p): Parameters<ReadReference>) -> ToolResult {
        let r = references::read(&self.pool, p.id, p.from_locator.as_deref(), p.max_chunks.unwrap_or(5)).await;
        respond(r, |c| format!("{} chunks.", c.len()))
    }

    // ---- Guide ----

    #[tool(description = "The current question-writing guide (level criteria, rules) with its good and bad examples. Read before writing questions.")]
    async fn get_guide(&self) -> ToolResult {
        respond(guide::current(&self.pool).await, |(v, e)| format!("Guide version {} with {} examples.", v.id, e.len()))
    }

    #[tool(description = "Usefulness ratings with their questions, for refining the guide.")]
    async fn list_ratings(&self, Parameters(p): Parameters<ListRatings>) -> ToolResult {
        respond(guide::list_ratings(&self.pool, p.unprocessed_only).await, |r| format!("{} ratings.", r.len()))
    }

    #[tool(description = "Set a rating's classification: question_quality, knowledge_value or positive.")]
    async fn classify_rating(&self, Parameters(p): Parameters<ClassifyRating>) -> ToolResult {
        respond(guide::classify(&self.pool, p.rating_id, p.classification).await, |_| {
            format!("Rating {} classified as {}.", p.rating_id, p.classification)
        })
    }

    #[tool(description = "Save a new question-writing guide version (after the learner approves it), with examples and the ratings it accounts for.")]
    async fn save_guide_version(&self, Parameters(p): Parameters<SaveGuideVersion>) -> ToolResult {
        let r = guide::save_version(
            &self.pool,
            p.content,
            p.change_summary,
            p.examples,
            p.processed_rating_ids,
            p.removed_example_question_ids,
            Utc::now(),
        )
        .await;
        respond(r, |v| format!("Saved guide version {}.", v.id))
    }
}
