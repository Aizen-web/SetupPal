//! Parse and strip the trailing `setuppal-intent` fenced JSON block from chat replies.

use serde::{Deserialize, Serialize};

const FENCE_OPEN: &str = "```setuppal-intent";
const FENCE_CLOSE: &str = "```";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupIntent {
    Install {
        goal: String,
        #[serde(default)]
        items: Vec<IntentItem>,
    },
    Manual {
        item: String,
        reason: String,
        #[serde(default)]
        alternatives: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntentItem {
    pub name: String,
    #[serde(default)]
    pub ecosystem: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentParseError {
    Empty,
    Json,
    Validation(&'static str),
}

impl SetupIntent {
    pub fn validate(&self) -> Result<(), IntentParseError> {
        match self {
            SetupIntent::Install { goal, items } => {
                let len = goal.chars().count();
                if len < 3 || len > 300 {
                    return Err(IntentParseError::Validation("goal length"));
                }
                if items.len() > 20 {
                    return Err(IntentParseError::Validation("too many items"));
                }
                for item in items {
                    if item.name.chars().count() > 80 {
                        return Err(IntentParseError::Validation("item name length"));
                    }
                    if item.id.as_ref().is_some_and(|v| v.chars().count() > 120) {
                        return Err(IntentParseError::Validation("item id length"));
                    }
                    if item.version.as_ref().is_some_and(|v| v.chars().count() > 40) {
                        return Err(IntentParseError::Validation("item version length"));
                    }
                    if let Some(eco) = item.ecosystem.as_deref() {
                        match eco {
                            "winget" | "npm" | "pypi" | "cargo" | "modrinth" | "github" | "vendor" => {}
                            _ => return Err(IntentParseError::Validation("ecosystem")),
                        }
                    }
                }
                Ok(())
            }
            SetupIntent::Manual {
                item,
                reason,
                alternatives,
            } => {
                if item.chars().count() > 80 || item.is_empty() {
                    return Err(IntentParseError::Validation("item length"));
                }
                if reason.chars().count() > 300 || reason.is_empty() {
                    return Err(IntentParseError::Validation("reason length"));
                }
                if alternatives.len() > 5 {
                    return Err(IntentParseError::Validation("too many alternatives"));
                }
                if alternatives.iter().any(|a| a.chars().count() > 80) {
                    return Err(IntentParseError::Validation("alternative length"));
                }
                Ok(())
            }
        }
    }
}

/// Incremental filter that holds back a trailing `setuppal-intent` fence from visible deltas.
#[derive(Debug, Default)]
pub struct IntentStreamFilter {
    /// Full text seen so far (including held intent body once capturing).
    full: String,
    /// Text that might still become the start of an intent fence; not yet forwarded.
    hold: String,
    capturing: bool,
    intent_body: String,
    /// Visible reply with the intent fence removed (or never present).
    visible: String,
}

impl IntentStreamFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a provider delta. Returns the slice that is safe to show the user.
    pub fn push(&mut self, chunk: &str) -> String {
        if chunk.is_empty() {
            return String::new();
        }
        if self.capturing {
            self.full.push_str(chunk);
            self.intent_body.push_str(chunk);
            if let Some(end) = find_closing_fence(&self.intent_body) {
                // Drop the closing ``` and anything after it from the capture.
                self.intent_body.truncate(end);
                self.capturing = false;
                // Trailing text after the fence is unusual; keep it invisible.
            }
            return String::new();
        }

        self.hold.push_str(chunk);
        let mut forward = String::new();

        loop {
            if self.capturing {
                break;
            }
            match find_fence_decision(&self.hold) {
                FenceDecision::NeedMore => break,
                FenceDecision::NotIntent { release_up_to } => {
                    forward.push_str(&self.hold[..release_up_to]);
                    self.visible.push_str(&self.hold[..release_up_to]);
                    self.full.push_str(&self.hold[..release_up_to]);
                    self.hold = self.hold[release_up_to..].to_string();
                    if release_up_to == 0 {
                        // Ambiguous prefix of "```" — wait for more bytes.
                        break;
                    }
                }
                FenceDecision::IsIntent { open_at } => {
                    forward.push_str(&self.hold[..open_at]);
                    self.visible.push_str(&self.hold[..open_at]);
                    self.full.push_str(&self.hold[..open_at]);
                    let after_open = &self.hold[open_at + FENCE_OPEN.len()..];
                    // Consume optional newline right after the language tag.
                    let after_open = after_open.strip_prefix('\r').unwrap_or(after_open);
                    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
                    self.full.push_str(&self.hold[open_at..]);
                    self.intent_body = after_open.to_string();
                    self.hold.clear();
                    self.capturing = true;
                    if let Some(end) = find_closing_fence(&self.intent_body) {
                        self.intent_body.truncate(end);
                        self.capturing = false;
                    }
                    break;
                }
            }
        }

        forward
    }

    /// Flush remaining hold buffer and parse any captured intent JSON.
    pub fn finish(mut self) -> (String, Result<Option<SetupIntent>, IntentParseError>) {
        if !self.capturing && !self.hold.is_empty() {
            // Incomplete fence marker at EOF — show it (not a real intent block).
            self.visible.push_str(&self.hold);
            self.full.push_str(&self.hold);
            self.hold.clear();
        }
        let visible = trim_trailing_blank_lines(&self.visible);
        let intent = if self.intent_body.trim().is_empty() && !self.capturing {
            Ok(None)
        } else {
            parse_intent_json(&self.intent_body).map(Some)
        };
        (visible, intent)
    }

}

fn trim_trailing_blank_lines(s: &str) -> String {
    s.trim_end_matches(['\n', '\r', ' ', '\t']).to_string()
}

fn parse_intent_json(raw: &str) -> Result<SetupIntent, IntentParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IntentParseError::Empty);
    }
    let intent: SetupIntent =
        serde_json::from_str(trimmed).map_err(|_| IntentParseError::Json)?;
    intent.validate()?;
    Ok(intent)
}

/// Strip a complete intent fence from a finished reply (non-streaming helper / tests).
pub fn split_intent_block(reply: &str) -> (String, Result<Option<SetupIntent>, IntentParseError>) {
    let mut filter = IntentStreamFilter::new();
    let _ = filter.push(reply);
    filter.finish()
}

#[derive(Debug)]
enum FenceDecision {
    /// Hold everything; need more characters to decide.
    NeedMore,
    /// First `release_up_to` chars are safe to forward (may be 0 when holding a bare ``` prefix).
    NotIntent { release_up_to: usize },
    /// Intent fence opens at byte index `open_at` within hold.
    IsIntent { open_at: usize },
}

/// Bytes at the end of `hold` that could still grow into a line-start ``` fence.
fn trailing_fence_prefix_len(hold: &str) -> usize {
    let bytes = hold.as_bytes();
    let mut i = bytes.len();
    let mut ticks = 0usize;
    while i > 0 && bytes[i - 1] == b'`' && ticks < 3 {
        i -= 1;
        ticks += 1;
    }
    if ticks == 0 {
        return 0;
    }
    // Only hold if those ticks sit at a line start (or the whole buffer is just ticks).
    if is_line_start(hold, i) {
        ticks
    } else {
        0
    }
}

fn find_fence_decision(hold: &str) -> FenceDecision {
    // Prefer a fence that starts at the beginning of a line.
    let mut search_from = 0usize;
    while search_from <= hold.len() {
        let rest = &hold[search_from..];
        let Some(rel) = rest.find("```") else {
            let hold_tail = trailing_fence_prefix_len(hold);
            return FenceDecision::NotIntent {
                release_up_to: hold.len().saturating_sub(hold_tail),
            };
        };
        let at = search_from + rel;
        if !is_line_start(hold, at) {
            // Forward through this non-line-start ``` and keep scanning.
            search_from = at + 3;
            continue;
        }

        let after = &hold[at..];
        if after.len() < FENCE_OPEN.len() {
            // Could still become ```setuppal-intent — hold from this fence.
            if FENCE_OPEN.starts_with(after) {
                return if at == 0 {
                    FenceDecision::NeedMore
                } else {
                    FenceDecision::NotIntent { release_up_to: at }
                };
            }
            // Prefix does not match setuppal-intent (e.g. ```po…) — treat as normal code.
            // If the tag is still incomplete but already diverges, forward past ```.
            return FenceDecision::NotIntent {
                release_up_to: at + 3.min(after.len()),
            };
        }

        if after.starts_with(FENCE_OPEN) {
            let after_tag = &after[FENCE_OPEN.len()..];
            // Require end-of-tag boundary: EOF, whitespace, or newline.
            if after_tag.is_empty()
                || after_tag.starts_with(|c: char| c.is_whitespace())
            {
                return FenceDecision::IsIntent { open_at: at };
            }
            // ```setuppal-intentional — not our fence.
            search_from = at + 3;
            continue;
        }

        // Some other fence (```powershell, ```json, …). Forward up to and including ```
        // so the rest of the language tag streams normally on later iterations.
        return FenceDecision::NotIntent {
            release_up_to: at + 3,
        };
    }
    FenceDecision::NotIntent {
        release_up_to: hold.len(),
    }
}

fn is_line_start(s: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    matches!(s.as_bytes().get(idx - 1), Some(b'\n') | Some(b'\r'))
}

fn find_closing_fence(body: &str) -> Option<usize> {
    // Closing fence must be at line start.
    let mut search = 0usize;
    while let Some(rel) = body[search..].find(FENCE_CLOSE) {
        let at = search + rel;
        if is_line_start(body, at) {
            return Some(at);
        }
        search = at + FENCE_CLOSE.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_json() -> &'static str {
        r#"{"kind":"install","goal":"Python 3.12 with a venv for data science","items":[{"name":"Python 3.12","ecosystem":"winget","id":"Python.Python.3.12"}]}"#
    }

    #[test]
    fn block_at_end_is_stripped_and_parsed() {
        let reply = format!("Here is the plan.\n\n```setuppal-intent\n{}\n```", install_json());
        let (visible, intent) = split_intent_block(&reply);
        assert_eq!(visible, "Here is the plan.");
        let intent = intent.expect("parse ok").expect("has intent");
        match intent {
            SetupIntent::Install { goal, items } => {
                assert!(goal.contains("Python"));
                assert_eq!(items.len(), 1);
            }
            _ => panic!("expected install"),
        }
    }

    #[test]
    fn block_split_across_chunks_including_fence_marker() {
        let mut filter = IntentStreamFilter::new();
        let parts = [
            "Ready to install.\n\n`",
            "``set",
            "uppal-intent\n",
            install_json(),
            "\n```",
        ];
        let mut forwarded = String::new();
        for part in parts {
            forwarded.push_str(&filter.push(part));
        }
        let (visible, intent) = filter.finish();
        assert_eq!(forwarded, "Ready to install.\n\n");
        assert_eq!(visible, "Ready to install.");
        assert!(matches!(intent, Ok(Some(SetupIntent::Install { .. }))));
    }

    #[test]
    fn malformed_json_strips_block_without_intent() {
        let reply = "Almost.\n\n```setuppal-intent\n{not-json}\n```";
        let (visible, intent) = split_intent_block(reply);
        assert_eq!(visible, "Almost.");
        assert!(matches!(intent, Err(IntentParseError::Json)));
    }

    #[test]
    fn no_block_passes_through() {
        let reply = "What do you want set up?";
        let (visible, intent) = split_intent_block(reply);
        assert_eq!(visible, reply);
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn normal_powershell_fence_passes_through() {
        let reply = "Run this:\n```powershell\nwinget install Git.Git\n```\nDone.";
        let mut filter = IntentStreamFilter::new();
        // Split so "```" arrives alone first, then "powershell".
        let mut out = String::new();
        out.push_str(&filter.push("Run this:\n```"));
        out.push_str(&filter.push("powershell\nwinget install Git.Git\n```\nDone."));
        let (visible, intent) = filter.finish();
        assert_eq!(visible, reply);
        assert_eq!(out, reply);
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn manual_intent_parses() {
        let body = r#"{"kind":"manual","item":"Steam","reason":"Needs a GUI launcher and account login","alternatives":["7-Zip","VLC"]}"#;
        let reply = format!("**Manual setup guide**\n\n```setuppal-intent\n{body}\n```");
        let (visible, intent) = split_intent_block(&reply);
        assert!(visible.starts_with("**Manual setup guide**"));
        assert!(matches!(intent, Ok(Some(SetupIntent::Manual { .. }))));
    }
}
