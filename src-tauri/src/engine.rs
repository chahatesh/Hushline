// SPDX-License-Identifier: MIT
//! The dictation actor owns session state. The settings webview is never on the critical path.
use crate::{
    audio,
    groq::Groq,
    model::*,
    native::{self, Target},
    offline::Offline,
    shortcuts::Action,
    store::Store,
    text,
};
use std::{
    sync::{mpsc, Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_util::sync::CancellationToken;

pub struct AppState {
    store: Store,
    settings: RwLock<Settings>,
    view: Mutex<View>,
    tx: UnboundedSender<Message>,
    groq: Groq,
    audio: audio::Engine,
    cancel: Mutex<CancellationToken>,
    offline: Offline,
    offline_cancel: Mutex<CancellationToken>,
}
enum Message {
    Action(Action),
    ReleaseAfter(u64),
    Test,
    Audio(u64, audio::Event),
    Target(u64, bool, Result<Target, String>),
    Done(u64, Box<Result<Completed, String>>),
    Delivered(u64, Box<Completed>, bool, String),
    Applied(u64, Result<String, String>),
    Apply,
    Hide(u64),
    ClipboardWarning(String),
    Polishing(u64),
}
struct Session {
    id: u64,
    capture: audio::Capture,
    settings: Settings,
    target: Option<Result<Target, String>>,
    hands_free: bool,
    command: bool,
    test: bool,
    cancelled: bool,
    ready: bool,
    stopping: bool,
}
struct Completed {
    text: String,
    target: Result<Target, String>,
    preview: bool,
    original: Option<String>,
    duration: u64,
    latency: u64,
    settings: Settings,
    warning: String,
}
struct Pending {
    text: String,
    target: Target,
}

fn publish(app: &tauri::AppHandle, state: &AppState, change: impl FnOnce(&mut View)) {
    if let Ok(mut view) = state.view.lock() {
        change(&mut view);
        native::set_active(view.phase != "idle");
        if app.get_webview_window("main").is_some() {
            let _ = app.emit_to("main", "dictation", view.clone());
        }
        native::update_overlay(&view);
    }
}
fn show(app: &tauri::AppHandle, page: Option<&str>) {
    if app.get_webview_window("main").is_none() {
        if let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main") {
            if let Ok(builder) = tauri::WebviewWindowBuilder::from_config(app, config) {
                let script = format!(
                    "window.__HUSHLINE_PAGE__={};",
                    serde_json::to_string(page.unwrap_or("Home")).unwrap()
                );
                let _ = builder.initialization_script(script).build();
            }
        }
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
        if let Some(page) = page {
            let _ = app.emit_to("main", "navigate", page);
        }
    }
}
fn clipboard_notice(tx: UnboundedSender<Message>) -> Option<Box<dyn FnOnce(String) + Send>> {
    Some(Box::new(move |warning| {
        if !warning.is_empty() {
            let _ = tx.send(Message::ClipboardWarning(warning));
        }
    }))
}
fn schedule_hide(tx: UnboundedSender<Message>, id: u64, millis: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(millis)).await;
        let _ = tx.send(Message::Hide(id));
    });
}
fn target_async(tx: UnboundedSender<Message>, id: u64, selection: bool, epoch: u64) {
    tauri::async_runtime::spawn(async move {
        let task = tauri::async_runtime::spawn_blocking(move || native::capture_target(selection));
        let result=match tokio::time::timeout(Duration::from_millis(1200),task).await {Ok(Ok(Ok(t))) if t.epoch==epoch=>Ok(t),Ok(Ok(Ok(_)))=>Err("Focus changed before capture. Transcript will be kept for recovery.".into()),Ok(Ok(Err(e)))=>Err(e),_=>Err("This application's accessibility inspection timed out. Transcript will be kept for recovery.".into())};
        let _ = tx.send(Message::Target(id, selection, result));
    });
}
fn begin(
    app: &tauri::AppHandle,
    state: &AppState,
    id: u64,
    hands_free: bool,
    command: bool,
    test: bool,
) -> Session {
    let settings = state.settings.read().unwrap().clone();
    let epoch = native::input_epoch();
    publish(app, state, |v| {
        v.phase = "listening".into();
        v.message = "Starting microphone".into();
        v.level = 0.0;
        v.seconds = 0.0;
        v.generation = id;
        v.preview = None;
        v.original = None;
        v.warning.clear();
    });
    let (events, rx) = mpsc::channel();
    let capture = state.audio.start(settings.clone(), events);
    capture.stop_on_silence(hands_free);
    let tx = state.tx.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let finished = matches!(event, audio::Event::Finished(_));
            if tx.send(Message::Audio(id, event)).is_err() || finished {
                break;
            }
        }
    });
    if !test {
        target_async(
            state.tx.clone(),
            id,
            command && settings.selection_permission,
            epoch,
        );
    }
    Session {
        id,
        capture,
        settings,
        target: None,
        hands_free,
        command,
        test,
        cancelled: false,
        ready: false,
        stopping: false,
    }
}

async fn process(
    state: Arc<AppState>,
    session: Session,
    recording: audio::Recording,
    cancel: CancellationToken,
) -> Result<Completed, String> {
    let started = Instant::now();
    let key = if session.settings.provider == "groq" {
        Some(native::read_key()?)
    } else {
        None
    };
    let dictionary: Vec<Entry> = state.store.get("dictionary")?;
    let snippets: Vec<Snippet> = state.store.get("snippets")?;
    let styles: Vec<Style> = state.store.get("styles")?;
    let hints = dictionary
        .iter()
        .map(|e| {
            if e.replacement.is_empty() {
                e.source.as_str()
            } else {
                e.replacement.as_str()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let speech = if let Some(key) = key.as_ref() {
        state
            .groq
            .transcribe(key, recording.wav, &session.settings, &hints, &cancel)
            .await?
    } else {
        state
            .offline
            .transcribe(recording.wav, &session.settings, &hints, &cancel)
            .await?
    };
    let local_mode = text::local_mode(
        &session.settings.mode,
        &session.settings.language,
        speech.language.as_deref(),
    );
    let spoken = speech.text;
    if spoken.trim().is_empty() {
        return Err("No speech was returned. Nothing was inserted.".into());
    }
    let target = session.target.unwrap_or_else(|| {
        Err("The original field could not be inspected in time. Transcript preserved.".into())
    });
    let mut output = text::normalize(&spoken, &local_mode, &dictionary);
    let mut preview = false;
    let mut original = None;
    let mut warning = if recording.limit_reached {
        "Recording reached its time limit. Everything recorded up to that point was kept."
            .to_string()
    } else {
        String::new()
    };
    let _encoding_ms = recording.encoding_ms;
    if session.command {
        if !session.settings.selection_permission {
            return Err(
                "Enable selected-text access in Settings before using Command Mode.".into(),
            );
        }
        let t = target.as_ref().map_err(Clone::clone)?;
        if t.selected.trim().is_empty() {
            return Err("Select text in the target application before using Command Mode. This app may not expose its selection.".into());
        }
        output = rewrite(
            &state,
            &session.settings,
            &t.selected,
            &spoken,
            true,
            &cancel,
        )
        .await?;
        original = Some(t.selected.clone());
        preview = true;
    } else if let Some((expanded, confirm)) = text::expand(&output, &snippets) {
        output = expanded;
        preview = confirm;
    } else {
        let style = if session.settings.context_enabled {
            target.as_ref().ok().and_then(|t| {
                styles.iter().find(|s| {
                    !s.app_match.trim().is_empty()
                        && t.app.to_lowercase().contains(&s.app_match.to_lowercase())
                })
            })
        } else {
            None
        };
        if text::should_polish(
            &output,
            &session.settings.polish_policy,
            &session.settings.mode,
            style.is_some_and(|s| s.enhanced),
        ) {
            let _ = state.tx.send(Message::Polishing(session.id));
            let instruction = format!(
                "{} {}",
                text::POLISH_INSTRUCTION,
                style.map(|s| s.instruction.as_str()).unwrap_or("")
            );
            match rewrite(
                &state,
                &session.settings,
                &output,
                &instruction,
                false,
                &cancel,
            )
            .await
            {
                Ok(value) => match text::validate_cleanup(&output, &value) {
                    Ok(()) => output = value,
                    Err(reason) => {
                        if !warning.is_empty() {
                            warning.push(' ');
                        }
                        warning.push_str(&reason);
                    }
                },
                Err(e) => {
                    if cancel.is_cancelled() {
                        return Err("Cancelled".into());
                    }
                    if !warning.is_empty() {
                        warning.push(' ');
                    }
                    warning.push_str(&format!(
                        "AI cleanup was unavailable; local cleanup was used. {e}"
                    ));
                }
            }
        }
        output = text::format(&output, &session.settings.formatting, &local_mode);
        output = text::sentence_endings(&output, &local_mode);
    }
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    Ok(Completed {
        text: output,
        target,
        preview,
        original,
        duration: recording.duration_ms,
        latency: started.elapsed().as_millis() as u64,
        settings: session.settings,
        warning,
    })
}

async fn rewrite(
    state: &AppState,
    settings: &Settings,
    input: &str,
    instruction: &str,
    command: bool,
    cancel: &CancellationToken,
) -> Result<String, String> {
    if settings.provider == "offline" {
        state
            .offline
            .rewrite(&settings.local_text_model, input, instruction, cancel)
            .await
    } else {
        let key = native::read_key()?;
        state
            .groq
            .rewrite(
                &key,
                if command {
                    &settings.command_model
                } else {
                    &settings.cleanup_model
                },
                input,
                instruction,
                cancel,
            )
            .await
    }
}

fn finish_completed(
    app: &tauri::AppHandle,
    state: &AppState,
    id: u64,
    done: Completed,
    inserted: bool,
    mut warning: String,
) -> Option<Pending> {
    let pending = if done.preview {
        done.target.as_ref().ok().map(|target| Pending {
            text: done.text.clone(),
            target: target.clone(),
        })
    } else {
        None
    };
    if done.settings.history_enabled && state.settings.read().unwrap().history_enabled {
        let record = Transcript {
            id: uuid::Uuid::new_v4().to_string(),
            text: done.text.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            app: done
                .target
                .as_ref()
                .map(|t| t.app.clone())
                .unwrap_or_default(),
            words: done.text.split_whitespace().count() as u32,
            duration_ms: done.duration,
            latency_ms: done.latency,
            inserted,
        };
        if let Err(e) = state.store.insert(&record) {
            warning = format!("Transcript ready; history could not be saved: {e}");
        }
    }
    publish(app, state, |v| {
        v.last_text = done.text.clone();
        v.phase = if done.preview {
            "preview"
        } else if inserted {
            "success"
        } else {
            "error"
        }
        .into();
        v.message = if done.preview {
            "Review your edit"
        } else if inserted {
            "Done"
        } else {
            "Transcript ready to copy"
        }
        .into();
        v.preview = if done.preview { Some(done.text) } else { None };
        v.original = done.original;
        v.warning = warning;
    });
    if done.preview {
        show(app, Some("Home"));
    } else {
        let has_notice = !state.view.lock().unwrap().warning.is_empty();
        schedule_hide(
            state.tx.clone(),
            id,
            if !inserted || has_notice { 12000 } else { 1300 },
        );
    }
    pending
}

async fn actor(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
) {
    let mut session: Option<Session> = None;
    let mut generation = 0u64;
    let mut cancellation = CancellationToken::new();
    let mut pending: Option<Pending> = None;
    let mut last_tap: Option<Instant> = None;
    while let Some(message) = rx.recv().await {
        match message {
            Message::Polishing(id) => {
                if id == generation && !cancellation.is_cancelled() {
                    publish(&app, &state, |v| v.message = "Formatting your words".into());
                }
            }
            Message::Action(Action::Open) => {
                if let Some(s) = session.as_mut() {
                    s.cancelled = true;
                    s.capture.cancel();
                }
                show(&app, None);
            }
            Message::Action(Action::Scratchpad) => {
                if let Some(s) = session.as_mut() {
                    s.cancelled = true;
                    s.capture.cancel();
                }
                show(&app, Some("Scratchpad"));
            }
            Message::Action(Action::Copy) => {
                if let Some(s) = session.as_mut() {
                    s.cancelled = true;
                    s.capture.cancel();
                }
                let text = state.view.lock().unwrap().last_text.clone();
                if !text.is_empty() {
                    let result =
                        tauri::async_runtime::spawn_blocking(move || native::copy_text(&text))
                            .await;
                    if let Ok(Err(e)) = result {
                        publish(&app, &state, |v| v.warning = e);
                    }
                }
            }
            Message::Action(Action::Paste) => {
                if let Some(s) = session.as_mut() {
                    s.cancelled = true;
                    s.capture.cancel();
                }
                let text = state.view.lock().unwrap().last_text.clone();
                let method = state.settings.read().unwrap().insertion.clone();
                let a = app.clone();
                let st = state.clone();
                let token = CancellationToken::new();
                *state.cancel.lock().unwrap() = token.clone();
                if !text.is_empty() {
                    tauri::async_runtime::spawn(async move {
                        let notice = clipboard_notice(st.tx.clone());
                        let result = tauri::async_runtime::spawn_blocking(move || {
                            let target = native::capture_target(false)?;
                            native::insert_with_notice(
                                &target, &text, &method, false, &token, notice,
                            )
                        })
                        .await;
                        publish(&a, &st, |v| {
                            v.warning = match result {
                                Ok(Ok(message)) => message,
                                Ok(Err(e)) => e,
                                _ => "Insertion worker failed; transcript preserved.".into(),
                            };
                        });
                    });
                }
            }
            Message::Action(Action::Cancel) => {
                generation += 1;
                cancellation.cancel();
                if let Some(s) = session.take() {
                    s.capture.cancel();
                }
                pending = None;
                publish(&app, &state, |v| {
                    v.phase = "idle".into();
                    v.preview = None;
                    v.original = None;
                    v.level = 0.0;
                    v.generation = generation;
                });
            }
            Message::Action(Action::Release) => {
                if let Some(s) = session.as_mut() {
                    if !s.hands_free {
                        last_tap = Some(Instant::now());
                        if s.settings.double_tap {
                            let tx = state.tx.clone();
                            let id = s.id;
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(350)).await;
                                let _ = tx.send(Message::ReleaseAfter(id));
                            });
                        } else {
                            s.stopping = true;
                            s.capture.stop();
                        }
                    }
                }
            }
            Message::ReleaseAfter(id) => {
                if let Some(s) = session.as_mut().filter(|s| s.id == id && !s.hands_free) {
                    s.stopping = true;
                    s.capture.stop();
                }
            }
            Message::Action(action @ (Action::Push | Action::HandsFree | Action::Command)) => {
                if session.as_ref().is_some_and(|s| s.stopping) {
                    if let Some(old) = session.take() {
                        old.capture.cancel();
                    }
                }
                if let Some(s) = session.as_mut() {
                    match action {
                        Action::Push
                            if s.settings.double_tap
                                && last_tap
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(350)) =>
                        {
                            s.hands_free = true;
                            s.capture.stop_on_silence(true);
                            publish(&app, &state, |v| {
                                v.message = if s.ready {
                                    "Listening · hands-free"
                                } else {
                                    "Starting microphone"
                                }
                                .into()
                            });
                        }
                        Action::HandsFree if !s.hands_free => {
                            s.hands_free = true;
                            s.capture.stop_on_silence(true);
                            publish(&app, &state, |v| {
                                v.message = if s.ready {
                                    "Listening · hands-free"
                                } else {
                                    "Starting microphone"
                                }
                                .into()
                            });
                        }
                        Action::HandsFree => {
                            s.stopping = true;
                            s.capture.stop();
                        }
                        Action::Command => {
                            s.command = true;
                            let epoch = native::input_epoch();
                            target_async(
                                state.tx.clone(),
                                s.id,
                                s.settings.selection_permission,
                                epoch,
                            );
                            publish(&app, &state, |v| {
                                v.message = if s.ready {
                                    "Listening for an edit"
                                } else {
                                    "Starting microphone"
                                }
                                .into()
                            });
                        }
                        _ => {}
                    }
                    continue;
                }
                cancellation.cancel();
                cancellation = CancellationToken::new();
                *state.cancel.lock().unwrap() = cancellation.clone();
                pending = None;
                generation += 1;
                let settings = state.settings.read().unwrap().clone();
                let command = action == Action::Command;
                if command && !settings.selection_permission {
                    publish(&app, &state, |v| {
                        v.phase = "error".into();
                        v.message =
                            "Enable selected-text access in Settings for Command Mode.".into();
                        v.generation = generation;
                    });
                    schedule_hide(state.tx.clone(), generation, 7000);
                    continue;
                }
                let availability = if settings.provider == "offline" {
                    state.offline.ready(&settings.offline_model)
                } else {
                    native::read_key().map(|_| ())
                };
                if let Err(e) = availability {
                    publish(&app, &state, |v| {
                        v.phase = "error".into();
                        v.message = e;
                        v.generation = generation;
                    });
                    schedule_hide(state.tx.clone(), generation, 7000);
                    continue;
                }
                let double = settings.double_tap
                    && action == Action::Push
                    && last_tap.is_some_and(|t| t.elapsed() < Duration::from_millis(350));
                session = Some(begin(
                    &app,
                    &state,
                    generation,
                    action == Action::HandsFree || double,
                    command,
                    false,
                ));
            }
            Message::Test => {
                if let Some(s) = session.as_ref() {
                    s.capture.stop();
                } else {
                    generation += 1;
                    cancellation.cancel();
                    session = Some(begin(&app, &state, generation, true, false, true));
                }
            }
            Message::Target(id, selection, result) => {
                if let Some(s) = session.as_mut().filter(|s| s.id == id) {
                    if !s.command || selection {
                        s.target = Some(result);
                    }
                }
            }
            Message::Audio(id, audio::Event::Ready) => {
                if id == generation {
                    if let Some(s) = session.as_mut() {
                        s.ready = true;
                        publish(&app, &state, |v| {
                            v.warning.clear();
                            v.message = if s.test {
                                "Testing microphone"
                            } else if s.command {
                                "Listening for an edit"
                            } else if s.hands_free {
                                "Listening · hands-free"
                            } else {
                                "Listening"
                            }
                            .into();
                        });
                    }
                }
            }
            Message::Audio(id, audio::Event::Meter { level, seconds }) => {
                if id == generation {
                    publish(&app, &state, |v| {
                        v.level = level;
                        v.seconds = seconds;
                    });
                }
            }
            Message::Audio(id, audio::Event::Finished(result)) => {
                if id != generation {
                    continue;
                }
                let Some(s) = session.take() else {
                    continue;
                };
                if s.cancelled {
                    publish(&app, &state, |v| v.phase = "idle".into());
                    continue;
                }
                if s.test {
                    publish(&app, &state, |v| {
                        v.phase = if result.is_ok() { "success" } else { "error" }.into();
                        v.message = result
                            .map(|_| "Microphone test complete. No audio was sent.".into())
                            .unwrap_or_else(|e| e);
                    });
                    schedule_hide(state.tx.clone(), id, 5000);
                    continue;
                }
                match result {
                    Err(e) => {
                        publish(&app, &state, |v| {
                            v.phase = if e == "Cancelled" { "idle" } else { "error" }.into();
                            v.message = e;
                            v.level = 0.0;
                        });
                        schedule_hide(state.tx.clone(), id, 7000);
                    }
                    Ok(recording) => {
                        publish(&app, &state, |v| {
                            v.phase = "processing".into();
                            v.message = if s.settings.provider == "offline" {
                                "Transcribing on this computer"
                            } else {
                                "Transcribing with Groq"
                            }
                            .into();
                            v.level = 0.0;
                        });
                        let st = state.clone();
                        let token = cancellation.clone();
                        let tx = state.tx.clone();
                        tauri::async_runtime::spawn(async move {
                            let result = process(st, s, recording, token).await;
                            let _ = tx.send(Message::Done(id, Box::new(result)));
                        });
                    }
                }
            }
            Message::Done(id, result) => {
                if id != generation || cancellation.is_cancelled() {
                    continue;
                }
                match *result {
                    Err(e) => {
                        publish(&app, &state, |v| {
                            v.phase = if e == "Cancelled" { "idle" } else { "error" }.into();
                            v.message = e;
                        });
                        schedule_hide(state.tx.clone(), id, 8000);
                    }
                    Ok(done) => {
                        if done.preview {
                            let warning = done.warning.clone();
                            pending = finish_completed(&app, &state, id, done, false, warning);
                        } else {
                            publish(&app, &state, |v| {
                                v.last_text = done.text.clone();
                                v.message = "Inserting text".into();
                            });
                            let target = done.target.clone();
                            let text = done.text.clone();
                            let method = done.settings.insertion.clone();
                            let token = cancellation.clone();
                            let tx = state.tx.clone();
                            tauri::async_runtime::spawn(async move {
                                let notice = clipboard_notice(tx.clone());
                                let result = tauri::async_runtime::spawn_blocking(move || {
                                    native::insert_with_notice(
                                        &target?, &text, &method, false, &token, notice,
                                    )
                                })
                                .await;
                                let (inserted, warning) = match result {
                                    Ok(Ok(message)) => (
                                        true,
                                        [done.warning.as_str(), message.as_str()]
                                            .into_iter()
                                            .filter(|v| !v.is_empty())
                                            .collect::<Vec<_>>()
                                            .join(" "),
                                    ),
                                    Ok(Err(e)) => (false, e),
                                    _ => (
                                        false,
                                        "Insertion worker failed. Transcript preserved.".into(),
                                    ),
                                };
                                let _ = tx.send(Message::Delivered(
                                    id,
                                    Box::new(done),
                                    inserted,
                                    warning,
                                ));
                            });
                        }
                    }
                }
            }
            Message::ClipboardWarning(warning) => {
                publish(&app, &state, |v| {
                    if !v.warning.is_empty() {
                        v.warning.push(' ');
                    }
                    v.warning.push_str(&warning);
                });
            }
            Message::Delivered(id, done, inserted, warning) => {
                if id == generation && !cancellation.is_cancelled() {
                    pending = finish_completed(&app, &state, id, *done, inserted, warning);
                }
            }
            Message::Apply => {
                if let Some(p) = pending.take() {
                    let method = state.settings.read().unwrap().insertion.clone();
                    let token = cancellation.clone();
                    let tx = state.tx.clone();
                    let id = generation;
                    publish(&app, &state, |v| {
                        v.phase = "processing".into();
                        v.message = "Applying your edit".into();
                        v.preview = None;
                        v.original = None;
                    });
                    tauri::async_runtime::spawn(async move {
                        let notice = clipboard_notice(tx.clone());
                        let result = tauri::async_runtime::spawn_blocking(move || {
                            native::insert_with_notice(
                                &p.target, &p.text, &method, true, &token, notice,
                            )
                        })
                        .await
                        .unwrap_or_else(|_| {
                            Err("Edit worker failed. Transcript preserved.".into())
                        });
                        let _ = tx.send(Message::Applied(id, result));
                    });
                }
            }
            Message::Applied(id, result) => {
                if id != generation || cancellation.is_cancelled() {
                    continue;
                }
                publish(&app, &state, |v| match result {
                    Ok(message) => {
                        v.phase = "success".into();
                        v.message = "Edit applied".into();
                        v.warning = message;
                    }
                    Err(e) => {
                        v.phase = "error".into();
                        v.message = "Edit ready to copy".into();
                        v.warning = e;
                    }
                });
                schedule_hide(state.tx.clone(), id, 5000);
            }
            Message::Hide(id) => {
                if id == generation && session.is_none() && pending.is_none() {
                    publish(&app, &state, |v| v.phase = "idle".into());
                }
            }
        }
    }
}

#[tauri::command]
fn snapshot(state: State<Arc<AppState>>, app: tauri::AppHandle) -> Result<Snapshot, String> {
    let mut view = state.view.lock().map_err(|_| "State unavailable")?.clone();
    view.has_key = native::read_key().is_ok();
    Ok(Snapshot {
        offline: state.offline.status(),
        settings: state
            .settings
            .read()
            .map_err(|_| "Settings unavailable")?
            .clone(),
        dictionary: state.store.get("dictionary")?,
        snippets: state.store.get("snippets")?,
        styles: state.store.get("styles")?,
        history: state.store.history()?,
        scratchpad: state.store.get("scratchpad")?,
        view,
        startup_enabled: app.autolaunch().is_enabled().unwrap_or(false),
    })
}
#[tauri::command]
fn save_settings(state: State<Arc<AppState>>, settings: Settings) -> Result<(), String> {
    settings.validate()?;
    state.store.set("settings", &settings)?;
    native::update_shortcuts(&settings.shortcuts);
    native::configure_overlay(&settings);
    state.audio.configure(settings.clone());
    *state.settings.write().map_err(|_| "Settings unavailable")? = settings;
    Ok(())
}
#[tauri::command]
fn save_dictionary(state: State<Arc<AppState>>, entries: Vec<Entry>) -> Result<(), String> {
    text::validate_entries(&entries, &[], &[])?;
    state.store.set("dictionary", &entries)
}
#[tauri::command]
fn save_snippets(state: State<Arc<AppState>>, entries: Vec<Snippet>) -> Result<(), String> {
    text::validate_entries(&[], &entries, &[])?;
    state.store.set("snippets", &entries)
}
#[tauri::command]
fn save_styles(state: State<Arc<AppState>>, entries: Vec<Style>) -> Result<(), String> {
    text::validate_entries(&[], &[], &entries)?;
    state.store.set("styles", &entries)
}
#[tauri::command]
fn save_scratchpad(state: State<Arc<AppState>>, content: String) -> Result<(), String> {
    if content.len() > 1_000_000 {
        return Err("Scratchpad is limited to one million bytes.".into());
    }
    state.store.set("scratchpad", &content)
}
#[tauri::command]
fn save_api_key(
    state: State<Arc<AppState>>,
    app: tauri::AppHandle,
    key: String,
) -> Result<(), String> {
    native::save_key(key)?;
    publish(&app, &state, |v| v.has_key = true);
    Ok(())
}
#[tauri::command]
fn delete_api_key(state: State<Arc<AppState>>, app: tauri::AppHandle) -> Result<(), String> {
    native::delete_key()?;
    publish(&app, &state, |v| v.has_key = false);
    Ok(())
}
#[tauri::command]
fn microphones() -> Result<Vec<String>, String> {
    audio::devices()
}
#[tauri::command]
async fn available_models(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let key = native::read_key()?;
    state.groq.models(&key, &CancellationToken::new()).await
}
#[tauri::command]
fn delete_history(state: State<Arc<AppState>>, id: Option<String>) -> Result<(), String> {
    state.store.delete(id)
}
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    if text.len() > 1_000_000 {
        return Err("Text is too large.".into());
    }
    native::copy_text(&text)
}
#[tauri::command]
fn recover_text(state: State<Arc<AppState>>, text: String) -> Result<(), String> {
    if text.len() > 100_000 {
        return Err("Text is too large.".into());
    }
    state
        .view
        .lock()
        .map_err(|_| "State unavailable")?
        .last_text = text;
    Ok(())
}
#[tauri::command]
fn action(state: State<Arc<AppState>>, name: String) -> Result<(), String> {
    let restart = matches!(name.as_str(), "toggle" | "test")
        && state.view.lock().map_err(|_| "State unavailable")?.phase == "processing";
    if name == "cancel" || restart {
        state
            .cancel
            .lock()
            .map_err(|_| "Cancellation unavailable")?
            .cancel();
    }
    let m = match name.as_str() {
        "toggle" => Message::Action(Action::HandsFree),
        "cancel" => Message::Action(Action::Cancel),
        "test" => Message::Test,
        "apply" => Message::Apply,
        _ => return Err("Unknown action".into()),
    };
    state
        .tx
        .send(m)
        .map_err(|_| "Dictation engine unavailable".into())
}
#[tauri::command]
fn export_configuration(state: State<Arc<AppState>>) -> Result<Backup, String> {
    Ok(Backup {
        version: 1,
        settings: state
            .settings
            .read()
            .map_err(|_| "Settings unavailable")?
            .clone(),
        dictionary: state.store.get("dictionary")?,
        snippets: state.store.get("snippets")?,
        styles: state.store.get("styles")?,
    })
}
#[tauri::command]
fn import_configuration(state: State<Arc<AppState>>, content: String) -> Result<(), String> {
    if content.len() > 8_000_000 {
        return Err("Backup is too large.".into());
    }
    let mut backup: Backup = serde_json::from_str(&content).map_err(|_| "Invalid backup file.")?;
    backup.settings.migrate();
    state.store.restore(&backup)?;
    native::update_shortcuts(&backup.settings.shortcuts);
    native::configure_overlay(&backup.settings);
    state.audio.configure(backup.settings.clone());
    *state.settings.write().map_err(|_| "Settings unavailable")? = backup.settings;
    Ok(())
}
#[tauri::command]
fn offline_status(state: State<Arc<AppState>>) -> crate::offline::Status {
    state.offline.status()
}
#[tauri::command]
async fn install_offline(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    model: String,
) -> Result<crate::offline::Status, String> {
    let token = CancellationToken::new();
    {
        let mut current = state
            .offline_cancel
            .lock()
            .map_err(|_| "Download state unavailable")?;
        current.cancel();
        *current = token.clone();
    }
    state
        .offline
        .install(&model, &token, |progress| {
            let _ = app.emit_to("main", "offline-download", progress);
        })
        .await
}
#[tauri::command]
fn cancel_offline_download(state: State<Arc<AppState>>) {
    if let Ok(token) = state.offline_cancel.lock() {
        token.cancel();
    }
}
#[tauri::command]
fn set_shortcut_capture(enabled: bool) {
    native::shortcut_capture(enabled);
}
#[tauri::command]
fn set_startup(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|_| "Windows startup preference could not be changed.".into())
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show(app, None)
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                // Keep the existing Windows startup entry across the product rename.
                .app_name("Hushline")
                .args(["--background"])
                .build(),
        );
    let application = builder
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_settings,
            save_dictionary,
            save_snippets,
            save_styles,
            save_scratchpad,
            save_api_key,
            delete_api_key,
            microphones,
            available_models,
            delete_history,
            copy_text,
            recover_text,
            action,
            export_configuration,
            import_configuration,
            set_startup,
            offline_status,
            install_offline,
            cancel_offline_download,
            set_shortcut_capture
        ])
        .setup(|app| {
            let path = app.path().app_data_dir()?;
            std::fs::create_dir_all(&path)?;
            let store =
                Store::open(&path.join("hushline.sqlite")).map_err(std::io::Error::other)?;
            let mut settings: Settings = store.get("settings").map_err(std::io::Error::other)?;
            if settings.migrate() {
                store
                    .set("settings", &settings)
                    .map_err(std::io::Error::other)?;
            }
            settings.validate().map_err(std::io::Error::other)?;
            let (tx, rx) = unbounded_channel();
            let state = Arc::new(AppState {
                store,
                settings: RwLock::new(settings.clone()),
                view: Mutex::new(View {
                    phase: "idle".into(),
                    has_key: native::read_key().is_ok(),
                    ..View::default()
                }),
                tx: tx.clone(),
                groq: Groq::new().map_err(std::io::Error::other)?,
                audio: audio::Engine::new(settings.clone()),
                cancel: Mutex::new(CancellationToken::new()),
                offline: Offline::new(app.path().app_local_data_dir()?.join("offline"))
                    .map_err(std::io::Error::other)?,
                offline_cancel: Mutex::new(CancellationToken::new()),
            });
            let (hook_tx, hook_rx) = mpsc::channel();
            if let Err(e) = native::start_hooks(&settings.shortcuts, hook_tx.clone()) {
                state.view.lock().unwrap().warning = e;
            }
            let cancel_state = state.clone();
            std::thread::spawn(move || {
                while let Ok(action) = hook_rx.recv() {
                    let restart =
                        matches!(action, Action::Push | Action::HandsFree | Action::Command)
                            && cancel_state
                                .view
                                .lock()
                                .is_ok_and(|v| v.phase == "processing");
                    if restart
                        || matches!(
                            action,
                            Action::Cancel
                                | Action::Copy
                                | Action::Paste
                                | Action::Open
                                | Action::Scratchpad
                        )
                    {
                        if let Ok(token) = cancel_state.cancel.lock() {
                            token.cancel();
                        }
                    }
                    if tx.send(Message::Action(action)).is_err() {
                        break;
                    }
                }
            });
            if let Err(e) = native::start_overlay(hook_tx) {
                state.view.lock().unwrap().warning = e;
            }
            native::configure_overlay(&settings);
            native::update_overlay(&state.view.lock().unwrap());
            let open =
                tauri::menu::MenuItem::with_id(app, "open", "Open Hush", true, None::<&str>)?;
            let cancel = tauri::menu::MenuItem::with_id(
                app,
                "cancel",
                "Cancel dictation",
                true,
                None::<&str>,
            )?;
            let quit =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit Hush", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open, &cancel, &quit])?;
            tauri::tray::TrayIconBuilder::new()
                .tooltip("Hush · Ctrl + Win to dictate")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show(app, None),
                    "cancel" => {
                        let s = app.state::<Arc<AppState>>();
                        if let Ok(token) = s.cancel.lock() {
                            token.cancel();
                        }
                        let _ = s.tx.send(Message::Action(Action::Cancel));
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(state.clone());
            tauri::async_runtime::spawn(actor(app.handle().clone(), state, rx));
            if !std::env::args().any(|s| s == "--background") {
                show(app.handle(), None);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    native::shortcut_capture(false);
                    api.prevent_close();
                    let _ = window.destroy();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Hush could not start");
    application.run(|_, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
