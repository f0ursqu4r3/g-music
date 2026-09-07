use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::Client;
use tauri::{
    AppHandle, Manager,
    async_runtime::{self, Receiver, Sender},
};

const ARTWORK_CACHE_LIMIT: u64 = 512 * 1024 * 1024;
const ARTWORK_MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_WORK: usize = 4;
const MAX_FAILURES: usize = 256;
const FAILURE_TTL_MS: u64 = 5_000;
const QUALITIES: [&str; 2] = ["maxresdefault", "hqdefault"];

type ArtworkResult = Result<Option<String>, String>;
type Key = (PathBuf, String);
type Fetch = dyn Fn(&str, &str) -> Result<Option<Vec<u8>>, String> + Send + Sync;
type SharedFlight = Arc<async_runtime::Mutex<Flight>>;

pub(crate) async fn resolve_youtube_artwork(app: &AppHandle, video_id: &str) -> ArtworkResult {
    if !is_youtube_video_id(video_id) {
        return Ok(None);
    }
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    // One resolver for all native windows. Client creation happens only in blocking work.
    static RESOLVER: OnceLock<Arc<ArtworkResolver>> = OnceLock::new();
    RESOLVER
        .get_or_init(|| {
            let start = Instant::now();
            ArtworkResolver::new(
                fetch_artwork,
                move || start.elapsed().as_millis() as u64,
                ARTWORK_CACHE_LIMIT,
            )
        })
        .resolve(directory, video_id.to_owned())
        .await
}

async fn run_blocking_artwork_task<T>(
    task: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| error.to_string())
}

enum Flight {
    Running(async_runtime::JoinHandle<ArtworkResult>),
    Done(ArtworkResult),
}

struct WorkPermit(Sender<()>);
impl Drop for WorkPermit {
    fn drop(&mut self) {
        let _ = self.0.try_send(());
    }
}

struct ArtworkResolver {
    fetch: Box<Fetch>,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    cache_limit: u64,
    flights: Mutex<HashMap<Key, SharedFlight>>,
    failures: Mutex<VecDeque<(Key, u64)>>,
    available: async_runtime::Mutex<Receiver<()>>,
    permits: Sender<()>,
    // Protect file/index updates and eviction, never network requests.
    disk: Mutex<()>,
}

impl ArtworkResolver {
    fn new(
        fetch: impl Fn(&str, &str) -> Result<Option<Vec<u8>>, String> + Send + Sync + 'static,
        clock: impl Fn() -> u64 + Send + Sync + 'static,
        cache_limit: u64,
    ) -> Arc<Self> {
        let (permits, available) = async_runtime::channel(MAX_WORK);
        for _ in 0..MAX_WORK {
            permits.try_send(()).expect("initial work permits fit");
        }
        Arc::new(Self {
            fetch: Box::new(fetch),
            clock: Box::new(clock),
            cache_limit,
            flights: Mutex::new(HashMap::new()),
            failures: Mutex::new(VecDeque::new()),
            available: async_runtime::Mutex::new(available),
            permits,
            disk: Mutex::new(()),
        })
    }

    async fn resolve(self: &Arc<Self>, directory: PathBuf, video_id: String) -> ArtworkResult {
        if !is_youtube_video_id(&video_id) {
            return Ok(None);
        }
        let key = (directory, video_id);
        let existing = self.flights.lock().unwrap().get(&key).cloned();
        let flight = if let Some(flight) = existing {
            flight
        } else {
            // Wait in the caller's async future, not a blocking worker or an
            // unbounded map of spawned jobs. A large view must not lose images.
            self.available
                .lock()
                .await
                .recv()
                .await
                .expect("resolver owns permit sender");
            let permit = WorkPermit(self.permits.clone());
            let mut flights = self.flights.lock().unwrap();
            if let Some(flight) = flights.get(&key) {
                flight.clone()
            } else {
                let resolver = self.clone();
                let task_key = key.clone();
                let task = async_runtime::spawn(async move {
                    let _permit = permit;
                    let worker = resolver.clone();
                    let worker_key = task_key.clone();
                    let result = run_blocking_artwork_task(move || {
                        worker.resolve_blocking(&worker_key.0, &worker_key.1)
                    })
                    .await
                    .and_then(|result| result);
                    resolver.flights.lock().unwrap().remove(&task_key);
                    result
                });
                let flight = Arc::new(async_runtime::Mutex::new(Flight::Running(task)));
                flights.insert(key, flight.clone());
                flight
            }
        };
        // Keep the task in the shared state if a command waiter is cancelled.
        let mut flight = flight.lock().await;
        if let Flight::Running(task) = &mut *flight {
            let result = task
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            *flight = Flight::Done(result);
        }
        match &*flight {
            Flight::Done(result) => result.clone(),
            Flight::Running(_) => unreachable!(),
        }
    }

    fn resolve_blocking(&self, application_directory: &Path, video_id: &str) -> ArtworkResult {
        let database_path = application_directory.join("library.sqlite3");
        let artwork_directory = application_directory.join("artwork");
        {
            let _disk = self.disk.lock().unwrap();
            fs::create_dir_all(&artwork_directory).map_err(|error| error.to_string())?;
            fs::set_permissions(&artwork_directory, fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
            // Search BOTH qualities on disk before any network or failure backoff.
            for quality in QUALITIES {
                let cache_key = format!("youtube:{video_id}:{quality}");
                let path = artwork_directory.join(format!("{video_id}-{quality}.jpg"));
                if crate::persistence::read_artwork_cache(
                    &database_path,
                    &cache_key,
                    now_epoch_ms(),
                )?
                .is_some()
                {
                    if is_valid_cached_image(&path) {
                        return Ok(Some(path.to_string_lossy().into()));
                    }
                    let _ = fs::remove_file(&path);
                    crate::persistence::remove_artwork_cache(&database_path, &cache_key)?;
                }
            }
        }
        let key = (application_directory.to_owned(), video_id.to_owned());
        {
            let mut failures = self.failures.lock().unwrap();
            let now = (self.clock)();
            failures.retain(|(_, until)| *until > now);
            if failures.iter().any(|(failed, _)| failed == &key) {
                return Ok(None);
            }
        }
        for quality in QUALITIES {
            // A timeout, bad response, or missing maxres must still try HQ.
            let Ok(Some(bytes)) = (self.fetch)(video_id, quality) else {
                continue;
            };
            if bytes.len() > ARTWORK_MAX_BYTES as usize || !is_image_signature(&bytes) {
                continue;
            }
            let _disk = self.disk.lock().unwrap();
            let cache_key = format!("youtube:{video_id}:{quality}");
            let relative_path = format!("artwork/{video_id}-{quality}.jpg");
            let path = application_directory.join(&relative_path);
            write_image_atomically(&path, &bytes)?;
            let evicted = match crate::persistence::write_artwork_cache(
                &database_path,
                &cache_key,
                &relative_path,
                bytes.len() as u64,
                now_epoch_ms(),
                self.cache_limit,
            ) {
                Ok(evicted) => evicted,
                Err(error) => {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
            };
            for relative in &evicted {
                let _ = fs::remove_file(application_directory.join(relative));
            }
            return Ok((!evicted.contains(&relative_path)).then(|| path.to_string_lossy().into()));
        }
        let mut failures = self.failures.lock().unwrap();
        if failures.len() >= MAX_FAILURES {
            failures.pop_front();
        }
        failures.push_back((key, (self.clock)().saturating_add(FAILURE_TTL_MS)));
        Ok(None)
    }
}

fn fetch_artwork(video_id: &str, quality: &str) -> Result<Option<Vec<u8>>, String> {
    // Building/dropping reqwest's blocking client inside an async runtime can panic.
    // Only blocking workers reach this pool; failed construction can be retried.
    static CLIENT: Mutex<Option<Client>> = Mutex::new(None);
    let client = {
        let mut shared = CLIENT.lock().unwrap();
        if shared.is_none() {
            *shared = Some(
                Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .map_err(|error| error.to_string())?,
            );
        }
        shared.as_ref().unwrap().clone()
    };
    fetch_url(
        &client,
        &format!("https://i.ytimg.com/vi/{video_id}/{quality}.jpg"),
    )
}

fn fetch_url(client: &Client, url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = client.get(url).send().map_err(|error| error.to_string())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > ARTWORK_MAX_BYTES)
    {
        return Ok(None);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("image/") {
        return Ok(None);
    }
    read_image(response)
}

fn read_image(reader: impl Read) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    reader
        .take(ARTWORK_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok((bytes.len() <= ARTWORK_MAX_BYTES as usize && is_image_signature(&bytes)).then_some(bytes))
}

struct TemporaryImage(PathBuf);
impl Drop for TemporaryImage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_image_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    for _ in 0..16 {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temporary_path = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
        let mut file = match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        };
        let temporary = TemporaryImage(temporary_path);
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary.0, path).map_err(|error| error.to_string())?;
        return Ok(());
    }
    Err("Could not allocate an artwork temporary file".into())
}

fn is_valid_cached_image(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > ARTWORK_MAX_BYTES {
        return false;
    }
    // Signature validation only needs 12 bytes, even for an oversized/sparse file.
    let mut signature = [0; 12];
    let Ok(count) = file.read(&mut signature) else {
        return false;
    };
    is_image_signature(&signature[..count])
}

fn is_image_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn is_youtube_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}
#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use std::sync::{
        Condvar, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    };
    use std::time::Duration;

    #[test]
    fn accepts_only_stable_youtube_video_ids() {
        assert!(is_youtube_video_id("M7lc1UVf-VE"));
        assert!(!is_youtube_video_id("../bad"));
    }

    #[test]
    fn validates_supported_image_signatures() {
        assert!(is_image_signature(&[0xff, 0xd8, 0xff, 0xe0]));
        assert!(is_image_signature(b"\x89PNG\r\n\x1a\nimage"));
        assert!(!is_image_signature(b"not an image"));
    }

    #[test]
    fn runs_artwork_work_off_the_calling_thread() {
        let calling_thread = thread::current().id();
        let worker_thread =
            tauri::async_runtime::block_on(run_blocking_artwork_task(|| thread::current().id()))
                .expect("artwork work completes");

        assert_ne!(worker_thread, calling_thread);
    }
    const ID: &str = "M7lc1UVf-VE";
    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0];
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "gmusic-artwork-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("artwork")).unwrap();
            Self(path)
        }
        fn seed(&self, id: &str, quality: &str, bytes: &[u8], time: u64) -> PathBuf {
            let relative = format!("artwork/{id}-{quality}.jpg");
            let path = self.0.join(&relative);
            fs::write(&path, bytes).unwrap();
            crate::persistence::write_artwork_cache(
                &self.0.join("library.sqlite3"),
                &format!("youtube:{id}:{quality}"),
                &relative,
                bytes.len() as u64,
                time,
                ARTWORK_CACHE_LIMIT,
            )
            .unwrap();
            path
        }
        fn resolve(
            &self,
            resolver: &Arc<ArtworkResolver>,
            id: &str,
        ) -> Result<Option<String>, String> {
            tauri::async_runtime::block_on(resolver.resolve(self.0.clone(), id.into()))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn local_hq_wins_before_any_network_even_with_a_fresh_resolver() {
        let fixture = Fixture::new();
        let path = fixture.seed(ID, "hqdefault", JPEG, 1);
        for _ in 0..2 {
            let calls = Arc::new(AtomicUsize::new(0));
            let count = calls.clone();
            let resolver = ArtworkResolver::new(
                move |_, _| {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err("offline".into())
                },
                || 10,
                ARTWORK_CACHE_LIMIT,
            );
            let result = fixture.resolve(&resolver, ID);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert_eq!(result.unwrap(), Some(path.to_string_lossy().into()));
        }
    }

    #[test]
    fn fetch_error_uses_hq_fallback() {
        let fixture = Fixture::new();
        let resolver = ArtworkResolver::new(
            |_, quality| {
                if quality == "maxresdefault" {
                    Err("timeout".into())
                } else {
                    Ok(Some(JPEG.to_vec()))
                }
            },
            || 10,
            ARTWORK_CACHE_LIMIT,
        );
        assert!(
            fixture
                .resolve(&resolver, ID)
                .unwrap()
                .unwrap()
                .ends_with("hqdefault.jpg")
        );
    }

    #[test]
    fn failure_cooldown_expires_and_never_hides_local_files() {
        let fixture = Fixture::new();
        let clock = Arc::new(AtomicU64::new(100));
        let now = clock.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            },
            move || now.load(Ordering::SeqCst),
            ARTWORK_CACHE_LIMIT,
        );
        assert_eq!(fixture.resolve(&resolver, ID).unwrap(), None);
        assert_eq!(fixture.resolve(&resolver, ID).unwrap(), None);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let path = fixture.seed(ID, "hqdefault", JPEG, 1);
        assert_eq!(
            fixture.resolve(&resolver, ID).unwrap(),
            Some(path.to_string_lossy().into())
        );
        fs::remove_file(path).unwrap();
        clock.store(5_100, Ordering::SeqCst);
        assert_eq!(fixture.resolve(&resolver, ID).unwrap(), None);
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn stale_missing_and_oversized_files_are_replaced() {
        let fixture = Fixture::new();
        let resolver =
            ArtworkResolver::new(|_, _| Ok(Some(JPEG.to_vec())), || 10, ARTWORK_CACHE_LIMIT);
        for state in 0..3 {
            let path = fixture.seed(ID, "maxresdefault", b"broken", 1);
            if state == 1 {
                fs::remove_file(&path).unwrap();
            }
            if state == 2 {
                fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(ARTWORK_MAX_BYTES + 1)
                    .unwrap();
            }
            assert_eq!(
                fixture.resolve(&resolver, ID).unwrap(),
                Some(path.to_string_lossy().into())
            );
            assert_eq!(fs::read(path).unwrap(), JPEG);
        }
    }

    #[test]
    fn bounded_reader_stops_at_limit_plus_one() {
        let mut reader = std::io::repeat(0xff).take(ARTWORK_MAX_BYTES * 2);
        assert_eq!(read_image(&mut reader).unwrap(), None);
        assert_eq!(reader.limit(), ARTWORK_MAX_BYTES - 1);
        assert_eq!(read_image(JPEG).unwrap(), Some(JPEG.to_vec()));
        assert_eq!(read_image(&b"invalid"[..]).unwrap(), None);
        assert_eq!(read_image(&b""[..]).unwrap(), None);
        let mut exact = vec![0; ARTWORK_MAX_BYTES as usize];
        exact[..JPEG.len()].copy_from_slice(JPEG);
        assert_eq!(
            read_image(exact.as_slice()).unwrap().unwrap().len(),
            ARTWORK_MAX_BYTES as usize
        );
    }

    #[test]
    fn failed_atomic_write_removes_temporary_file() {
        let fixture = Fixture::new();
        let path = fixture.0.join("artwork/destination.jpg");
        fs::create_dir(&path).unwrap();
        assert!(write_image_atomically(&path, JPEG).is_err());
        assert_eq!(fs::read_dir(fixture.0.join("artwork")).unwrap().count(), 1);
    }

    #[test]
    fn atomic_writes_do_not_collide_and_keep_owner_only_permissions() {
        let fixture = Fixture::new();
        let path = fixture.0.join("artwork/destination.jpg");
        thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| scope.spawn(|| write_image_atomically(&path, JPEG)))
                .collect();
            for handle in handles {
                handle.join().unwrap().unwrap();
            }
        });
        assert_eq!(fs::read(&path).unwrap(), JPEG);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(fixture.0.join("artwork")).unwrap().count(), 1);
    }

    #[test]
    fn disk_eviction_uses_last_access_and_removes_the_file() {
        let fixture = Fixture::new();
        let a = fixture.seed(ID, "maxresdefault", JPEG, 1);
        let b = fixture.seed("BaW_jenozKc", "maxresdefault", JPEG, 2);
        let resolver = ArtworkResolver::new(|_, _| Ok(Some(JPEG.to_vec())), || 3, 8);
        fixture.resolve(&resolver, ID).unwrap();
        fixture.resolve(&resolver, "00000000000").unwrap();
        assert!(a.exists());
        assert!(!b.exists());
        assert!(
            crate::persistence::read_artwork_cache(
                &fixture.0.join("library.sqlite3"),
                "youtube:BaW_jenozKc:maxresdefault",
                4
            )
            .unwrap()
            .is_none()
        );
    }

    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        changed: Condvar,
    }
    impl Gate {
        fn wait(&self) {
            let guard = self.open.lock().unwrap();
            let (guard, timeout) = self
                .changed
                .wait_timeout_while(guard, Duration::from_secs(10), |open| !*open)
                .unwrap();
            assert!(*guard && !timeout.timed_out(), "test must release fetches");
        }
        fn release(&self) {
            *self.open.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    #[test]
    fn concurrent_same_id_shares_one_fetch() {
        let fixture = Fixture::new();
        let gate = Arc::new(Gate::default());
        let fetch_gate = gate.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let (entered, receive) = std::sync::mpsc::channel();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                count.fetch_add(1, Ordering::SeqCst);
                entered.send(()).unwrap();
                fetch_gate.wait();
                Ok(Some(JPEG.to_vec()))
            },
            || 10,
            ARTWORK_CACHE_LIMIT,
        );
        let handles: Vec<_> = (0..12)
            .map(|_| {
                let resolver = resolver.clone();
                let directory = fixture.0.clone();
                tauri::async_runtime::spawn(
                    async move { resolver.resolve(directory, ID.into()).await },
                )
            })
            .collect();
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        let duplicate = receive.recv_timeout(Duration::from_millis(100)).is_ok();
        gate.release();
        for handle in handles {
            assert!(
                tauri::async_runtime::block_on(handle)
                    .unwrap()
                    .unwrap()
                    .is_some()
            );
        }
        assert!(!duplicate, "same ID started a second fetch");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_downloads_are_bounded_but_not_serial() {
        let fixture = Fixture::new();
        let gate = Arc::new(Gate::default());
        let fetch_gate = gate.clone();
        let (entered, receive) = std::sync::mpsc::channel();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                entered.send(()).unwrap();
                fetch_gate.wait();
                Ok(Some(JPEG.to_vec()))
            },
            || 10,
            ARTWORK_CACHE_LIMIT,
        );
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let resolver = resolver.clone();
                let directory = fixture.0.clone();
                tauri::async_runtime::spawn(async move {
                    resolver.resolve(directory, format!("{i:011}")).await
                })
            })
            .collect();
        for _ in 0..4 {
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        let exceeded = receive.recv_timeout(Duration::from_millis(100)).is_ok();
        gate.release();
        for handle in handles {
            assert!(
                tauri::async_runtime::block_on(handle)
                    .unwrap()
                    .unwrap()
                    .is_some()
            );
        }
        assert!(!exceeded, "more than four downloads started");
    }

    #[test]
    fn failure_memory_is_bounded_and_fetch_errors_can_recover() {
        let fixture = Fixture::new();
        let clock = Arc::new(AtomicU64::new(0));
        let now = clock.clone();
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fetch_ready = ready.clone();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                if fetch_ready.load(Ordering::SeqCst) {
                    Ok(Some(JPEG.to_vec()))
                } else {
                    Err("offline".into())
                }
            },
            move || now.load(Ordering::SeqCst),
            ARTWORK_CACHE_LIMIT,
        );
        for i in 0..=MAX_FAILURES {
            assert_eq!(
                fixture.resolve(&resolver, &format!("{i:011}")).unwrap(),
                None
            );
        }
        assert_eq!(resolver.failures.lock().unwrap().len(), MAX_FAILURES);
        ready.store(true, Ordering::SeqCst);
        let last = format!("{MAX_FAILURES:011}");
        assert_eq!(fixture.resolve(&resolver, &last).unwrap(), None);
        clock.store(FAILURE_TTL_MS, Ordering::SeqCst);
        assert!(fixture.resolve(&resolver, &last).unwrap().is_some());
        assert!(resolver.failures.lock().unwrap().is_empty());
    }

    #[test]
    fn large_views_wait_for_capacity_without_dropping_artwork() {
        let fixture = Fixture::new();
        let gate = Arc::new(Gate::default());
        let fetch_gate = gate.clone();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                fetch_gate.wait();
                Ok(Some(JPEG.to_vec()))
            },
            || 10,
            ARTWORK_CACHE_LIMIT,
        );
        let handles: Vec<_> = (0..160)
            .map(|i| {
                let resolver = resolver.clone();
                let directory = fixture.0.clone();
                async_runtime::spawn(async move {
                    resolver.resolve(directory, format!("{i:011}")).await
                })
            })
            .collect();
        let deadline = Instant::now() + Duration::from_secs(5);
        while resolver.flights.lock().unwrap().len() < MAX_WORK && Instant::now() < deadline {
            thread::yield_now();
        }
        let pending = resolver.flights.lock().unwrap().len();
        gate.release();
        assert!(
            pending > 0 && pending <= MAX_WORK,
            "only active jobs occupy the flight map"
        );
        for handle in handles {
            assert!(async_runtime::block_on(handle).unwrap().unwrap().is_some());
        }
        assert!(resolver.flights.lock().unwrap().is_empty());
        assert!(fixture.resolve(&resolver, ID).unwrap().is_some());
    }

    #[test]
    fn cancelling_a_waiter_does_not_cancel_shared_work() {
        let fixture = Fixture::new();
        let gate = Arc::new(Gate::default());
        let fetch_gate = gate.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let (entered, receive) = std::sync::mpsc::channel();
        let resolver = ArtworkResolver::new(
            move |_, _| {
                count.fetch_add(1, Ordering::SeqCst);
                entered.send(()).unwrap();
                fetch_gate.wait();
                Ok(Some(JPEG.to_vec()))
            },
            || 10,
            ARTWORK_CACHE_LIMIT,
        );
        let first = resolver.clone();
        let directory = fixture.0.clone();
        let waiter = async_runtime::spawn(async move { first.resolve(directory, ID.into()).await });
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        waiter.abort();
        assert!(async_runtime::block_on(waiter).is_err());
        gate.release();
        assert!(fixture.resolve(&resolver, ID).unwrap().is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[ignore = "requires network access to the public YouTube artwork CDN"]
    fn live_download_is_reused_by_a_fresh_offline_resolver() {
        let fixture = Fixture::new();
        let online = ArtworkResolver::new(fetch_artwork, || 0, ARTWORK_CACHE_LIMIT);
        let path = fixture
            .resolve(&online, ID)
            .unwrap()
            .expect("public artwork is available");
        assert!(is_valid_cached_image(Path::new(&path)));
        let bytes = fs::metadata(&path).unwrap().len();
        let offline = ArtworkResolver::new(
            |_, _| panic!("a persisted cache hit must not contact the network"),
            || 0,
            ARTWORK_CACHE_LIMIT,
        );
        assert_eq!(fixture.resolve(&offline, ID).unwrap(), Some(path));
        println!(
            "Downloaded {bytes} artwork bytes; fresh offline resolver reused the cached file."
        );
    }

    #[test]
    fn http_reader_checks_status_type_length_and_streamed_size() {
        use std::net::TcpListener;
        fn fetch(response: Vec<u8>) -> Result<Option<Vec<u8>>, String> {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/image", listener.local_addr().unwrap());
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = stream.read(&mut request).unwrap();
                assert!(count > 0, "client sent an HTTP request");
                // Oversized responses may be rejected before the body is consumed.
                let _ = stream.write_all(&response);
            });
            let client = Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let result = fetch_url(&client, &url);
            server.join().unwrap();
            result
        }
        let success = [
            b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\n\r\n".as_slice(),
            JPEG,
        ]
        .concat();
        assert_eq!(fetch(success).unwrap(), Some(JPEG.to_vec()));
        assert_eq!(
            fetch(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()).unwrap(),
            None
        );
        assert_eq!(
            fetch(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\n\r\ntext"
                    .to_vec()
            )
            .unwrap(),
            None
        );
        // No body: success here proves the declared oversized length is checked before reading.
        assert_eq!(
            fetch(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    ARTWORK_MAX_BYTES + 1
                )
                .into_bytes()
            )
            .unwrap(),
            None
        );
        let mut streamed =
            b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nConnection: close\r\n\r\n".to_vec();
        streamed.extend_from_slice(JPEG);
        streamed.resize(streamed.len() + ARTWORK_MAX_BYTES as usize, 0);
        assert_eq!(fetch(streamed).unwrap(), None);
        assert!(
            fetch(
                b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\n\r\nx"
                    .to_vec()
            )
            .is_err()
        );
    }
}
