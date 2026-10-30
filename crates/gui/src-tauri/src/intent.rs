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
                if !(3..=300).contains(&len) {
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
                    if item
                        .version
                        .as_ref()
                        .is_some_and(|v| v.chars().count() > 40)
                    {
                        return Err(IntentParseError::Validation("item version length"));
                    }
                    if let Some(eco) = item.ecosystem.as_deref() {
                        match eco {
                            "winget" | "npm" | "pypi" | "cargo" | "modrinth" | "github"
                            | "vendor" => {}
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
    /// Second stage: finds unfenced `{"kind":...}` objects the model forgot to fence.
    loose: LooseJsonFilter,
    /// Visible reply with every intent removed (or never present).
    visible: String,
}

impl IntentStreamFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a provider delta. Returns the slice that is safe to show the user.
    pub fn push(&mut self, chunk: &str) -> String {
        let fenced_safe = self.push_fenced(chunk);
        let shown = self.loose.push(&fenced_safe);
        self.visible.push_str(&shown);
        shown
    }

    /// First stage: hold back a trailing `setuppal-intent` fence.
    fn push_fenced(&mut self, chunk: &str) -> String {
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
                    self.full.push_str(&self.hold[..release_up_to]);
                    self.hold = self.hold[release_up_to..].to_string();
                    if release_up_to == 0 {
                        // Ambiguous prefix of "```" — wait for more bytes.
                        break;
                    }
                }
                FenceDecision::IsIntent { open_at } => {
                    forward.push_str(&self.hold[..open_at]);
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

    /// Flush remaining hold buffers and parse any captured intent JSON.
    pub fn finish(mut self) -> (String, Result<Option<SetupIntent>, IntentParseError>) {
        if !self.capturing && !self.hold.is_empty() {
            // Incomplete fence marker at EOF: show it (not a real intent block).
            let rest = std::mem::take(&mut self.hold);
            self.full.push_str(&rest);
            let shown = self.loose.push(&rest);
            self.visible.push_str(&shown);
        }
        let (tail, loose_json) = std::mem::take(&mut self.loose).finish();
        self.visible.push_str(&tail);
        let visible = trim_trailing_blank_lines(&self.visible);
        let intent = if !self.intent_body.trim().is_empty() || self.capturing {
            parse_intent_json(&self.intent_body).map(Some)
        } else if let Some(json) = loose_json {
            parse_intent_json(&json).map(Some)
        } else {
            Ok(None)
        };
        (visible, intent)
    }
}

/// Second stage. Holds back a line-start `{...}` object whose `kind` is `install` or `manual`
/// and drops it only when nothing but whitespace follows. Anything else (code blocks, other
/// JSON, text after it) is released unchanged.
#[derive(Debug)]
struct LooseJsonFilter {
    at_line_start: bool,
    in_code: bool,
    fence_probe: String,
    /// Text held back since a candidate `{` (empty when not holding).
    pending: String,
    depth: u32,
    in_string: bool,
    escaped: bool,
    /// True once the first non-space character after the opening brace has been checked.
    key_checked: bool,
    /// The object closed and is a known intent; waiting to see if only whitespace follows.
    closed: bool,
    trailing_ws: String,
}

impl Default for LooseJsonFilter {
    fn default() -> Self {
        Self {
            at_line_start: true,
            in_code: false,
            fence_probe: String::new(),
            pending: String::new(),
            depth: 0,
            in_string: false,
            escaped: false,
            key_checked: false,
            closed: false,
            trailing_ws: String::new(),
        }
    }
}

/// Longest object we hold back before deciding it is ordinary text.
const MAX_LOOSE_JSON_CHARS: usize = 4000;

fn is_known_intent_json(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned))
        .is_some_and(|k| k == "install" || k == "manual")
}

impl LooseJsonFilter {
    fn push(&mut self, text: &str) -> String {
        let mut out = String::new();
        for c in text.chars() {
            self.step(c, &mut out);
        }
        out
    }

    /// Give the held text back to the user as ordinary reply text.
    fn release(&mut self, out: &mut String) {
        out.push_str(&self.pending);
        out.push_str(&self.trailing_ws);
        self.pending.clear();
        self.trailing_ws.clear();
        self.depth = 0;
        self.in_string = false;
        self.escaped = false;
        self.key_checked = false;
        self.closed = false;
        self.at_line_start = false;
    }

    fn step(&mut self, c: char, out: &mut String) {
        if self.closed {
            if c.is_whitespace() {
                self.trailing_ws.push(c);
            } else {
                // Text follows the object, so it was not a trailing intent.
                self.release(out);
                self.step_plain(c, out);
            }
            return;
        }
        if self.pending.is_empty() {
            self.step_plain(c, out);
            return;
        }
        self.pending.push(c);
        if self.pending.chars().count() > MAX_LOOSE_JSON_CHARS {
            self.release(out);
            return;
        }
        if !self.key_checked && !c.is_whitespace() {
            self.key_checked = true;
            if c != '"' {
                // Not a JSON key, so this is not an intent object.
                self.release(out);
                return;
            }
        }
        if self.in_string {
            if self.escaped {
                self.escaped = false;
            } else if c == '\\' {
                self.escaped = true;
            } else if c == '"' {
                self.in_string = false;
            }
            return;
        }
        match c {
            '"' => self.in_string = true,
            '{' => self.depth += 1,
            '}' => {
                self.depth -= 1;
                if self.depth == 0 {
                    if is_known_intent_json(&self.pending) {
                        self.closed = true;
                    } else {
                        self.release(out);
                    }
                }
            }
            _ => {}
        }
    }

    fn step_plain(&mut self, c: char, out: &mut String) {
        if c == '\n' {
            self.at_line_start = true;
            self.fence_probe.clear();
            out.push(c);
            return;
        }
        if self.at_line_start {
            if c == ' ' || c == '\t' || c == '\r' {
                out.push(c);
                return;
            }
            if c == '`' {
                self.fence_probe.push('`');
                out.push(c);
                if self.fence_probe == "```" {
                    self.in_code = !self.in_code;
                    self.fence_probe.clear();
                    self.at_line_start = false;
                }
                return;
            }
            self.fence_probe.clear();
            self.at_line_start = false;
            if c == '{' && !self.in_code {
                self.pending.push('{');
                self.depth = 1;
                return;
            }
        }
        out.push(c);
    }

    /// Returns text still owed to the user plus the stripped intent JSON, if any.
    fn finish(mut self) -> (String, Option<String>) {
        let mut out = String::new();
        if self.closed {
            return (out, Some(self.pending.trim().to_string()));
        }
        if !self.pending.is_empty() {
            self.release(&mut out);
        }
        (out, None)
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
    let intent: SetupIntent = serde_json::from_str(trimmed).map_err(|_| IntentParseError::Json)?;
    intent.validate()?;
    Ok(intent)
}

/// Strip a complete intent fence from a finished reply. Production code streams through
/// `IntentStreamFilter`; this one-shot form keeps the tests short.
#[cfg(test)]
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

        if let Some(after_tag) = after.strip_prefix(FENCE_OPEN) {
            // Require end-of-tag boundary: EOF, whitespace, or newline.
            if after_tag.is_empty() || after_tag.starts_with(|c: char| c.is_whitespace()) {
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
        let reply = format!(
            "Here is the plan.\n\n```setuppal-intent\n{}\n```",
            install_json()
        );
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

    fn stream_chars(reply: &str) -> (String, String, Result<Option<SetupIntent>, IntentParseError>) {
        let mut filter = IntentStreamFilter::new();
        let mut forwarded = String::new();
        let mut buf = [0u8; 4];
        for ch in reply.chars() {
            forwarded.push_str(&filter.push(ch.encode_utf8(&mut buf)));
        }
        let (visible, intent) = filter.finish();
        (forwarded, visible, intent)
    }

    #[test]
    fn unfenced_install_json_at_end_is_stripped_and_parsed() {
        let reply = format!("Sounds good, installing it.\n\n{}", install_json());
        let (forwarded, visible, intent) = stream_chars(&reply);
        assert_eq!(visible, "Sounds good, installing it.");
        assert!(!forwarded.contains('{'), "raw JSON leaked into deltas: {forwarded}");
        assert!(matches!(intent, Ok(Some(SetupIntent::Install { .. }))));
    }

    #[test]
    fn unfenced_manual_json_is_stripped_and_parsed() {
        let body = r#"{"kind":"manual","item":"Steam","reason":"Needs a store login","alternatives":["7-Zip"]}"#;
        let reply = format!("**Manual setup guide**\n1. Download Steam.\n{body}\n");
        let (forwarded, visible, intent) = stream_chars(&reply);
        assert_eq!(visible, "**Manual setup guide**\n1. Download Steam.");
        assert!(!forwarded.contains("\"kind\""));
        assert!(matches!(intent, Ok(Some(SetupIntent::Manual { .. }))));
    }

    #[test]
    fn unfenced_json_works_in_a_single_chunk_and_with_braces_in_strings() {
        let body = r#"{"kind":"install","goal":"a } tricky { goal \"quoted\""}"#;
        let (visible, intent) = split_intent_block(&format!("Ok.\n{body}"));
        assert_eq!(visible, "Ok.");
        match intent.expect("parse ok").expect("intent") {
            SetupIntent::Install { goal, .. } => assert!(goal.contains("tricky")),
            _ => panic!("expected install"),
        }
    }

    #[test]
    fn unfenced_json_with_unknown_kind_or_no_kind_is_left_alone() {
        for body in [
            r#"{"kind":"other","goal":"x"}"#,
            r#"{"name":"demo","version":"1"}"#,
            "{}",
            "{not json at all}",
        ] {
            let reply = format!("Example:\n{body}");
            let (forwarded, visible, intent) = stream_chars(&reply);
            assert_eq!(visible, reply, "body {body}");
            assert_eq!(forwarded.trim_end(), reply, "body {body}");
            assert_eq!(intent, Ok(None));
        }
    }

    #[test]
    fn unfenced_json_followed_by_more_text_is_not_stripped() {
        let reply = format!("Config looks like:\n{}\nThat is all.", install_json());
        let (_, visible, intent) = stream_chars(&reply);
        assert_eq!(visible, reply);
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn json_inside_a_code_block_is_never_stripped() {
        let reply = format!("Here:\n```json\n{}\n```", install_json());
        let (forwarded, visible, intent) = stream_chars(&reply);
        assert_eq!(visible, reply);
        // The closing fence is held until the stream ends, then flushed into `visible`.
        assert!(forwarded.contains(install_json()));
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn inline_json_mid_line_is_not_stripped() {
        let reply = format!("Use {} as the payload", install_json());
        let (_, visible, intent) = stream_chars(&reply);
        assert_eq!(visible, reply);
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn unterminated_json_is_shown_not_swallowed() {
        let reply = "Plan:\n{\"kind\":\"install\",\"goal\":\"typescri";
        let (_, visible, intent) = stream_chars(reply);
        assert_eq!(visible, reply);
        assert_eq!(intent, Ok(None));
    }

    #[test]
    fn invalid_unfenced_intent_is_stripped_without_an_intent() {
        let (visible, intent) =
            split_intent_block("Almost.\n{\"kind\":\"install\",\"goal\":\"ab\"}");
        assert_eq!(visible, "Almost.");
        assert!(matches!(intent, Err(IntentParseError::Validation(_))));
    }

    #[test]
    fn fenced_intent_wins_and_plain_fences_still_pass_through() {
        let reply = format!(
            "Run:\n```powershell\nwinget install Git.Git\n```\nDone.\n\n```setuppal-intent\n{}\n```",
            install_json()
        );
        let (_, visible, intent) = stream_chars(&reply);
        assert_eq!(
            visible,
            "Run:\n```powershell\nwinget install Git.Git\n```\nDone."
        );
        assert!(matches!(intent, Ok(Some(SetupIntent::Install { .. }))));
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
