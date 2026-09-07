use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::MediaItem;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylistDefinition {
    #[serde(rename = "match")]
    pub match_mode: SmartMatch,
    pub rules: Vec<SmartPlaylistRule>,
    pub sort: SmartPlaylistSort,
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartMatch {
    All,
    Any,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylistRule {
    pub field: SmartRuleField,
    pub operator: SmartOperator,
    pub value: SmartRuleValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SmartRuleValue {
    Text(String),
    Number(serde_json::Number),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartRuleField {
    Title,
    Artist,
    Album,
    Label,
    Genre,
    DurationMs,
    PlayCount,
    Favorite,
    LastPlayedDays,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartOperator {
    Contains,
    Equals,
    NotContains,
    LessThan,
    GreaterThan,
    Within,
    NotWithin,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylistSort {
    pub field: SmartSortField,
    pub direction: SmartSortDirection,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartSortField {
    LibraryOrder,
    Title,
    Artist,
    Album,
    DurationMs,
    PlayCount,
    LastPlayedAtMs,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartSortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartPlaylistPreview {
    pub total_matches: usize,
    pub matches: Vec<SmartPlaylistMatch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartPlaylistMatch {
    pub track_id: String,
    pub matched_rule_indexes: Vec<usize>,
}

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const DAY_MS: u64 = 86_400_000;

fn integer(value: &serde_json::Number) -> Option<u64> {
    if let Some(value) = value.as_u64() {
        return (value <= MAX_SAFE_INTEGER).then_some(value);
    }
    let value = value.as_f64()?;
    (value.is_finite() && value >= 0.0 && value <= MAX_SAFE_INTEGER as f64 && value.fract() == 0.0)
        .then_some(value as u64)
}

impl SmartPlaylistDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=20).contains(&self.rules.len()) {
            return Err("a smart playlist must have 1–20 rules".into());
        }
        if self
            .limit
            .is_some_and(|limit| !(1..=10000).contains(&limit))
        {
            return Err("the smart playlist limit must be 1–10000".into());
        }
        use SmartOperator as Op;
        use SmartRuleField as Field;
        for (index, rule) in self.rules.iter().enumerate() {
            let valid = match (&rule.field, &rule.operator, &rule.value) {
                (
                    Field::Title | Field::Artist | Field::Album | Field::Label | Field::Genre,
                    Op::Contains | Op::Equals | Op::NotContains,
                    SmartRuleValue::Text(value),
                ) => (1..=200).contains(&value.trim().chars().count()),
                (
                    Field::DurationMs | Field::PlayCount,
                    Op::Equals | Op::LessThan | Op::GreaterThan,
                    SmartRuleValue::Number(value),
                ) => integer(value).is_some(),
                (Field::Favorite, Op::Equals, SmartRuleValue::Boolean(_)) => true,
                (
                    Field::LastPlayedDays,
                    Op::Within | Op::NotWithin,
                    SmartRuleValue::Number(value),
                ) => integer(value).is_some_and(|days| (1..=36500).contains(&days)),
                _ => false,
            };
            if !valid {
                return Err(format!(
                    "smart playlist rule {} has an invalid field, operator, or value",
                    index + 1
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn normalized(mut self) -> Result<Self, String> {
        self.validate()?;
        for rule in &mut self.rules {
            if let SmartRuleValue::Text(value) = &mut rule.value {
                *value = value.trim().to_owned();
            }
        }
        Ok(self)
    }
}

fn matches_rule(rule: &SmartPlaylistRule, track: &MediaItem, favorite: bool, now_ms: u64) -> bool {
    use SmartOperator as Op;
    use SmartRuleField as Field;
    match &rule.value {
        SmartRuleValue::Text(value) => {
            let needle = value.trim().to_lowercase();
            let matches = |text: &str| {
                let text = text.to_lowercase();
                if rule.operator == Op::Equals {
                    text == needle
                } else {
                    text.contains(&needle)
                }
            };
            let matched = match rule.field {
                Field::Title => matches(&track.title),
                Field::Artist => matches(&track.artist),
                Field::Album => track.album.as_deref().is_some_and(matches),
                Field::Label => track.label.as_deref().is_some_and(matches),
                Field::Genre => track.genres.iter().any(|genre| matches(genre)),
                _ => false,
            };
            if rule.operator == Op::NotContains {
                !matched
            } else {
                matched
            }
        }
        SmartRuleValue::Number(value) => {
            let Some(value) = integer(value) else {
                return false;
            };
            if rule.field == Field::LastPlayedDays {
                let cutoff = now_ms.saturating_sub(value.saturating_mul(DAY_MS));
                let within = track
                    .last_played_at_ms
                    .is_some_and(|played| played >= cutoff);
                return if rule.operator == Op::Within {
                    within
                } else {
                    !within
                };
            }
            let actual = match rule.field {
                Field::DurationMs => track.duration_ms,
                Field::PlayCount => track.play_count,
                _ => return false,
            };
            match rule.operator {
                Op::Equals => actual == value,
                Op::LessThan => actual < value,
                Op::GreaterThan => actual > value,
                _ => false,
            }
        }
        SmartRuleValue::Boolean(value) => favorite == *value,
    }
}

// Callers validate definitions before storing them or resolving a preview. The clock is
// supplied once per snapshot, so every relative rule uses the same cutoff.
pub(crate) fn resolve(
    definition: &SmartPlaylistDefinition,
    tracks: &[MediaItem],
    favorites: &HashSet<&str>,
    now_ms: u64,
) -> SmartPlaylistPreview {
    let mut matches = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.is_available())
        .filter_map(|(position, track)| {
            let matched_rule_indexes = definition
                .rules
                .iter()
                .enumerate()
                .filter_map(|(index, rule)| {
                    matches_rule(rule, track, favorites.contains(track.id.as_str()), now_ms)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            let matched = match definition.match_mode {
                SmartMatch::All => matched_rule_indexes.len() == definition.rules.len(),
                SmartMatch::Any => !matched_rule_indexes.is_empty(),
            };
            matched.then_some((position, track, matched_rule_indexes))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_index, left, _), (right_index, right, _)| {
        use SmartSortField as Field;
        let order = match definition.sort.field {
            Field::LibraryOrder => left_index.cmp(right_index),
            Field::Title => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
            Field::Artist => left.artist.to_lowercase().cmp(&right.artist.to_lowercase()),
            Field::Album => left
                .album
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .cmp(&right.album.as_deref().unwrap_or_default().to_lowercase()),
            Field::DurationMs => left.duration_ms.cmp(&right.duration_ms),
            Field::PlayCount => left.play_count.cmp(&right.play_count),
            Field::LastPlayedAtMs => left.last_played_at_ms.cmp(&right.last_played_at_ms),
        };
        let order = if definition.sort.direction == SmartSortDirection::Desc {
            order.reverse()
        } else {
            order
        };
        order.then_with(|| left_index.cmp(right_index))
    });
    let total_matches = matches.len();
    if let Some(limit) = definition.limit {
        matches.truncate(limit as usize);
    }
    SmartPlaylistPreview {
        total_matches,
        matches: matches
            .into_iter()
            .map(|(_, track, matched_rule_indexes)| SmartPlaylistMatch {
                track_id: track.id.clone(),
                matched_rule_indexes,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn track(id: &str) -> MediaItem {
        serde_json::from_value(json!({
            "id": id, "title": "Blue Sky", "artist": "ARTIST", "album": "Album",
            "label": "Label", "genres": ["Jazz", "Post Rock"], "durationMs": 120000,
            "playCount": 3
        }))
        .unwrap()
    }

    fn definition(rules: Value) -> SmartPlaylistDefinition {
        serde_json::from_value(json!({
            "match": "all", "rules": rules,
            "sort": {"field": "libraryOrder", "direction": "asc"}, "limit": null
        }))
        .unwrap()
    }

    fn rule(field: &str, operator: &str, value: Value) -> Value {
        json!({"field": field, "operator": operator, "value": value})
    }

    fn ids(preview: &SmartPlaylistPreview) -> Vec<&str> {
        preview
            .matches
            .iter()
            .map(|item| item.track_id.as_str())
            .collect()
    }

    #[test]
    fn smart_all_any_and_matched_rule_indexes() {
        let tracks = vec![track("a")];
        let mut def = definition(json!([
            rule("title", "contains", json!("blue")),
            rule("durationMs", "lessThan", json!(1)),
            rule("artist", "equals", json!("artist"))
        ]));
        assert!(
            resolve(&def, &tracks, &HashSet::new(), 0)
                .matches
                .is_empty()
        );
        def.match_mode = SmartMatch::Any;
        let preview = resolve(&def, &tracks, &HashSet::new(), 0);
        assert_eq!(ids(&preview), ["a"]);
        assert_eq!(preview.matches[0].matched_rule_indexes, [0, 2]);
    }

    #[test]
    fn smart_text_trims_case_and_matches_individual_genres() {
        for (field, op, value, expected) in [
            ("title", "contains", "  BLUE  ", true),
            ("artist", "equals", " artist ", true),
            ("album", "equals", "ALBUM", true),
            ("label", "notContains", "other", true),
            ("genre", "equals", "jazz", true),
            ("genre", "equals", "Jazz, Post Rock", false),
            ("genre", "notContains", "rock", false),
            ("genre", "notContains", "pop", true),
        ] {
            let def = definition(json!([rule(field, op, json!(value))]));
            assert_eq!(
                resolve(&def, &[track("a")], &HashSet::new(), 0).total_matches == 1,
                expected,
                "{field} {op}"
            );
        }
        let mut missing = track("missing");
        missing.album = None;
        missing.genres.clear();
        let def = definition(json!([rule("genre", "notContains", json!("rock"))]));
        assert_eq!(
            resolve(&def, &[missing], &HashSet::new(), 0).total_matches,
            1
        );
    }

    #[test]
    fn smart_numbers_and_favorite_membership() {
        for (field, op, value, expected) in [
            ("durationMs", "equals", 120000, true),
            ("durationMs", "lessThan", 120000, false),
            ("durationMs", "greaterThan", 119999, true),
            ("playCount", "equals", 3, true),
            ("playCount", "lessThan", 4, true),
            ("playCount", "greaterThan", 3, false),
        ] {
            let def = definition(json!([rule(field, op, json!(value))]));
            assert_eq!(
                resolve(&def, &[track("a")], &HashSet::new(), 0).total_matches == 1,
                expected
            );
        }
        let def = definition(json!([rule("favorite", "equals", json!(true))]));
        assert_eq!(
            ids(&resolve(
                &def,
                &[track("a"), track("b")],
                &HashSet::from(["b"]),
                0
            )),
            ["b"]
        );
    }

    #[test]
    fn smart_relative_time_includes_cutoff_and_never_played() {
        let now = 10 * 86_400_000;
        let mut tracks = vec![
            track("never"),
            track("before"),
            track("cutoff"),
            track("now"),
        ];
        tracks[1].last_played_at_ms = Some(now - 86_400_000 - 1);
        tracks[2].last_played_at_ms = Some(now - 86_400_000);
        tracks[3].last_played_at_ms = Some(now);
        let def = definition(json!([rule("lastPlayedDays", "within", json!(1))]));
        assert_eq!(
            ids(&resolve(&def, &tracks, &HashSet::new(), now)),
            ["cutoff", "now"]
        );
        assert_eq!(
            ids(&resolve(&def, &tracks, &HashSet::new(), now + 1)),
            ["now"]
        );
        let def = definition(json!([rule("lastPlayedDays", "notWithin", json!(1))]));
        assert_eq!(
            ids(&resolve(&def, &tracks, &HashSet::new(), now)),
            ["never", "before"]
        );
        let def = definition(json!([rule("lastPlayedDays", "within", json!(36500))]));
        tracks[1].last_played_at_ms = Some(0);
        assert_eq!(resolve(&def, &tracks, &HashSet::new(), 0).total_matches, 3);
    }

    #[test]
    fn smart_sort_ties_limits_and_unavailable_tracks() {
        let mut tracks = vec![track("z"), track("a"), track("hidden"), track("b")];
        tracks[2].availability = Some("subscriber_only_unavailable".into());
        for field in [
            "libraryOrder",
            "title",
            "artist",
            "album",
            "durationMs",
            "playCount",
            "lastPlayedAtMs",
        ] {
            for direction in ["asc", "desc"] {
                let mut def = definition(json!([rule("playCount", "greaterThan", json!(0))]));
                def.sort = serde_json::from_value(json!({"field": field, "direction": direction}))
                    .unwrap();
                def.limit = Some(2);
                let preview = resolve(&def, &tracks, &HashSet::new(), 0);
                assert_eq!(preview.total_matches, 3);
                assert_eq!(
                    ids(&preview),
                    if field == "libraryOrder" && direction == "desc" {
                        vec!["b", "a"]
                    } else {
                        vec!["z", "a"]
                    },
                    "{field} {direction}"
                );
            }
        }
    }

    #[test]
    fn smart_last_played_sort_orders_never_values() {
        let mut tracks = vec![track("never"), track("old"), track("new"), track("tie")];
        tracks[1].last_played_at_ms = Some(1);
        tracks[2].last_played_at_ms = Some(2);
        tracks[3].last_played_at_ms = Some(2);
        let mut def = definition(json!([rule("playCount", "equals", json!(3))]));
        def.sort.field = SmartSortField::LastPlayedAtMs;
        assert_eq!(
            ids(&resolve(&def, &tracks, &HashSet::new(), 0)),
            ["never", "old", "new", "tie"]
        );
        def.sort.direction = SmartSortDirection::Desc;
        assert_eq!(
            ids(&resolve(&def, &tracks, &HashSet::new(), 0)),
            ["new", "tie", "old", "never"]
        );
    }

    #[test]
    fn smart_validation_rejects_invalid_definitions() {
        for invalid in [
            rule("unknown", "equals", json!("x")),
            rule("title", "within", json!(1)),
            rule("title", "contains", json!(true)),
            rule("title", "equals", json!("  ")),
            rule("title", "contains", json!("x".repeat(201))),
            rule("durationMs", "equals", json!(-1)),
            rule("playCount", "equals", json!(1.5)),
            rule("playCount", "equals", json!(9007199254740992_u64)),
            rule("playCount", "equals", json!("1")),
            rule("favorite", "contains", json!(true)),
            rule("favorite", "equals", json!(1)),
            rule("lastPlayedDays", "within", json!(0)),
            rule("lastPlayedDays", "within", json!(36501)),
            rule("lastPlayedDays", "notWithin", json!(1.5)),
            rule("lastPlayedDays", "equals", json!(1)),
            rule("durationMs", "equals", Value::Null),
        ] {
            let value = json!({"match": "all", "rules": [invalid], "sort": {"field": "title", "direction": "asc"}, "limit": null});
            assert!(
                serde_json::from_value::<SmartPlaylistDefinition>(value)
                    .and_then(|def| def.validate().map_err(serde::de::Error::custom))
                    .is_err()
            );
        }
        let mut def = definition(json!([]));
        assert!(def.validate().is_err());
        def.rules = vec![serde_json::from_value(rule("title", "equals", json!("x"))).unwrap(); 21];
        assert!(def.validate().is_err());
        def.rules.truncate(20);
        assert!(def.validate().is_ok());
        for limit in [0, 10001] {
            def.limit = Some(limit);
            assert!(def.validate().is_err());
        }
        def.limit = Some(10000);
        assert!(def.validate().is_ok());
        assert!(serde_json::from_str::<SmartPlaylistDefinition>(r#"{"match":"all","rules":[{"field":"playCount","operator":"equals","value":1e400}],"sort":{"field":"title","direction":"asc"},"limit":null}"#).is_err());
    }
    #[test]
    fn smart_sort_fields_compare_values_in_both_directions() {
        let mut tracks = vec![track("later"), track("earlier")];
        tracks[0].title = "Zulu".into();
        tracks[0].artist = "Zulu".into();
        tracks[0].album = Some("Zulu".into());
        tracks[0].duration_ms = 200000;
        tracks[0].play_count = 9;
        for field in ["title", "artist", "album", "durationMs", "playCount"] {
            for (direction, expected) in [
                ("asc", ["earlier", "later"]),
                ("desc", ["later", "earlier"]),
            ] {
                let mut def = definition(json!([rule("playCount", "greaterThan", json!(0))]));
                def.sort =
                    serde_json::from_value(json!({"field":field,"direction":direction})).unwrap();
                assert_eq!(
                    ids(&resolve(&def, &tracks, &HashSet::new(), 0)),
                    expected,
                    "{field} {direction}"
                );
            }
        }
    }

    #[test]
    fn smart_validation_accepts_boundaries_and_rejects_unknown_contracts() {
        for (field, operator, value) in [
            ("title", "equals", json!(" é ")),
            ("title", "equals", json!("é".repeat(200))),
            ("durationMs", "equals", json!(0)),
            ("playCount", "equals", json!(9007199254740991_u64)),
            ("playCount", "equals", json!(1.0)),
            ("lastPlayedDays", "notWithin", json!(36500)),
            ("favorite", "equals", json!(false)),
        ] {
            assert!(
                definition(json!([rule(field, operator, value)]))
                    .validate()
                    .is_ok()
            );
        }
        let value = serde_json::to_value(definition(json!([rule(
            "favorite",
            "equals",
            json!(false)
        )])))
        .unwrap();
        for (pointer, invalid) in [
            ("/match", json!("none")),
            ("/sort/field", json!("unknown")),
            ("/sort/direction", json!("sideways")),
            ("/limit", json!(-1)),
            ("/limit", json!(1.5)),
            ("/limit", json!(1e100)),
            ("/rules/0/operator", json!("unknown")),
            ("/rules/0/value", json!([])),
            ("/rules/0/value", json!({})),
        ] {
            let mut invalid_value = value.clone();
            *invalid_value.pointer_mut(pointer).unwrap() = invalid;
            assert!(
                serde_json::from_value::<SmartPlaylistDefinition>(invalid_value).is_err(),
                "{pointer}"
            );
        }
    }
}
