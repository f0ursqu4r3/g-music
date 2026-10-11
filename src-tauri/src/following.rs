//! Followed YouTube channels and their recent uploads.
//!
//! Identity is the validated YouTube channel ID. Display names never identify a follow.
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use rusqlite::OptionalExtension;
use serde::Serialize;

pub(crate) const MAX_FOLLOWED_ARTISTS: usize = 200;
pub(crate) const MAX_STORED_UPLOADS: usize = 50;
pub(crate) const FETCH_LIMIT: usize = 15;
pub(crate) const MAX_SEEN_IDS: usize = 1000;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FollowUpload {
    pub(crate) video_id: String,
    pub(crate) title: String,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) url: String,
    pub(crate) discovered_at_ms: u64,
    /// True only for uploads first seen after the follow baseline was established.
    pub(crate) is_new: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FollowedArtist {
    pub(crate) channel_id: String,
    pub(crate) name: String,
    pub(crate) followed_at_ms: u64,
    pub(crate) baseline_established: bool,
    pub(crate) last_checked_at_ms: Option<u64>,
    pub(crate) last_error: Option<String>,
    pub(crate) checking: bool,
    pub(crate) uploads: Vec<FollowUpload>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FollowingSnapshot {
    pub(crate) artists: Vec<FollowedArtist>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FetchedUpload {
    pub(crate) video_id: String,
    pub(crate) title: String,
    pub(crate) duration_ms: Option<u64>,
}

pub(crate) trait UploadFetcher: Send + Sync {
    fn fetch(&self, channel_id: &str) -> Result<Vec<FetchedUpload>, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FollowingError {
    Invalid(String),
    Busy,
    Storage(String),
}

pub(crate) fn validate_channel_id(value: &str) -> Result<String, FollowingError> {
    let valid = value.len() == 24
        && value.starts_with("UC")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if valid {
        Ok(value.to_owned())
    } else {
        Err(FollowingError::Invalid(
            "This artist has no valid YouTube channel ID. Refresh the track metadata and try again."
                .into(),
        ))
    }
}

pub(crate) fn validate_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(crate) fn channel_uploads_url(channel_id: &str) -> Result<String, FollowingError> {
    let channel_id = validate_channel_id(channel_id)?;
    Ok(format!(
        "https://www.youtube.com/channel/{channel_id}/videos"
    ))
}

fn storage(error: impl std::fmt::Display) -> FollowingError {
    FollowingError::Storage(error.to_string())
}

fn clean_name(name: &str) -> Result<String, FollowingError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 200 || name.chars().any(char::is_control) {
        return Err(FollowingError::Invalid(
            "The artist name must have 1 to 200 characters.".into(),
        ));
    }
    Ok(name.to_owned())
}

struct RunGuard<'a>(&'a AtomicBool);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct Subscription {
    id: i64,
    channel_id: String,
    last_checked_at_ms: Option<u64>,
}

pub(crate) struct FollowingService {
    database_path: PathBuf,
    running: AtomicBool,
    stopping: Arc<AtomicBool>,
    checking: Mutex<HashSet<String>>,
}

impl FollowingService {
    pub(crate) fn new(database_path: PathBuf, stopping: Arc<AtomicBool>) -> Self {
        Self {
            database_path,
            running: AtomicBool::new(false),
            stopping,
            checking: Mutex::new(HashSet::new()),
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub(crate) fn stopping(&self) -> Arc<AtomicBool> {
        self.stopping.clone()
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    fn set_checking(&self, channel_id: &str, checking: bool) {
        if let Ok(mut set) = self.checking.lock() {
            if checking {
                set.insert(channel_id.to_owned());
            } else {
                set.remove(channel_id);
            }
        }
    }

    pub(crate) fn snapshot(&self) -> Result<FollowingSnapshot, FollowingError> {
        let connection = crate::persistence::open(&self.database_path).map_err(storage)?;
        let checking = self
            .checking
            .lock()
            .map(|set| set.clone())
            .unwrap_or_default();
        let mut artists_statement = connection
            .prepare(
                "SELECT subscription, channel_id, name, followed_at_ms, baseline_at_ms,
                        last_checked_at_ms, last_error
                 FROM followed_artists ORDER BY name COLLATE NOCASE, channel_id",
            )
            .map_err(storage)?;
        let mut uploads_statement = connection
            .prepare(
                "SELECT video_id, title, duration_ms, discovered_at_ms, is_new
                 FROM followed_uploads WHERE subscription = ?1
                 ORDER BY discovered_at_ms DESC, rank ASC",
            )
            .map_err(storage)?;
        let rows = artists_statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    FollowedArtist {
                        channel_id: row.get(1)?,
                        name: row.get(2)?,
                        followed_at_ms: row.get(3)?,
                        baseline_established: row.get::<_, Option<u64>>(4)?.is_some(),
                        last_checked_at_ms: row.get(5)?,
                        last_error: row.get(6)?,
                        checking: false,
                        uploads: Vec::new(),
                    },
                ))
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        let mut artists = Vec::with_capacity(rows.len());
        for (subscription, mut artist) in rows {
            artist.checking = checking.contains(&artist.channel_id);
            artist.uploads = uploads_statement
                .query_map([subscription], |row| {
                    let video_id: String = row.get(0)?;
                    Ok(FollowUpload {
                        url: format!("https://www.youtube.com/watch?v={video_id}"),
                        video_id,
                        title: row.get(1)?,
                        duration_ms: row.get(2)?,
                        discovered_at_ms: row.get(3)?,
                        is_new: row.get::<_, i64>(4)? != 0,
                    })
                })
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
            artists.push(artist);
        }
        Ok(FollowingSnapshot { artists })
    }

    pub(crate) fn follow(
        &self,
        channel_id: &str,
        name: &str,
        now_ms: u64,
    ) -> Result<FollowingSnapshot, FollowingError> {
        let channel_id = validate_channel_id(channel_id)?;
        let name = clean_name(name)?;
        let mut connection = crate::persistence::open(&self.database_path).map_err(storage)?;
        let transaction = connection.transaction().map_err(storage)?;
        let known: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM followed_artists WHERE channel_id = ?1",
                [&channel_id],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let total: i64 = transaction
            .query_row("SELECT COUNT(*) FROM followed_artists", [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        if known == 0 && total as usize >= MAX_FOLLOWED_ARTISTS {
            return Err(FollowingError::Invalid(format!(
                "You can follow up to {MAX_FOLLOWED_ARTISTS} artists."
            )));
        }
        transaction
            .execute(
                "INSERT INTO followed_artists (channel_id, name, followed_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(channel_id) DO UPDATE SET name = excluded.name",
                rusqlite::params![channel_id, name, now_ms],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        self.snapshot()
    }

    pub(crate) fn unfollow(&self, channel_id: &str) -> Result<FollowingSnapshot, FollowingError> {
        let channel_id = validate_channel_id(channel_id)?;
        let connection = crate::persistence::open(&self.database_path).map_err(storage)?;
        connection
            .execute(
                "DELETE FROM followed_artists WHERE channel_id = ?1",
                [&channel_id],
            )
            .map_err(storage)?;
        self.snapshot()
    }

    fn subscriptions(&self) -> Result<Vec<Subscription>, FollowingError> {
        let connection = crate::persistence::open(&self.database_path).map_err(storage)?;
        let mut statement = connection
            .prepare("SELECT subscription, channel_id, last_checked_at_ms FROM followed_artists")
            .map_err(storage)?;
        statement
            .query_map([], |row| {
                Ok(Subscription {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    last_checked_at_ms: row.get(2)?,
                })
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)
    }

    /// Stores a check result. Returns false when the subscription no longer exists.
    fn record_result(
        &self,
        subscription: i64,
        result: Result<Vec<FetchedUpload>, String>,
        now_ms: u64,
    ) -> Result<bool, FollowingError> {
        let mut connection = crate::persistence::open(&self.database_path).map_err(storage)?;
        let transaction = connection.transaction().map_err(storage)?;
        let baseline: Option<Option<u64>> = transaction
            .query_row(
                "SELECT baseline_at_ms FROM followed_artists WHERE subscription = ?1",
                [subscription],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(baseline) = baseline else {
            return Ok(false);
        };
        match result {
            Err(message) => {
                transaction
                    .execute(
                        "UPDATE followed_artists SET last_error = ?2 WHERE subscription = ?1",
                        rusqlite::params![subscription, message],
                    )
                    .map_err(storage)?;
            }
            Ok(fetched) => {
                let is_new = baseline.is_some();
                let mut seen = HashSet::new();
                let mut rank = 0_i64;
                for upload in fetched {
                    if !validate_video_id(&upload.video_id) || !seen.insert(upload.video_id.clone())
                    {
                        continue;
                    }
                    let stored: bool = transaction
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM followed_uploads
                                 WHERE subscription = ?1 AND video_id = ?2)",
                            rusqlite::params![subscription, upload.video_id],
                            |row| row.get(0),
                        )
                        .map_err(storage)?;
                    // Seen history outlives the capped list. A pruned ID that returns is not new.
                    let newly_seen = transaction
                        .execute(
                            "INSERT OR IGNORE INTO followed_seen (subscription, video_id)
                             VALUES (?1, ?2)",
                            rusqlite::params![subscription, upload.video_id],
                        )
                        .map_err(storage)?
                        > 0;
                    if stored {
                        transaction
                            .execute(
                                "UPDATE followed_uploads SET title = ?3, duration_ms = ?4, rank = ?5
                                 WHERE subscription = ?1 AND video_id = ?2",
                                rusqlite::params![
                                    subscription,
                                    upload.video_id,
                                    upload.title,
                                    upload.duration_ms,
                                    rank
                                ],
                            )
                            .map_err(storage)?;
                    } else if newly_seen {
                        transaction
                            .execute(
                                "INSERT INTO followed_uploads
                                     (subscription, video_id, title, duration_ms, is_new,
                                      discovered_at_ms, rank)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                rusqlite::params![
                                    subscription,
                                    upload.video_id,
                                    upload.title,
                                    upload.duration_ms,
                                    is_new,
                                    now_ms,
                                    rank
                                ],
                            )
                            .map_err(storage)?;
                    }
                    rank += 1;
                }
                transaction
                    .execute(
                        "DELETE FROM followed_uploads WHERE subscription = ?1 AND video_id NOT IN (
                             SELECT video_id FROM followed_uploads WHERE subscription = ?1
                             ORDER BY discovered_at_ms DESC, rank ASC LIMIT ?2)",
                        rusqlite::params![subscription, MAX_STORED_UPLOADS as i64],
                    )
                    .map_err(storage)?;
                transaction
                    .execute(
                        "DELETE FROM followed_seen WHERE subscription = ?1 AND seq NOT IN (
                             SELECT seq FROM followed_seen WHERE subscription = ?1
                             ORDER BY seq DESC LIMIT ?2)",
                        rusqlite::params![subscription, MAX_SEEN_IDS as i64],
                    )
                    .map_err(storage)?;
                transaction
                    .execute(
                        "UPDATE followed_artists SET last_error = NULL, last_checked_at_ms = ?2,
                             baseline_at_ms = COALESCE(baseline_at_ms, ?2)
                         WHERE subscription = ?1",
                        rusqlite::params![subscription, now_ms],
                    )
                    .map_err(storage)?;
            }
        }
        transaction.commit().map_err(storage)?;
        Ok(true)
    }

    /// Checks one followed channel, or all of them in oldest-checked order. Never overlaps.
    pub(crate) fn check(
        &self,
        fetcher: &dyn UploadFetcher,
        target: Option<&str>,
        now_ms: impl Fn() -> u64,
        notify: &dyn Fn(),
    ) -> Result<(), FollowingError> {
        let mut subscriptions = self.subscriptions()?;
        if let Some(target) = target {
            let target = validate_channel_id(target)?;
            subscriptions.retain(|subscription| subscription.channel_id == target);
            if subscriptions.is_empty() {
                return Err(FollowingError::Invalid(
                    "This artist is not followed.".into(),
                ));
            }
        }
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(FollowingError::Busy);
        }
        let _run = RunGuard(&self.running);
        subscriptions
            .sort_by_key(|subscription| (subscription.last_checked_at_ms, subscription.id));
        for subscription in subscriptions {
            if self.is_stopping() {
                break;
            }
            self.set_checking(&subscription.channel_id, true);
            notify();
            // No lock is held while the fetcher uses the network.
            let result = fetcher.fetch(&subscription.channel_id);
            self.set_checking(&subscription.channel_id, false);
            if !self.is_stopping() {
                let _ = self.record_result(subscription.id, result, now_ms())?;
            }
            notify();
        }
        Ok(())
    }
}

pub(crate) fn run_poller(
    service: &FollowingService,
    fetcher: &dyn UploadFetcher,
    interval: Duration,
    now_ms: impl Fn() -> u64,
    notify: &dyn Fn(),
) {
    while !service.is_stopping() {
        if let Err(FollowingError::Storage(message)) = service.check(fetcher, None, &now_ms, notify)
        {
            tracing::warn!(%message, "followed artist check could not save results");
        }
        let deadline = Instant::now() + interval;
        while !service.is_stopping() && Instant::now() < deadline {
            thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::atomic::AtomicUsize,
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    const CHANNEL: &str = "UCMYSjwig4-XrzqY6Ka28t4g";
    const OTHER: &str = "UC_x5XG1OV2P6uZZ5FSM9Ttw";

    struct Fixture {
        directory: PathBuf,
        path: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let directory = std::env::temp_dir().join(format!(
                "gmusic-following-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&directory).unwrap();
            Self {
                path: directory.join("library.sqlite3"),
                directory,
            }
        }

        fn service(&self) -> FollowingService {
            FollowingService::new(self.path.clone(), Arc::new(AtomicBool::new(false)))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    fn upload(id: &str) -> FetchedUpload {
        FetchedUpload {
            video_id: id.into(),
            title: format!("Title {id}"),
            duration_ms: Some(180_000),
        }
    }

    type Responses = Mutex<HashMap<String, Result<Vec<FetchedUpload>, String>>>;

    #[derive(Default)]
    struct FakeFetcher {
        responses: Responses,
        calls: AtomicUsize,
    }

    impl FakeFetcher {
        fn set(&self, channel: &str, response: Result<Vec<FetchedUpload>, String>) {
            self.responses
                .lock()
                .unwrap()
                .insert(channel.into(), response);
        }
    }

    impl UploadFetcher for FakeFetcher {
        fn fetch(&self, channel_id: &str) -> Result<Vec<FetchedUpload>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.responses
                .lock()
                .unwrap()
                .get(channel_id)
                .cloned()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    fn check_all(service: &FollowingService, fetcher: &dyn UploadFetcher, now: u64) {
        service.check(fetcher, None, || now, &|| {}).unwrap();
    }

    #[test]
    fn accepts_only_stable_channel_ids() {
        assert_eq!(validate_channel_id(CHANNEL).as_deref(), Ok(CHANNEL));
        for bad in [
            "",
            "@lemurian.archives",
            "Lemuria",
            "UCshort",
            "uc MYSjwig4-XrzqY6Ka28t4g",
            "ucMYSjwig4-XrzqY6Ka28t4g",
            "UCMYSjwig4-XrzqY6Ka28t4g\n",
            " UCMYSjwig4-XrzqY6Ka28t4g",
            "UCMYSjwig4-XrzqY6Ka28t4gX",
            "UCMYSjwig4/XrzqY6Ka28t4g",
            "https://www.youtube.com/channel/UCMYSjwig4-XrzqY6Ka28t4g",
            "PLMYSjwig4-XrzqY6Ka28t4g",
        ] {
            assert!(
                matches!(validate_channel_id(bad), Err(FollowingError::Invalid(_))),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn accepts_only_eleven_character_video_ids() {
        assert!(validate_video_id("1dcns_zjbbs"));
        assert!(!validate_video_id("short"));
        assert!(!validate_video_id("1dcns_zjbbs1"));
        assert!(!validate_video_id("1dcns zjbbs"));
        assert!(!validate_video_id("../../etc/p"));
    }

    #[test]
    fn builds_the_uploads_url_only_from_a_validated_id() {
        assert_eq!(
            channel_uploads_url(CHANNEL).unwrap(),
            format!("https://www.youtube.com/channel/{CHANNEL}/videos")
        );
        assert!(channel_uploads_url("@lemurian.archives").is_err());
        assert!(channel_uploads_url("UCx/../../evil?x=1&yy=22222").is_err());
    }

    #[test]
    fn follow_rejects_malformed_identity_and_empty_names() {
        let fixture = Fixture::new("malformed");
        let service = fixture.service();
        assert!(matches!(
            service.follow("@lemurian.archives", "Lemuria", 1),
            Err(FollowingError::Invalid(_))
        ));
        assert!(matches!(
            service.follow(CHANNEL, "   ", 1),
            Err(FollowingError::Invalid(_))
        ));
        assert!(matches!(
            service.follow(CHANNEL, &"x".repeat(300), 1),
            Err(FollowingError::Invalid(_))
        ));
        assert!(service.snapshot().unwrap().artists.is_empty());
    }

    #[test]
    fn follows_survive_restart_and_deduplicate_by_channel_id() {
        let fixture = Fixture::new("restart");
        {
            let service = fixture.service();
            service.follow(CHANNEL, "Lemuria", 10).unwrap();
            let again = service.follow(CHANNEL, "Lemuria (renamed)", 99).unwrap();
            assert_eq!(again.artists.len(), 1);
            assert_eq!(again.artists[0].followed_at_ms, 10);
        }
        let restarted = fixture.service().snapshot().unwrap();
        assert_eq!(restarted.artists.len(), 1);
        assert_eq!(restarted.artists[0].channel_id, CHANNEL);
        assert_eq!(restarted.artists[0].name, "Lemuria (renamed)");
        assert!(!restarted.artists[0].baseline_established);
    }

    #[test]
    fn rejects_following_more_than_the_limit() {
        let fixture = Fixture::new("limit");
        let service = fixture.service();
        for index in 0..MAX_FOLLOWED_ARTISTS {
            service
                .follow(&format!("UC{index:022}"), "Artist", 1)
                .unwrap();
        }
        assert!(matches!(
            service.follow(CHANNEL, "One too many", 1),
            Err(FollowingError::Invalid(_))
        ));
    }

    #[test]
    fn first_check_is_a_baseline_and_later_uploads_are_new() {
        let fixture = Fixture::new("baseline");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(
            CHANNEL,
            Ok(vec![upload("aaaaaaaaaaa"), upload("bbbbbbbbbbb")]),
        );
        check_all(&service, &fetcher, 100);

        let artist = &service.snapshot().unwrap().artists[0];
        assert!(artist.baseline_established);
        assert_eq!(artist.last_checked_at_ms, Some(100));
        assert_eq!(artist.uploads.len(), 2);
        assert!(artist.uploads.iter().all(|upload| !upload.is_new));
        assert_eq!(
            artist.uploads[0].url,
            "https://www.youtube.com/watch?v=aaaaaaaaaaa"
        );

        fetcher.set(
            CHANNEL,
            Ok(vec![
                upload("ccccccccccc"),
                upload("aaaaaaaaaaa"),
                upload("bbbbbbbbbbb"),
            ]),
        );
        check_all(&service, &fetcher, 200);
        let artist = &service.snapshot().unwrap().artists[0];
        assert_eq!(artist.uploads.len(), 3);
        assert_eq!(artist.uploads[0].video_id, "ccccccccccc");
        assert!(artist.uploads[0].is_new);
        assert_eq!(artist.uploads[0].discovered_at_ms, 200);
        assert!(!artist.uploads[1].is_new);
        assert!(!artist.uploads[2].is_new);

        // A repeated check keeps one row per video and the same new flag.
        check_all(&service, &fetcher, 300);
        let artist = &service.snapshot().unwrap().artists[0];
        assert_eq!(artist.uploads.len(), 3);
        assert_eq!(artist.uploads[0].discovered_at_ms, 200);
        assert!(artist.uploads[0].is_new);
    }

    #[test]
    fn duplicate_video_ids_in_one_result_are_stored_once() {
        let fixture = Fixture::new("duplicates");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(
            CHANNEL,
            Ok(vec![
                upload("aaaaaaaaaaa"),
                upload("aaaaaaaaaaa"),
                upload("bbbbbbbbbbb"),
            ]),
        );
        check_all(&service, &fetcher, 100);
        assert_eq!(service.snapshot().unwrap().artists[0].uploads.len(), 2);
    }

    #[test]
    fn invalid_video_ids_are_not_stored() {
        let fixture = Fixture::new("bad-video");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("nope"), upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        let uploads = &service.snapshot().unwrap().artists[0].uploads;
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].video_id, "aaaaaaaaaaa");
    }

    #[test]
    fn stored_uploads_are_capped_keeping_the_newest() {
        let fixture = Fixture::new("cap");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        for round in 0..6 {
            let batch = (0..FETCH_LIMIT)
                .map(|index| upload(&format!("r{round}-{index:08}")))
                .collect();
            fetcher.set(CHANNEL, Ok(batch));
            check_all(&service, &fetcher, 200 + round as u64);
        }
        let uploads = &service.snapshot().unwrap().artists[0].uploads;
        assert_eq!(uploads.len(), MAX_STORED_UPLOADS);
        assert!(uploads[0].video_id.starts_with("r5-"));
        assert!(
            uploads
                .iter()
                .all(|upload| upload.video_id != "aaaaaaaaaaa")
        );
    }

    #[test]
    fn a_pruned_upload_that_reappears_is_not_marked_new() {
        let fixture = Fixture::new("seen");
        let fetcher = FakeFetcher::default();
        {
            let service = fixture.service();
            service.follow(CHANNEL, "Lemuria", 1).unwrap();
            fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
            check_all(&service, &fetcher, 100);
            for round in 0..6 {
                let batch = (0..FETCH_LIMIT)
                    .map(|index| upload(&format!("r{round}-{index:08}")))
                    .collect();
                fetcher.set(CHANNEL, Ok(batch));
                check_all(&service, &fetcher, 200 + round as u64);
            }
            let uploads = &service.snapshot().unwrap().artists[0].uploads;
            assert!(
                uploads
                    .iter()
                    .all(|upload| upload.video_id != "aaaaaaaaaaa")
            );
        }
        // The older ID returns after a restart. It was seen before, so it is not new.
        let service = fixture.service();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 900);
        let uploads = &service.snapshot().unwrap().artists[0].uploads;
        assert!(
            uploads
                .iter()
                .all(|upload| !(upload.video_id == "aaaaaaaaaaa" && upload.is_new))
        );
        assert!(uploads.iter().all(|upload| upload.discovered_at_ms != 900));
    }

    #[test]
    fn seen_history_is_compact_bounded_and_removed_on_unfollow() {
        let fixture = Fixture::new("seen-cleanup");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        let count = || -> i64 {
            crate::persistence::open(&fixture.path)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM followed_seen", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(count(), 1);
        for round in 0..(MAX_SEEN_IDS / FETCH_LIMIT + 5) {
            let batch = (0..FETCH_LIMIT)
                .map(|index| upload(&format!("{round:04}-{index:06}")))
                .collect();
            fetcher.set(CHANNEL, Ok(batch));
            check_all(&service, &fetcher, 200 + round as u64);
        }
        assert_eq!(count() as usize, MAX_SEEN_IDS);
        service.unfollow(CHANNEL).unwrap();
        assert_eq!(count(), 0);
    }

    #[test]
    fn failed_check_keeps_results_and_reports_the_error() {
        let fixture = Fixture::new("failure");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        fetcher.set(CHANNEL, Err("Dependency timed out.".into()));
        check_all(&service, &fetcher, 200);

        let artist = &service.snapshot().unwrap().artists[0];
        assert_eq!(artist.uploads.len(), 1);
        assert_eq!(artist.last_checked_at_ms, Some(100));
        assert_eq!(artist.last_error.as_deref(), Some("Dependency timed out."));
        assert!(!artist.checking);

        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 300);
        let artist = &service.snapshot().unwrap().artists[0];
        assert_eq!(artist.last_error, None);
        assert_eq!(artist.last_checked_at_ms, Some(300));
    }

    #[test]
    fn one_failing_channel_does_not_stop_the_others() {
        let fixture = Fixture::new("isolated");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        service.follow(OTHER, "Google", 2).unwrap();
        fetcher.set(CHANNEL, Err("boom".into()));
        fetcher.set(OTHER, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        let artists = service.snapshot().unwrap().artists;
        let other = artists.iter().find(|a| a.channel_id == OTHER).unwrap();
        assert_eq!(other.uploads.len(), 1);
        let first = artists.iter().find(|a| a.channel_id == CHANNEL).unwrap();
        assert_eq!(first.last_error.as_deref(), Some("boom"));
    }

    #[test]
    fn unfollow_during_a_check_does_not_recreate_the_subscription() {
        let fixture = Fixture::new("race");
        let service = Arc::new(fixture.service());
        service.follow(CHANNEL, "Lemuria", 1).unwrap();

        struct UnfollowingFetcher(Arc<FollowingService>);
        impl UploadFetcher for UnfollowingFetcher {
            fn fetch(&self, channel_id: &str) -> Result<Vec<FetchedUpload>, String> {
                self.0.unfollow(channel_id).unwrap();
                Ok(vec![upload("aaaaaaaaaaa")])
            }
        }
        check_all(&service, &UnfollowingFetcher(service.clone()), 100);

        assert!(service.snapshot().unwrap().artists.is_empty());
        let connection = crate::persistence::open(&fixture.path).unwrap();
        for table in ["followed_artists", "followed_uploads", "followed_seen"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} must stay empty");
        }
    }

    #[test]
    fn unfollow_then_refollow_during_a_check_ignores_the_stale_result() {
        let fixture = Fixture::new("refollow");
        let service = Arc::new(fixture.service());
        service.follow(CHANNEL, "Lemuria", 1).unwrap();

        struct RefollowingFetcher(Arc<FollowingService>);
        impl UploadFetcher for RefollowingFetcher {
            fn fetch(&self, channel_id: &str) -> Result<Vec<FetchedUpload>, String> {
                self.0.unfollow(channel_id).unwrap();
                self.0.follow(channel_id, "Lemuria", 50).unwrap();
                Ok(vec![upload("aaaaaaaaaaa")])
            }
        }
        check_all(&service, &RefollowingFetcher(service.clone()), 100);
        let artist = &service.snapshot().unwrap().artists[0];
        assert!(!artist.baseline_established);
        assert!(artist.uploads.is_empty());
    }

    #[test]
    fn unfollow_removes_uploads_and_survives_restart() {
        let fixture = Fixture::new("unfollow");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);
        service.unfollow(CHANNEL).unwrap();
        assert!(fixture.service().snapshot().unwrap().artists.is_empty());
    }

    #[test]
    fn uploads_and_baseline_survive_restart() {
        let fixture = Fixture::new("durable");
        let fetcher = FakeFetcher::default();
        {
            let service = fixture.service();
            service.follow(CHANNEL, "Lemuria", 1).unwrap();
            fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
            check_all(&service, &fetcher, 100);
            fetcher.set(
                CHANNEL,
                Ok(vec![upload("bbbbbbbbbbb"), upload("aaaaaaaaaaa")]),
            );
            check_all(&service, &fetcher, 200);
        }
        let artist = &fixture.service().snapshot().unwrap().artists[0];
        assert!(artist.baseline_established);
        assert_eq!(artist.uploads.len(), 2);
        assert!(artist.uploads[0].is_new);
    }

    #[test]
    fn followed_artists_survive_library_saves_and_backup_export() {
        let fixture = Fixture::new("library");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        fetcher.set(CHANNEL, Ok(vec![upload("aaaaaaaaaaa")]));
        check_all(&service, &fetcher, 100);

        crate::persistence::save_library(&fixture.path, &[], &[]).unwrap();
        assert_eq!(service.snapshot().unwrap().artists.len(), 1);

        let backup = crate::persistence::export_library_backup(&fixture.path).unwrap();
        let restored = FollowingService::new(backup, Arc::new(AtomicBool::new(false)));
        let artists = restored.snapshot().unwrap().artists;
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].uploads.len(), 1);
    }

    #[test]
    fn concurrent_checks_never_overlap() {
        let fixture = Fixture::new("overlap");
        let service = Arc::new(fixture.service());
        service.follow(CHANNEL, "Lemuria", 1).unwrap();

        struct BlockingFetcher {
            entered: Arc<AtomicBool>,
            release: Arc<AtomicBool>,
        }
        impl UploadFetcher for BlockingFetcher {
            fn fetch(&self, _: &str) -> Result<Vec<FetchedUpload>, String> {
                self.entered.store(true, Ordering::Release);
                while !self.release.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(Vec::new())
            }
        }
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let worker = {
            let service = service.clone();
            let fetcher = BlockingFetcher {
                entered: entered.clone(),
                release: release.clone(),
            };
            thread::spawn(move || service.check(&fetcher, None, || 1, &|| {}))
        };
        while !entered.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(service.is_running());
        assert!(service.snapshot().unwrap().artists[0].checking);
        let second = FakeFetcher::default();
        assert_eq!(
            service.check(&second, None, || 2, &|| {}),
            Err(FollowingError::Busy)
        );
        assert_eq!(second.calls.load(Ordering::Relaxed), 0);
        release.store(true, Ordering::Release);
        worker.join().unwrap().unwrap();
        assert!(!service.is_running());
        assert!(!service.snapshot().unwrap().artists[0].checking);
    }

    #[test]
    fn a_check_stops_between_channels_when_the_app_is_stopping() {
        let fixture = Fixture::new("stopping");
        let stopping = Arc::new(AtomicBool::new(false));
        let service = FollowingService::new(fixture.path.clone(), stopping.clone());
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        service.follow(OTHER, "Google", 2).unwrap();

        struct StoppingFetcher(Arc<AtomicBool>, AtomicUsize);
        impl UploadFetcher for StoppingFetcher {
            fn fetch(&self, _: &str) -> Result<Vec<FetchedUpload>, String> {
                self.1.fetch_add(1, Ordering::Relaxed);
                self.0.store(true, Ordering::Release);
                Ok(Vec::new())
            }
        }
        let fetcher = StoppingFetcher(stopping, AtomicUsize::new(0));
        service.check(&fetcher, None, || 1, &|| {}).unwrap();
        assert_eq!(fetcher.1.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn checking_an_unknown_channel_is_rejected_without_network() {
        let fixture = Fixture::new("unknown");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        assert!(matches!(
            service.check(&fetcher, Some(CHANNEL), || 1, &|| {}),
            Err(FollowingError::Invalid(_))
        ));
        assert!(matches!(
            service.check(&fetcher, Some("@handle"), || 1, &|| {}),
            Err(FollowingError::Invalid(_))
        ));
        assert_eq!(fetcher.calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn notifies_after_each_channel_state_change() {
        let fixture = Fixture::new("notify");
        let service = fixture.service();
        let fetcher = FakeFetcher::default();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        let notified = AtomicUsize::new(0);
        service
            .check(&fetcher, None, || 1, &|| {
                notified.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        // One notification when checking starts and one when it finishes.
        assert_eq!(notified.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn poller_checks_on_startup_then_repeats_until_stopped() {
        let fixture = Fixture::new("poller");
        let stopping = Arc::new(AtomicBool::new(false));
        let service = Arc::new(FollowingService::new(
            fixture.path.clone(),
            stopping.clone(),
        ));
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        let fetcher = Arc::new(FakeFetcher::default());
        let worker = {
            let service = service.clone();
            let fetcher = fetcher.clone();
            thread::spawn(move || {
                run_poller(&service, &*fetcher, Duration::from_millis(40), || 1, &|| {})
            })
        };
        let started = Instant::now();
        while fetcher.calls.load(Ordering::Relaxed) < 3
            && started.elapsed() < Duration::from_secs(5)
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(fetcher.calls.load(Ordering::Relaxed) >= 3);
        stopping.store(true, Ordering::Release);
        let stop_started = Instant::now();
        worker.join().unwrap();
        assert!(stop_started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn poller_stops_promptly_during_a_long_wait() {
        let fixture = Fixture::new("poller-wait");
        let stopping = Arc::new(AtomicBool::new(false));
        let service = Arc::new(FollowingService::new(
            fixture.path.clone(),
            stopping.clone(),
        ));
        let fetcher = Arc::new(FakeFetcher::default());
        let worker = {
            let service = service.clone();
            let fetcher = fetcher.clone();
            thread::spawn(move || {
                run_poller(&service, &*fetcher, Duration::from_secs(3600), || 1, &|| {})
            })
        };
        thread::sleep(Duration::from_millis(100));
        stopping.store(true, Ordering::Release);
        let stop_started = Instant::now();
        worker.join().unwrap();
        assert!(stop_started.elapsed() < Duration::from_secs(2));
    }
}
