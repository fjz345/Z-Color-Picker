use std::{
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    export::{bake_linear, gradient, UnrealLiveSettings, CURVE_KEY_COUNT},
    ui_egui::app::AppDataCtx,
};

const BRIDGE_OBJECT_PATH: &str = "/Script/ZColorPickerBridge.Default__ZColorPickerBridgeLibrary";
const BRIDGE_FUNCTION: &str = "ApplyGradient";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_INTERVAL: Duration = Duration::from_secs(2);
const ANSWER_EXCERPT_CHARS: usize = 200;

fn asset_name(export_name: &str) -> String {
    export_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn asset_path(asset_folder: &str, export_name: &str) -> String {
    let folder = asset_folder.trim().trim_end_matches('/');
    format!("{folder}/{}", asset_name(export_name))
}

fn call_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/remote/object/call")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCall<'a> {
    object_path: &'static str,
    function_name: &'static str,
    parameters: ApplyGradientParameters<'a>,
    generate_transaction: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ApplyGradientParameters<'a> {
    asset_path: &'a str,
    colors: Vec<LinearColor>,
}

#[derive(Serialize)]
#[serde(rename_all = "UPPERCASE")]
struct LinearColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

fn request_body(asset_path: &str, colors: &[[f32; 3]]) -> Result<String, String> {
    if let Some(i) = colors.iter().position(|c| !c.iter().all(|v| v.is_finite())) {
        return Err(format!("The gradient has a non-finite color at sample {i}"));
    }
    let call = RemoteCall {
        object_path: BRIDGE_OBJECT_PATH,
        function_name: BRIDGE_FUNCTION,
        parameters: ApplyGradientParameters {
            asset_path,
            colors: colors
                .iter()
                .map(|&[r, g, b]| LinearColor { r, g, b, a: 1.0 })
                .collect(),
        },
        generate_transaction: false,
    };
    serde_json::to_string(&call).map_err(|e| format!("Building the Unreal request failed: {e}"))
}

#[derive(Clone, Debug, PartialEq)]
enum PushFailure {
    Unreachable(String),
    Rejected(String),
}

impl PushFailure {
    fn into_text(self) -> String {
        match self {
            PushFailure::Unreachable(text) | PushFailure::Rejected(text) => text,
        }
    }
}

fn call_outcome(status: u16, body: &str) -> Result<(), PushFailure> {
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let text_field = |name: &str| json.as_ref()?.get(name)?.as_str();
    let excerpt = || -> String {
        match body.trim() {
            "" => "empty answer".to_string(),
            text => text.chars().take(ANSWER_EXCERPT_CHARS).collect(),
        }
    };
    if !(200..300).contains(&status) {
        return Err(PushFailure::Unreachable(match text_field("errorMessage") {
            Some(message) => format!("{message} (HTTP {status})"),
            None => format!("Unreal answered HTTP {status}: {}", excerpt()),
        }));
    }
    match text_field("ReturnValue") {
        Some("") => Ok(()),
        Some(error) => Err(PushFailure::Rejected(error.to_string())),
        None => Err(PushFailure::Unreachable(format!(
            "Unexpected answer from Unreal: {}",
            excerpt()
        ))),
    }
}

fn transport_error_text(error: &ureq::Error, port: u16) -> String {
    match error {
        ureq::Error::Io(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            not_listening_hint(port)
        }
        ureq::Error::Timeout(ureq::Timeout::Connect) => not_listening_hint(port),
        ureq::Error::Timeout(_) => format!(
            "Unreal did not answer on port {port} within {} s",
            REQUEST_TIMEOUT.as_secs()
        ),
        _ => format!("Push to Unreal failed: {error}"),
    }
}

fn not_listening_hint(port: u16) -> String {
    format!(
        "Nothing listens on port {port}: start the Unreal editor with the ZColorPickerBridge \
         plugin (it enables Remote Control)"
    )
}

#[derive(Debug)]
struct PushJob {
    asset_path: String,
    port: u16,
    body: String,
}

#[derive(Clone, Debug, PartialEq)]
struct PushContent {
    data: AppDataCtx,
    asset_path: String,
    port: u16,
}

impl PushContent {
    fn job(&self) -> Result<PushJob, String> {
        let colors = bake_linear(&gradient(&self.data), CURVE_KEY_COUNT)
            .map_err(|e| format!("Nothing to push: {e}"))?;
        Ok(PushJob {
            body: request_body(&self.asset_path, &colors)?,
            asset_path: self.asset_path.clone(),
            port: self.port,
        })
    }
}

#[derive(Debug, Default)]
struct PushTrigger {
    last_pushed: Option<PushContent>,
}

impl PushTrigger {
    fn changed(
        &mut self,
        settings: &UnrealLiveSettings,
        asset_path: String,
        data: &AppDataCtx,
    ) -> Option<PushContent> {
        if !settings.enabled {
            self.last_pushed = None;
            return None;
        }
        let is_pushed = self.last_pushed.as_ref().is_some_and(|last| {
            last.port == settings.port && last.asset_path == asset_path && last.data == *data
        });
        if is_pushed {
            return None;
        }
        let content = PushContent {
            data: data.clone(),
            asset_path,
            port: settings.port,
        };
        self.last_pushed = Some(content.clone());
        Some(content)
    }
}

#[derive(Debug)]
struct LatestSlot<T> {
    state: Mutex<SlotState<T>>,
    changed: Condvar,
}

#[derive(Debug)]
enum Taken<T> {
    Item(T),
    TimedOut,
    Closed,
}

#[derive(Debug)]
struct SlotState<T> {
    latest: Option<T>,
    is_closed: bool,
}

impl<T> Default for LatestSlot<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(SlotState {
                latest: None,
                is_closed: false,
            }),
            changed: Condvar::new(),
        }
    }
}

impl<T> LatestSlot<T> {
    fn put(&self, item: T) {
        lock(&self.state).latest = Some(item);
        self.changed.notify_one();
    }

    fn take(&self) -> Option<T> {
        match self.take_until(None) {
            Taken::Item(item) => Some(item),
            Taken::TimedOut | Taken::Closed => None,
        }
    }

    fn take_within(&self, timeout: Duration) -> Taken<T> {
        self.take_until(Some(Instant::now() + timeout))
    }

    fn take_until(&self, deadline: Option<Instant>) -> Taken<T> {
        let mut state = lock(&self.state);
        loop {
            if state.is_closed {
                return Taken::Closed;
            }
            if let Some(item) = state.latest.take() {
                return Taken::Item(item);
            }
            state = match deadline {
                None => self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Taken::TimedOut;
                    }
                    self.changed
                        .wait_timeout(state, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    fn close(&self) {
        let mut state = lock(&self.state);
        state.is_closed = true;
        state.latest = None;
        drop(state);
        self.changed.notify_all();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveStatus {
    pub last_success: Option<Instant>,
    pub error: Option<String>,
}

impl LiveStatus {
    fn record(&mut self, outcome: Result<(), String>) -> bool {
        let is_news = match &outcome {
            Ok(()) => self.error.is_some() || self.last_success.is_none(),
            Err(e) => self.error.as_ref() != Some(e),
        };
        match outcome {
            Ok(()) => {
                self.last_success = Some(Instant::now());
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        is_news
    }
}

fn report(status: &Mutex<LiveStatus>, outcome: Result<(), String>, asset_path: &str) -> bool {
    let is_news = lock(status).record(outcome.clone());
    match outcome {
        Ok(()) if is_news => log::info!("Unreal live link: pushed {asset_path}"),
        Err(e) if is_news => log::warn!("Unreal live link: {e}"),
        _ => {}
    }
    is_news
}

fn serve(
    slot: &LatestSlot<PushJob>,
    status: &Mutex<LiveStatus>,
    mut send: impl FnMut(&PushJob) -> Result<(), PushFailure>,
    wake_ui: impl Fn(),
    retry_interval: Duration,
) {
    let mut next = slot.take();
    while let Some(job) = next {
        let outcome = send(&job);
        let is_unreachable = matches!(outcome, Err(PushFailure::Unreachable(_)));
        let is_news = report(
            status,
            outcome.map_err(PushFailure::into_text),
            &job.asset_path,
        );
        if is_news {
            wake_ui();
        }
        next = if is_unreachable {
            match slot.take_within(retry_interval) {
                Taken::Item(newer) => Some(newer),
                Taken::TimedOut => Some(job),
                Taken::Closed => None,
            }
        } else {
            slot.take()
        };
    }
}

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .proxy(None)
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build();
    ureq::Agent::new_with_config(config)
}

fn send(agent: &ureq::Agent, job: &PushJob) -> Result<(), PushFailure> {
    let transport_error =
        |e: ureq::Error| PushFailure::Unreachable(transport_error_text(&e, job.port));
    let mut response = agent
        .put(call_url(job.port).as_str())
        .content_type("application/json")
        .send(job.body.as_str())
        .map_err(transport_error)?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(transport_error)?;
    call_outcome(status, &body)
}

#[derive(Debug)]
struct Worker {
    slot: Arc<LatestSlot<PushJob>>,
}

impl Worker {
    fn spawn(
        status: Arc<Mutex<LiveStatus>>,
        wake_ui: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let slot = Arc::new(LatestSlot::default());
        let worker_slot = slot.clone();
        thread::Builder::new()
            .name("unreal-live".to_string())
            .spawn(move || {
                let agent = agent();
                serve(
                    &worker_slot,
                    &status,
                    |job| send(&agent, job),
                    wake_ui,
                    RETRY_INTERVAL,
                );
            })?;
        Ok(Self { slot })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.slot.close();
    }
}

#[derive(Debug, Default)]
pub struct UnrealLiveLink {
    trigger: PushTrigger,
    status: Arc<Mutex<LiveStatus>>,
    worker: Option<Worker>,
}

impl UnrealLiveLink {
    pub fn status(&self) -> LiveStatus {
        lock(&self.status).clone()
    }

    pub fn stop(&mut self) {
        self.worker = None;
        self.trigger = PushTrigger::default();
    }

    pub fn sync(
        &mut self,
        settings: &UnrealLiveSettings,
        asset_path: String,
        data: &AppDataCtx,
        wake_ui: impl Fn() + Send + 'static,
    ) {
        if !settings.enabled {
            self.stop();
            return;
        }
        let Some(content) = self.trigger.changed(settings, asset_path, data) else {
            return;
        };
        if self.worker.is_none() {
            self.status = Arc::default();
            match Worker::spawn(self.status.clone(), wake_ui) {
                Ok(worker) => self.worker = Some(worker),
                Err(e) => {
                    let message = format!("Starting the Unreal live link failed: {e}");
                    report(&self.status, Err(message), &content.asset_path);
                }
            }
        }
        let Some(worker) = &self.worker else {
            return;
        };
        match content.job() {
            Ok(job) => worker.slot.put(job),
            Err(e) => {
                report(&self.status, Err(e), &content.asset_path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color_space::{BlendSpace, PickerSpace},
        common::SplineMode,
        datatypes::control_point::{ControlPoint, ControlPointValue},
    };
    use std::sync::mpsc;

    const WAIT_FOR_THE_OTHER_THREAD: Duration = Duration::from_millis(20);
    const NO_RETRY: Duration = Duration::from_secs(3600);

    fn red_to_blue() -> AppDataCtx {
        AppDataCtx {
            spline_mode: SplineMode::Linear,
            control_points: vec![
                ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.0), 0.0),
                ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 2.0 / 3.0), 1.0),
            ],
            picker_space: PickerSpace::Hsv,
            blend_space: BlendSpace::Picker,
        }
    }

    fn live_on() -> UnrealLiveSettings {
        UnrealLiveSettings {
            enabled: true,
            ..Default::default()
        }
    }

    fn job_to(asset_path: &str, port: u16) -> PushJob {
        PushJob {
            asset_path: asset_path.to_string(),
            port,
            body: String::new(),
        }
    }

    #[test]
    fn request_body_is_the_remote_control_call_byte_for_byte() {
        let body = request_body(
            "/Game/ZColorPicker/Warm",
            &[[0.1, 0.2, 0.3], [1.0, 0.5, 0.0]],
        )
        .unwrap();

        assert_eq!(
            body,
            concat!(
                r#"{"objectPath":"/Script/ZColorPickerBridge.Default__ZColorPickerBridgeLibrary","#,
                r#""functionName":"ApplyGradient","#,
                r#""parameters":{"AssetPath":"/Game/ZColorPicker/Warm","#,
                r#""Colors":[{"R":0.1,"G":0.2,"B":0.3,"A":1.0},{"R":1.0,"G":0.5,"B":0.0,"A":1.0}]},"#,
                r#""generateTransaction":false}"#
            )
        );
    }

    #[test]
    fn request_body_escapes_the_asset_path_and_refuses_non_finite_colors() {
        let body = request_body(r#"/Game/a"b"#, &[[0.0; 3]]).unwrap();
        let error =
            request_body("/Game/ZColorPicker/Warm", &[[0.0; 3], [f32::NAN, 0.0, 0.0]]).unwrap_err();

        assert!(body.contains(r#""AssetPath":"/Game/a\"b""#), "{body}");
        assert!(error.contains("sample 1"), "{error}");
    }

    #[test]
    fn pushed_job_holds_the_64_baked_linear_colors_for_the_port() {
        let data = red_to_blue();
        let colors = bake_linear(&gradient(&data), 64).unwrap();
        let mut trigger = PushTrigger::default();

        let content = trigger
            .changed(&live_on(), "/Game/ZColorPicker/Warm".to_string(), &data)
            .unwrap();
        let job = content.job().unwrap();

        assert_eq!(
            job.body,
            request_body("/Game/ZColorPicker/Warm", &colors).unwrap()
        );
        assert_eq!(job.body.matches(r#""A":1.0}"#).count(), 64);
        assert_eq!(
            call_url(job.port),
            "http://127.0.0.1:30010/remote/object/call"
        );
    }

    #[test]
    fn a_gradient_without_points_is_an_error_not_a_job() {
        let data = AppDataCtx {
            control_points: Vec::new(),
            ..red_to_blue()
        };
        let content = PushContent {
            data,
            asset_path: "/Game/ZColorPicker/Empty".to_string(),
            port: 30010,
        };

        assert!(content.job().is_err());
    }

    #[test]
    fn asset_name_replaces_every_char_outside_ascii_letters_digits_and_underscore() {
        assert_eq!(asset_name("Warm_2"), "Warm_2");
        assert_eq!(asset_name("Sky blue-2.0 (v1)"), "Sky_blue_2_0__v1_");
        assert_eq!(asset_name("Grün"), "Gr_n");
        assert_eq!(asset_name("火山"), "__");
    }

    #[test]
    fn asset_path_joins_the_trimmed_folder_and_the_asset_name() {
        assert_eq!(
            asset_path("/Game/ZColorPicker", "Warm"),
            "/Game/ZColorPicker/Warm"
        );
        assert_eq!(
            asset_path(" /Game/Curves/ ", "Sky blue"),
            "/Game/Curves/Sky_blue"
        );
    }

    #[test]
    fn call_outcome_maps_the_return_value_and_remote_control_errors() {
        assert_eq!(call_outcome(200, r#"{"ReturnValue":""}"#), Ok(()));
        assert_eq!(
            call_outcome(200, r#"{"ReturnValue":"Colors is empty"}"#),
            Err(PushFailure::Rejected("Colors is empty".to_string()))
        );
        let unreachable_text = |status, body| match call_outcome(status, body) {
            Err(PushFailure::Unreachable(text)) => text,
            other => panic!("{other:?}"),
        };
        let not_found = unreachable_text(404, r#"{"errorMessage":"Object not found"}"#);
        assert!(
            not_found.contains("Object not found") && not_found.contains("404"),
            "{not_found}"
        );
        let garbage = unreachable_text(200, "<html>");
        assert!(garbage.contains("<html>"), "{garbage}");
        let bare = unreachable_text(500, "");
        assert!(bare.contains("500"), "{bare}");
    }

    #[test]
    fn a_refused_connection_hints_to_start_the_editor_with_the_plugin() {
        let refused = ureq::Error::Io(io::ErrorKind::ConnectionRefused.into());

        let hint = transport_error_text(&refused, 30010);

        assert!(
            hint.contains("30010") && hint.contains("ZColorPickerBridge"),
            "{hint}"
        );
        let connect_timeout = ureq::Error::Timeout(ureq::Timeout::Connect);
        assert_eq!(transport_error_text(&connect_timeout, 30010), hint);
        let no_answer = ureq::Error::Timeout(ureq::Timeout::Global);
        assert_ne!(transport_error_text(&no_answer, 30010), hint);
    }

    #[test]
    fn slot_hands_out_only_the_newest_item() {
        let slot = LatestSlot::default();

        slot.put(1);
        slot.put(2);
        slot.put(3);

        assert_eq!(slot.take(), Some(3));
        slot.put(4);
        assert_eq!(slot.take(), Some(4));
    }

    #[test]
    fn slot_take_waits_for_a_put_from_another_thread() {
        let slot = Arc::new(LatestSlot::default());
        let taker = {
            let slot = slot.clone();
            thread::spawn(move || slot.take())
        };

        thread::sleep(WAIT_FOR_THE_OTHER_THREAD);
        slot.put(7);

        assert_eq!(taker.join().unwrap(), Some(7));
    }

    #[test]
    fn closed_slot_drops_its_item_and_ends_a_waiting_take() {
        let slot = Arc::new(LatestSlot::<i32>::default());
        let taker = {
            let slot = slot.clone();
            thread::spawn(move || slot.take())
        };

        thread::sleep(WAIT_FOR_THE_OTHER_THREAD);
        slot.close();

        assert_eq!(taker.join().unwrap(), None);
        slot.put(1);
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn trigger_pushes_on_a_change_only_and_once_when_live_turns_on() {
        let data = red_to_blue();
        let warm = || "/Game/ZColorPicker/Warm".to_string();
        let cold = || "/Game/ZColorPicker/Cold".to_string();
        let off = UnrealLiveSettings::default();
        let other_port = UnrealLiveSettings {
            port: 30011,
            ..live_on()
        };
        let bent = AppDataCtx {
            spline_mode: SplineMode::HermiteBezier,
            ..data.clone()
        };
        let mut trigger = PushTrigger::default();

        assert!(trigger.changed(&off, warm(), &data).is_none());
        assert!(trigger.changed(&live_on(), warm(), &data).is_some());
        assert!(trigger.changed(&live_on(), warm(), &data).is_none());
        assert!(trigger.changed(&live_on(), warm(), &bent).is_some());
        assert!(trigger.changed(&live_on(), cold(), &bent).is_some());
        assert!(trigger.changed(&other_port, cold(), &bent).is_some());
        assert!(trigger.changed(&other_port, cold(), &bent).is_none());
        assert!(trigger.changed(&off, cold(), &bent).is_none());
        assert!(trigger.changed(&other_port, cold(), &bent).is_some());
    }

    #[test]
    fn slot_take_within_times_out_hands_out_an_item_or_ends_when_closed() {
        let slot = LatestSlot::default();

        assert!(matches!(
            slot.take_within(Duration::from_millis(1)),
            Taken::TimedOut
        ));
        slot.put(5);
        assert!(matches!(slot.take_within(NO_RETRY), Taken::Item(5)));
        slot.close();
        assert!(matches!(slot.take_within(NO_RETRY), Taken::Closed));
    }

    #[test]
    fn worker_retries_a_failed_push_until_it_succeeds() {
        let slot = Arc::new(LatestSlot::default());
        let status = Arc::new(Mutex::new(LiveStatus::default()));
        let (woken_tx, woken) = mpsc::channel();
        let worker = {
            let (slot, status) = (slot.clone(), status.clone());
            thread::spawn(move || {
                let mut attempts = 0;
                let send = |_: &PushJob| {
                    attempts += 1;
                    match attempts {
                        1 | 2 => Err(PushFailure::Unreachable("refused".to_string())),
                        _ => Ok(()),
                    }
                };
                let wake = || woken_tx.send(lock(&status).clone()).unwrap();
                serve(&slot, &status, send, wake, Duration::from_millis(1));
                attempts
            })
        };

        slot.put(job_to("/Game/A", 30010));
        let outcomes: Vec<LiveStatus> = (0..2).map(|_| woken.recv().unwrap()).collect();
        slot.close();

        assert_eq!(worker.join().unwrap(), 3);
        assert!(woken.try_recv().is_err(), "woken for an unchanged error");
        assert_eq!(outcomes[0].error.as_deref(), Some("refused"));
        assert_eq!(outcomes[1].error, None);
        assert!(outcomes[1].last_success.is_some());
    }

    #[test]
    fn worker_waits_for_new_content_after_the_plugin_rejects_a_push() {
        let slot = Arc::new(LatestSlot::default());
        let status = Arc::new(Mutex::new(LiveStatus::default()));
        let (sent_tx, sent) = mpsc::channel();
        let worker = {
            let (slot, status) = (slot.clone(), status.clone());
            thread::spawn(move || {
                let send = |job: &PushJob| {
                    sent_tx.send(job.asset_path.clone()).unwrap();
                    match job.asset_path.as_str() {
                        "/Engine/A" => Err(PushFailure::Rejected("not under /Game/".to_string())),
                        _ => Ok(()),
                    }
                };
                serve(&slot, &status, send, || {}, Duration::from_millis(1));
            })
        };

        slot.put(job_to("/Engine/A", 30010));
        assert_eq!(sent.recv().unwrap(), "/Engine/A");
        let resent = sent.recv_timeout(Duration::from_millis(100));
        let rejected = lock(&status).clone();
        slot.put(job_to("/Game/B", 30010));
        assert_eq!(sent.recv().unwrap(), "/Game/B");
        slot.close();
        worker.join().unwrap();

        assert!(resent.is_err(), "resent {resent:?}");
        assert_eq!(rejected.error.as_deref(), Some("not under /Game/"));
    }

    #[test]
    fn stopped_link_pushes_the_same_content_again() {
        let data = red_to_blue();
        let warm = || "/Game/ZColorPicker/Warm".to_string();
        let mut link = UnrealLiveLink::default();
        assert!(link.trigger.changed(&live_on(), warm(), &data).is_some());

        link.stop();

        assert!(link.worker.is_none());
        assert!(link.trigger.changed(&live_on(), warm(), &data).is_some());
    }

    #[test]
    fn worker_records_each_outcome_wakes_the_ui_and_stops_when_closed() {
        let slot = Arc::new(LatestSlot::default());
        let status = Arc::new(Mutex::new(LiveStatus::default()));
        let (sent_tx, sent) = mpsc::channel();
        let (woken_tx, woken) = mpsc::channel();
        let worker = {
            let (slot, status) = (slot.clone(), status.clone());
            thread::spawn(move || {
                let send = |job: &PushJob| {
                    sent_tx.send(job.asset_path.clone()).unwrap();
                    match job.port {
                        1 => Err(PushFailure::Unreachable("refused".to_string())),
                        _ => Ok(()),
                    }
                };
                serve(
                    &slot,
                    &status,
                    send,
                    || woken_tx.send(()).unwrap(),
                    NO_RETRY,
                );
            })
        };

        slot.put(job_to("/Game/A", 1));
        woken.recv().unwrap();
        let failed = lock(&status).clone();
        slot.put(job_to("/Game/B", 30010));
        woken.recv().unwrap();
        let pushed = lock(&status).clone();
        slot.close();
        worker.join().unwrap();

        assert_eq!(failed.error.as_deref(), Some("refused"));
        assert!(failed.last_success.is_none());
        assert_eq!(pushed.error, None);
        assert!(pushed.last_success.is_some());
        assert_eq!(sent.try_iter().collect::<Vec<_>>(), ["/Game/A", "/Game/B"]);
    }
}
