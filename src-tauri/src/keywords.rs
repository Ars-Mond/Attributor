use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::sync::OnceLock;

const KEYWORDS_RAW: &str = include_str!("../resources/keywords.txt");

static KEYWORDS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn all_keywords() -> &'static Vec<&'static str> {
    KEYWORDS.get_or_init(|| {
        KEYWORDS_RAW
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect()
    })
}

/// Search the keyword dictionary and return up to `limit` results (default 100).
///
/// Sorting priority:
///   0 — exact match           ("road")
///   1 — prefix match          ("roads", "road bicycle")   → shorter first
///   2 — word-boundary match   ("dirt road", "gravel road") → shorter first
///   3 — other substring       ("crossroads", "railroad")   → shorter first
///   4 — fuzzy match only      (nucleo score, desc)
pub fn search_keywords_impl(query: String, limit: Option<usize>) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let limit = limit.unwrap_or(100);
    let word_boundary = format!(" {q}");

    // Tiers 0-3: substring-based matches
    let mut substring_hits: Vec<(u8, usize, &str)> = all_keywords()
        .iter()
        .filter_map(|kw| {
            let lower = kw.to_lowercase();
            if !lower.contains(&q) {
                return None;
            }
            let tier: u8 = if lower == q {
                0
            } else if lower.starts_with(&q) {
                1
            } else if lower.contains(&word_boundary) {
                2
            } else {
                3
            };
            Some((tier, kw.len(), *kw))
        })
        .collect();

    substring_hits.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    // Collect the set of already-matched keywords so fuzzy doesn't duplicate them
    let substring_set: std::collections::HashSet<&str> =
        substring_hits.iter().map(|(_, _, kw)| *kw).collect();

    // Tier 4: fuzzy matches for keywords NOT already in substring results
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(&q, CaseMatching::Ignore, Normalization::Smart);

    let mut fuzzy_hits: Vec<(u32, &str)> = all_keywords()
        .iter()
        .filter(|kw| !substring_set.contains(*kw))
        .filter_map(|kw| {
            let mut buf = Vec::new();
            let score = pattern.score(Utf32Str::new(kw, &mut buf), &mut matcher)?;
            Some((score, *kw))
        })
        .collect();

    // Higher score = better match → sort descending
    fuzzy_hits.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut out: Vec<String> = Vec::with_capacity(limit.min(substring_hits.len() + fuzzy_hits.len()));
    for (_, _, kw) in substring_hits.into_iter().take(limit) {
        out.push(kw.to_string());
    }
    let remaining = limit.saturating_sub(out.len());
    for (_, kw) in fuzzy_hits.into_iter().take(remaining) {
        out.push(kw.to_string());
    }
    out
}
