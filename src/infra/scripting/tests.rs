use crate::core::action::Action;
use crate::core::plugin_api::{ArtistRef, PlaybackState, TrackInfo};

use super::engine::ScriptEngine;
use super::events::{diff_events, ScriptEvent};
use super::shared::HttpResponseData;

fn track(uri: &str, name: &str) -> TrackInfo {
  TrackInfo {
    generated_name: None,
    uri: Some(uri.to_string()),
    name: name.to_string(),
    artists: vec!["Artist".to_string()],
    album: "Album".to_string(),
    duration_ms: 200_000,
    id: None,
    album_id: None,
    artist_refs: vec![ArtistRef {
      id: None,
      name: "Artist".to_string(),
    }],
    is_playable: true,
    is_local: false,
    track_number: 0,
    explicit: false,
    image_url: None,
    release_date: None,
  }
}

fn playback(track: Option<TrackInfo>, is_playing: bool, progress_ms: u64) -> PlaybackState {
  PlaybackState {
    track,
    is_playing,
    progress_ms,
    shuffle: false,
    repeat: "off".to_string(),
    volume_percent: Some(50),
    device: None,
  }
}

/// Take all currently-queued actions out of the shared buffer.
/// (Assertions pattern-match, the style this suite has always used.)
fn drain(engine: &ScriptEngine) -> Vec<Action> {
  engine.shared.effects.borrow_mut().drain(..).collect()
}

/// Assert a single action was queued and return it.
fn one(engine: &ScriptEngine) -> Action {
  let mut effects = drain(engine);
  assert_eq!(effects.len(), 1, "expected exactly one effect");
  effects.pop().unwrap()
}

// --- handler registration + emission ---

#[test]
fn track_change_handler_queues_notify() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source(
      "test",
      r#"
        qbal.on("track_change", function(pb)
          qbal.notify("now: " .. pb.track.name, 5)
        end)
      "#,
    )
    .unwrap();

  *engine.shared.playback.borrow_mut() = Some(playback(Some(track("uri:1", "Song A")), true, 0));
  engine.emit(ScriptEvent::TrackChange);

  match one(&engine) {
    Action::Notify(msg, ttl) => {
      assert_eq!(msg, "now: Song A");
      assert_eq!(ttl, 5);
    }
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn empty_playback_fields_reach_lua_as_nil() {
  let mut engine = ScriptEngine::new().unwrap();
  *engine.shared.playback.borrow_mut() = Some(playback(Some(track("uri:1", "Song A")), true, 0));
  engine
    .load_source(
      "test",
      r#"
        local pb = qbal.playback()
        local t = qbal.current_track()
        qbal.notify(tostring(pb.device == nil) .. ":" .. tostring(pb.track.release_date == nil) .. ":" .. tostring(t.id == nil), 1)
      "#,
    )
    .unwrap();

  match one(&engine) {
    Action::Notify(msg, 1) => assert_eq!(msg, "true:true:true"),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn empty_fields_in_an_event_playback_reach_lua_as_nil() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source(
      "test",
      r#"
        qbal.on("track_change", function(pb)
          qbal.notify(tostring(pb.device == nil), 1)
        end)
      "#,
    )
    .unwrap();

  *engine.shared.playback.borrow_mut() = Some(playback(Some(track("uri:1", "Song A")), true, 0));
  engine.emit(ScriptEvent::TrackChange);

  match one(&engine) {
    Action::Notify(msg, 1) => assert_eq!(msg, "true"),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn erroring_handler_is_disabled_after_one_strike() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source(
      "bad",
      r#"
        qbal.on("start", function() error("boom") end)
        qbal.on("start", function() qbal.notify("healthy", 1) end)
      "#,
    )
    .unwrap();

  engine.emit(ScriptEvent::Start);
  let first = drain(&engine);
  // One error notify (from the bad handler) plus the healthy notify.
  assert_eq!(first.len(), 2);
  match &first[0] {
    Action::NotifyError(m, 6) => assert!(m.contains("error in on_start")),
    _ => panic!("expected error notify first"),
  }
  match &first[1] {
    Action::Notify(m, 1) => assert_eq!(m, "healthy"),
    _ => panic!("expected healthy notify second"),
  }

  // Second emit: bad handler removed, only the healthy one fires (no new error).
  engine.emit(ScriptEvent::Start);
  match one(&engine) {
    Action::Notify(m, 1) => assert_eq!(m, "healthy"),
    _ => panic!("expected only the healthy notify"),
  }
}

#[test]
fn unknown_event_name_is_an_error() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source("test", r#"qbal.on("bogus_event", function() end)"#);
  assert!(result.is_err());
}

// --- require_api ---

#[test]
fn require_api_at_or_below_current_succeeds() {
  use crate::core::plugin_api::API_VERSION;
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("test", &format!("qbal.require_api({API_VERSION})"))
    .unwrap();
  engine.load_source("test2", "qbal.require_api(1)").unwrap();
}

#[test]
fn require_api_above_current_fails_with_clear_message() {
  use crate::core::plugin_api::API_VERSION;
  let mut engine = ScriptEngine::new().unwrap();
  let too_high = API_VERSION + 1;
  let err = engine
    .load_source("test", &format!("qbal.require_api({too_high})"))
    .unwrap_err()
    .to_string();
  assert!(err.contains(&too_high.to_string()), "message: {err}");
  assert!(err.contains(&API_VERSION.to_string()), "message: {err}");

  // The engine is not poisoned: a later, compatible plugin still loads.
  engine.load_source("ok", "qbal.require_api(1)").unwrap();
}

#[test]
fn require_api_rejects_non_positive_version() {
  let mut engine = ScriptEngine::new().unwrap();
  assert!(engine.load_source("test", "qbal.require_api(0)").is_err());
}

// --- action functions queue the right effect ---

fn run_action(src: &str) -> Action {
  let mut engine = ScriptEngine::new().unwrap();
  engine.load_source("test", src).unwrap();
  one(&engine)
}

#[test]
fn action_play_queues_play() {
  assert!(matches!(run_action("qbal.play()"), Action::Play));
}

#[test]
fn action_pause_queues_pause() {
  assert!(matches!(run_action("qbal.pause()"), Action::Pause));
}

#[test]
fn action_next_queues_next() {
  assert!(matches!(run_action("qbal.next()"), Action::NextTrack));
}

#[test]
fn action_previous_queues_previous() {
  assert!(matches!(
    run_action("qbal.previous()"),
    Action::PreviousTrack
  ));
}

#[test]
fn action_seek_queues_seek() {
  match run_action("qbal.seek(12345)") {
    Action::SeekTo(ms) => assert_eq!(ms, 12345),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn action_set_volume_clamps_above_100() {
  match run_action("qbal.set_volume(250)") {
    Action::SetVolume(v) => assert_eq!(v, 100),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
  match run_action("qbal.set_volume(-10)") {
    Action::SetVolume(v) => assert_eq!(v, 0),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn action_shuffle_queues_set_shuffle() {
  match run_action("qbal.shuffle(true)") {
    Action::SetShuffle(on) => assert!(on),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
  match run_action("qbal.shuffle(false)") {
    Action::SetShuffle(on) => assert!(!on),
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn action_notify_default_ttl_is_4() {
  match run_action(r#"qbal.notify("hi")"#) {
    Action::Notify(m, ttl) => {
      assert_eq!(m, "hi");
      assert_eq!(ttl, 4);
    }
    _ => panic!("expected a Notify effect"),
  }
}

mod http_tests {
  use super::*;

  /// Run `test` inside a tokio runtime, passing a URL backed by a local listener that
  /// accepts connections but never responds. The real request spawned by `http_get` /
  /// `http_post` hangs until the client timeout, so it can never race the injected
  /// synthetic result, and no traffic leaves the machine.
  fn with_runtime_engine(test: impl FnOnce(&mut ScriptEngine, &str)) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let mut engine = ScriptEngine::new().unwrap();
    test(&mut engine, &url);
  }

  fn response(status: u16, body: &str) -> HttpResponseData {
    HttpResponseData {
      status,
      body: body.to_string(),
    }
  }

  #[test]
  fn json_round_trip() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "json",
        r#"
          local decoded = qbal.json_decode('{"name":"Song","nested":{"ok":true},"items":[1,2]}')
          local encoded = qbal.json_encode(decoded)
          local again = qbal.json_decode(encoded)
          qbal.notify(again.name .. ":" .. tostring(again.nested.ok) .. ":" .. tostring(again.items[2]), 1)
        "#,
      )
      .unwrap();

    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "Song:true:2"),
      _ => panic!("expected json round-trip notify"),
    }
  }

  #[test]
  fn json_decode_invalid_input_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "json",
        r#"
          local ok = pcall(function()
            qbal.json_decode("{")
          end)
          qbal.notify(tostring(ok), 1)
        "#,
      )
      .unwrap();

    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "false"),
      _ => panic!("expected pcall failure notify"),
    }
  }

  #[test]
  fn json_encode_non_serializable_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "json",
        r#"
          local ok = pcall(function()
            qbal.json_encode(function() end)
          end)
          qbal.notify(tostring(ok), 1)
        "#,
      )
      .unwrap();

    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "false"),
      _ => panic!("expected pcall failure notify"),
    }
  }

  #[test]
  fn json_null_decodes_to_sentinel_not_nil() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "json",
        r#"
          local NULL = qbal.json_decode("null")
          local decoded = qbal.json_decode('{"x":null}')
          qbal.notify(tostring(decoded.x == nil) .. ":" .. tostring(decoded.x == NULL), 1)
        "#,
      )
      .unwrap();

    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "false:true"),
      _ => panic!("expected null sentinel notify"),
    }
  }

  #[test]
  fn http_get_callback_fires_on_synthetic_success() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              if err then
                qbal.notify(err, 1)
              else
                qbal.notify(resp.body, 1)
              end
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(200, "hello")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "hello"),
        _ => panic!("expected http success notify"),
      }
    });
  }

  #[tokio::test(flavor = "current_thread")]
  async fn http_get_spawn_path_delivers_response() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
      let (mut socket, _) = listener.accept().await.unwrap();
      let mut buf = [0_u8; 1024];
      let _ = socket.read(&mut buf).await.unwrap();
      let body = "from server";
      let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
      );
      socket.write_all(response.as_bytes()).await.unwrap();
    });

    let mut engine = ScriptEngine::new().unwrap();
    let source = format!(
      r#"
        qbal.http_get("http://{addr}/lyrics", function(resp, err)
          if err then
            qbal.notify(err, 1)
          else
            qbal.notify(tostring(resp.status) .. ":" .. resp.body, 1)
          end
        end)
      "#
    );
    engine.load_source("fetcher", &source).unwrap();

    for _ in 0..100 {
      engine.drain_http_callbacks_for_test();
      if !engine.shared.effects.borrow().is_empty() {
        break;
      }
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "200:from server"),
      _ => panic!("expected spawned http response notify"),
    }
    server.await.unwrap();
  }

  #[test]
  fn http_post_callback_fires_on_synthetic_success() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_post("{url}", "body", nil, function(resp, err)
              if err then
                qbal.notify(err, 1)
              else
                qbal.notify(tostring(resp.status) .. ":" .. resp.body, 1)
              end
            end)
          "#
      );
      engine.load_source("poster", &source).unwrap();

      engine.inject_http_result(1, Ok(response(201, "created")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "201:created"),
        _ => panic!("expected http post success notify"),
      }
    });
  }

  #[test]
  fn http_get_callback_fires_on_synthetic_error() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              if err then
                qbal.notify(err, 1)
              else
                qbal.notify(resp.body, 1)
              end
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Err("dns failed".to_string()));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "dns failed"),
        _ => panic!("expected http error notify"),
      }
    });
  }

  #[test]
  fn http_callback_is_one_shot() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              qbal.notify(resp.body, 1)
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(200, "first")));
      engine.drain_http_callbacks_for_test();
      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "first"),
        _ => panic!("expected first callback notify"),
      }

      engine.inject_http_result(1, Ok(response(200, "second")));
      engine.drain_http_callbacks_for_test();
      assert!(drain(engine).is_empty());
    });
  }

  #[test]
  fn http_callbacks_keep_token_identity_after_earlier_delivery() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}a", function(resp, err)
              qbal.notify("a:" .. resp.body, 1)
            end)
            qbal.http_get("{url}b", function(resp, err)
              qbal.notify("b:" .. resp.body, 1)
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(200, "one")));
      engine.drain_http_callbacks_for_test();
      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "a:one"),
        _ => panic!("expected first callback notify"),
      }

      engine.inject_http_result(2, Ok(response(200, "two")));
      engine.drain_http_callbacks_for_test();
      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "b:two"),
        _ => panic!("expected second callback notify"),
      }
    });
  }

  #[test]
  fn http_callback_attribution() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              qbal.set_playbar(resp.body)
            end)
          "#
      );
      engine.load_source("lyrics_plugin", &source).unwrap();

      engine.inject_http_result(1, Ok(response(200, "lyrics ready")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::SetPlaybarSegment { plugin, text } => {
          assert_eq!(plugin, "lyrics_plugin");
          assert_eq!(text.as_deref(), Some("lyrics ready"));
        }
        _ => panic!("expected attributed playbar segment"),
      }
    });
  }

  #[test]
  fn http_callback_error_queues_notify_error_without_breaking_engine() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              error("callback boom")
            end)
          "#
      );
      engine.load_source("bad_fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(200, "ignored")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::NotifyError(msg, 6) => {
          assert!(msg.contains("bad_fetcher"));
          assert!(msg.contains("error in http callback"));
          assert!(msg.contains("callback boom"));
        }
        _ => panic!("expected http callback error notify"),
      }
      assert!(engine.shared.current_plugin.borrow().is_empty());

      engine
        .load_source("healthy", r#"qbal.notify("still alive", 1)"#)
        .unwrap();
      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "still alive"),
        _ => panic!("expected engine to keep running"),
      }
    });
  }

  #[test]
  fn http_get_invalid_scheme_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    let result = engine.load_source(
      "fetcher",
      r#"qbal.http_get("ftp://example.com", function() end)"#,
    );
    assert!(result.is_err());
    assert!(result
      .unwrap_err()
      .to_string()
      .contains("unsupported URL scheme"));
  }

  #[test]
  fn http_get_no_runtime_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    let result = engine.load_source(
      "fetcher",
      r#"qbal.http_get("https://example.com", function() end)"#,
    );
    assert!(result.is_err());
    assert!(result
      .unwrap_err()
      .to_string()
      .contains("no tokio runtime available"));
  }

  #[test]
  fn http_resp_ok_true_for_2xx() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              qbal.notify(tostring(resp.ok), 1)
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(204, "")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "true"),
        _ => panic!("expected ok=true notify"),
      }
    });
  }

  #[test]
  fn http_resp_ok_false_for_4xx() {
    with_runtime_engine(|engine, url| {
      let source = format!(
        r#"
            qbal.http_get("{url}", function(resp, err)
              qbal.notify(tostring(resp.ok) .. ":" .. tostring(err == nil), 1)
            end)
          "#
      );
      engine.load_source("fetcher", &source).unwrap();

      engine.inject_http_result(1, Ok(response(404, "not found")));
      engine.drain_http_callbacks_for_test();

      match one(engine) {
        Action::Notify(msg, 1) => assert_eq!(msg, "false:true"),
        _ => panic!("expected ok=false notify"),
      }
    });
  }
}

// --- drain_effects: routes through App methods ---

#[cfg(test)]
mod drain_tests {}

// --- register_command ---

#[test]
fn register_command_happy_path() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source(
      "myplugin",
      r#"qbal.register_command("hello", function() qbal.notify("hi", 1) end)"#,
    )
    .unwrap();
  assert!(drain(&engine).is_empty());
}

#[test]
fn register_command_empty_name_is_error() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source("test", r#"qbal.register_command("", function() end)"#);
  assert!(result.is_err());
}

#[test]
fn register_command_whitespace_name_is_error() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source(
    "test",
    r#"qbal.register_command("bad name", function() end)"#,
  );
  assert!(result.is_err());
}

#[test]
fn register_command_duplicate_is_error() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("a", r#"qbal.register_command("cmd", function() end)"#)
    .unwrap();
  let result = engine.load_source("b", r#"qbal.register_command("cmd", function() end)"#);
  assert!(result.is_err());
}

// --- run_pending_commands ---

#[cfg(test)]
mod command_tests {}

// --- set_playbar ---

#[test]
fn set_playbar_queues_segment_with_current_plugin() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("myplugin", r#"qbal.set_playbar("hello world")"#)
    .unwrap();
  match one(&engine) {
    Action::SetPlaybarSegment { plugin, text } => {
      assert_eq!(plugin, "myplugin");
      assert_eq!(text, Some("hello world".to_string()));
    }
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn set_playbar_nil_queues_clear() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("myplugin", r#"qbal.set_playbar(nil)"#)
    .unwrap();
  match one(&engine) {
    Action::SetPlaybarSegment { plugin, text } => {
      assert_eq!(plugin, "myplugin");
      assert!(text.is_none());
    }
    other => panic!("unexpected effect: {:?}", std::mem::discriminant(&other)),
  }
}

#[cfg(test)]
mod playbar_effect_tests {}

// --- popup ---

#[test]
fn popup_plain_string_lines_work() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("test", r#"qbal.popup("My Title", "single line")"#)
    .unwrap();
  match one(&engine) {
    Action::ShowPopup(p) => {
      assert_eq!(p.title, "My Title");
      assert_eq!(p.lines.len(), 1);
      assert_eq!(p.lines[0].text, "single line");
      assert!(p.lines[0].fg.is_none());
      assert!(!p.lines[0].bold);
      assert!(!p.lines[0].italic);
    }
    other => panic!("unexpected: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn popup_array_of_strings() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("test", r#"qbal.popup("T", {"line 1", "line 2"})"#)
    .unwrap();
  match one(&engine) {
    Action::ShowPopup(p) => {
      assert_eq!(p.lines.len(), 2);
      assert_eq!(p.lines[0].text, "line 1");
      assert_eq!(p.lines[1].text, "line 2");
    }
    other => panic!("unexpected: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn popup_styled_table_lines() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source(
      "test",
      r#"qbal.popup("T", {{ text = "bold red", fg = "Red", bold = true, italic = false }})"#,
    )
    .unwrap();
  match one(&engine) {
    Action::ShowPopup(p) => {
      assert_eq!(p.lines.len(), 1);
      assert_eq!(p.lines[0].text, "bold red");
      assert_eq!(p.lines[0].fg, Some(crate::core::theme::Color::Red));
      assert!(p.lines[0].bold);
      assert!(!p.lines[0].italic);
    }
    other => panic!("unexpected: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn popup_bad_color_raises() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source(
    "test",
    r#"qbal.popup("T", {{ text = "hi", fg = "NotAColor" }})"#,
  );
  assert!(result.is_err(), "unknown named colors should raise");
  assert!(
    engine.shared.effects.borrow().is_empty(),
    "invalid popup must not be queued"
  );
}

#[test]
fn popup_missing_text_field_raises() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source("test", r#"qbal.popup("T", {{ bold = true }})"#);
  assert!(result.is_err(), "missing 'text' field should be an error");
}

#[test]
fn popup_non_table_non_string_line_raises() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source("test", r#"qbal.popup("T", {42})"#);
  assert!(result.is_err(), "integer line should be an error");
}

#[cfg(test)]
mod popup_effect_tests {}

// --- set_theme ---

#[test]
fn set_theme_valid_field_queues_effect() {
  let mut engine = ScriptEngine::new().unwrap();
  engine
    .load_source("test", r#"qbal.set_theme({ playbar_text = "Magenta" })"#)
    .unwrap();
  match one(&engine) {
    Action::SetTheme(pairs) => {
      assert_eq!(pairs.len(), 1);
      assert_eq!(pairs[0].0, crate::core::theme::ThemeField::PlaybarText);
      assert_eq!(pairs[0].1, crate::core::theme::Color::Magenta);
    }
    other => panic!("unexpected: {:?}", std::mem::discriminant(&other)),
  }
}

#[test]
fn set_theme_unknown_field_raises() {
  let mut engine = ScriptEngine::new().unwrap();
  let result = engine.load_source("test", r#"qbal.set_theme({ not_a_field = "Red" })"#);
  assert!(result.is_err(), "unknown theme field should raise");
}

#[test]
fn set_theme_bad_color_raises() {
  let mut engine = ScriptEngine::new().unwrap();
  for color in ["NotAColor", "999, 999, 999", "1, 2, 3, 4"] {
    let source = format!(r#"qbal.set_theme({{ playbar_text = "{color}" }})"#);
    let result = engine.load_source("test", &source);
    assert!(result.is_err(), "invalid color {color:?} should raise");
    assert!(
      engine.shared.effects.borrow().is_empty(),
      "invalid theme must not be queued"
    );
  }
}

#[cfg(test)]
mod theme_effect_tests {}

// --- diff_events ---

#[test]
fn diff_none_to_some_is_track_change() {
  let new = Some(playback(Some(track("uri:1", "A")), true, 0));
  let q = Some(vec![]);
  let events = diff_events(&None, &None, &new, &q);
  assert!(events.contains(&ScriptEvent::TrackChange));
  // None -> playing also flips is_playing.
  assert!(events.contains(&ScriptEvent::PlaybackStateChange));
}

#[test]
fn diff_track_change_on_different_uri() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 0));
  let new = Some(playback(Some(track("uri:2", "B")), true, 0));
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &new, &q);
  assert!(events.contains(&ScriptEvent::TrackChange));
  assert!(!events.contains(&ScriptEvent::PlaybackStateChange));
}

#[test]
fn diff_play_pause_flip() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 1000));
  let new = Some(playback(Some(track("uri:1", "A")), false, 1000));
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &new, &q);
  assert_eq!(events, vec![ScriptEvent::PlaybackStateChange]);
}

#[test]
fn diff_seek_backward_beyond_threshold() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 10_000));
  let new = Some(playback(Some(track("uri:1", "A")), true, 5_000));
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &new, &q);
  assert!(events.contains(&ScriptEvent::Seek));
}

#[test]
fn diff_seek_forward_beyond_threshold() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 1_000));
  let new = Some(playback(Some(track("uri:1", "A")), true, 9_000));
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &new, &q);
  assert!(events.contains(&ScriptEvent::Seek));
}

#[test]
fn diff_small_forward_jump_is_not_seek() {
  // A 3s forward jump is within the polling tolerance.
  let old = Some(playback(Some(track("uri:1", "A")), true, 1_000));
  let new = Some(playback(Some(track("uri:1", "A")), true, 4_000));
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &new, &q);
  assert!(!events.contains(&ScriptEvent::Seek));
}

#[test]
fn diff_volume_change() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 1_000));
  let mut new = playback(Some(track("uri:1", "A")), true, 1_000);
  new.volume_percent = Some(80);
  let q = Some(vec![]);
  let events = diff_events(&old, &q, &Some(new), &q);
  assert!(events.contains(&ScriptEvent::VolumeChange));
}

#[test]
fn diff_queue_change() {
  let old = Some(playback(Some(track("uri:1", "A")), true, 1_000));
  let new = old.clone();
  let old_q = Some(vec!["a".to_string()]);
  let new_q = Some(vec!["a".to_string(), "b".to_string()]);
  let events = diff_events(&old, &old_q, &new, &new_q);
  assert_eq!(events, vec![ScriptEvent::QueueChange]);
}

// --- dispatch-backed actions ---

#[cfg(test)]
mod action_tests {
  use super::*;
  use crate::core::action::RepeatSetting;

  #[test]
  fn set_repeat_maps_all_modes() {
    for (mode, expected) in [
      ("off", RepeatSetting::Off),
      ("track", RepeatSetting::Track),
      ("context", RepeatSetting::Context),
    ] {
      match run_action(&format!(r#"qbal.set_repeat("{mode}")"#)) {
        Action::SetRepeat(setting) => assert_eq!(setting, expected),
        _ => panic!("expected SetRepeat for mode '{mode}'"),
      }
    }
  }

  #[test]
  fn set_repeat_invalid_mode_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source("test", r#"qbal.set_repeat("all")"#)
      .is_err());
  }

  #[test]
  fn cycle_repeat_queues_cycle_effect() {
    assert!(matches!(
      run_action("qbal.cycle_repeat()"),
      Action::CycleRepeat
    ));
  }

  #[test]
  fn play_uri_track_uses_uri_list() {
    match run_action(r#"qbal.play_uri("qobuz:track:abc123")"#) {
      Action::PlayUris { uris, offset: None } => {
        assert_eq!(uris, vec!["qobuz:track:abc123".to_string()]);
      }
      _ => panic!("expected PlayUris"),
    }
  }

  #[test]
  fn play_uri_album_plays_the_whole_album() {
    assert!(matches!(
      run_action(r#"qbal.play_uri("qobuz:album:0825646")"#),
      Action::PlayAlbum { uri } if uri == "qobuz:album:0825646"
    ));
  }

  #[test]
  fn play_uri_garbage_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source("test", r#"qbal.play_uri("not-a-uri")"#)
      .is_err());
  }

  #[test]
  fn search_with_a_blank_query_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine.load_source("test", r#"qbal.search("  ")"#).is_err());
  }

  #[test]
  fn search_with_a_query_is_accepted() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source("test", r#"qbal.search(" joni ")"#)
      .is_ok());
  }

  #[test]
  fn play_uri_with_an_empty_string_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine.load_source("test", r#"qbal.play_uri("")"#).is_err());
  }
}

// --- async data reads (qbal.get_*) ---

#[cfg(test)]
mod data_read_tests {
  use super::*;

  #[test]
  fn navigating_to_a_removed_screen_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    let result = engine.load_source("navigator", r#"qbal.navigate("devices")"#);
    assert!(result.is_err());
  }
}

// --- timers ---

#[cfg(test)]
mod timer_tests {
  use super::*;
  use std::time::{Duration, Instant};

  #[test]
  fn set_timeout_fires_once() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "timer",
        r#"qbal.set_timeout(100, function() qbal.notify("fired", 1) end)"#,
      )
      .unwrap();

    let now = Instant::now();
    engine.process_timers_for_test(now); // arms; not due yet
    assert!(drain(&engine).is_empty());

    engine.process_timers_for_test(now + Duration::from_millis(150));
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "fired"),
      _ => panic!("expected timeout notify"),
    }

    // One-shot: never fires again.
    engine.process_timers_for_test(now + Duration::from_millis(500));
    assert!(drain(&engine).is_empty());
  }

  #[test]
  fn set_interval_repeats_and_skips_missed_periods() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "timer",
        r#"qbal.set_interval(100, function() qbal.notify("tick", 1) end)"#,
      )
      .unwrap();

    let now = Instant::now();
    engine.process_timers_for_test(now); // arm
    engine.process_timers_for_test(now + Duration::from_millis(150));
    assert_eq!(drain(&engine).len(), 1);

    engine.process_timers_for_test(now + Duration::from_millis(260));
    assert_eq!(drain(&engine).len(), 1);

    // A long stall fires ONCE (no catch-up burst), rescheduled from `now`.
    engine.process_timers_for_test(now + Duration::from_millis(2000));
    assert_eq!(drain(&engine).len(), 1);
  }

  #[test]
  fn cancel_timer_before_due_prevents_firing() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "timer",
        r#"
          local h = qbal.set_timeout(100, function() qbal.notify("nope", 1) end)
          qbal.cancel_timer(h)
        "#,
      )
      .unwrap();

    let now = Instant::now();
    engine.process_timers_for_test(now);
    engine.process_timers_for_test(now + Duration::from_millis(500));
    assert!(drain(&engine).is_empty());
  }

  #[test]
  fn erroring_interval_is_removed_with_notify_error() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "bad_timer",
        r#"qbal.set_interval(100, function() error("interval boom") end)"#,
      )
      .unwrap();

    let now = Instant::now();
    engine.process_timers_for_test(now);
    engine.process_timers_for_test(now + Duration::from_millis(150));
    match one(&engine) {
      Action::NotifyError(msg, 6) => {
        assert!(msg.contains("bad_timer"));
        assert!(msg.contains("interval boom"));
      }
      _ => panic!("expected timer error notify"),
    }

    // One strike: the interval is gone.
    engine.process_timers_for_test(now + Duration::from_millis(1000));
    assert!(drain(&engine).is_empty());
  }

  #[test]
  fn timer_set_inside_event_handler_fires() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "timer",
        r#"
          qbal.on("start", function()
            qbal.set_timeout(50, function() qbal.notify("deferred", 1) end)
          end)
        "#,
      )
      .unwrap();

    engine.emit(ScriptEvent::Start);
    assert!(drain(&engine).is_empty());

    let now = Instant::now();
    engine.process_timers_for_test(now); // arms next pass
    engine.process_timers_for_test(now + Duration::from_millis(100));
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "deferred"),
      _ => panic!("expected deferred timer notify"),
    }
  }

  #[test]
  fn timer_set_inside_timer_callback_fires_next_pass() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
      .load_source(
        "timer",
        r#"
          qbal.set_timeout(10, function()
            qbal.set_timeout(10, function() qbal.notify("nested", 1) end)
          end)
        "#,
      )
      .unwrap();

    let now = Instant::now();
    engine.process_timers_for_test(now);
    engine.process_timers_for_test(now + Duration::from_millis(20));
    assert!(drain(&engine).is_empty(), "outer fired, nested only queued");
    // The nested timer arms on the NEXT pass; its delay counts from there.
    engine.process_timers_for_test(now + Duration::from_millis(50));
    assert!(drain(&engine).is_empty(), "nested armed, not yet due");
    engine.process_timers_for_test(now + Duration::from_millis(100));
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "nested"),
      _ => panic!("expected nested timer notify"),
    }
  }

  #[test]
  fn negative_ms_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source("timer", r#"qbal.set_timeout(-5, function() end)"#)
      .is_err());
    assert!(engine
      .load_source("timer", r#"qbal.set_interval(0, function() end)"#)
      .is_err());
  }
}

// --- plugin_id sanitizer ---

#[test]
fn plugin_id_strips_lua_suffix_and_sanitizes() {
  use super::shared::plugin_id;
  assert_eq!(plugin_id("stats.lua"), "stats");
  assert_eq!(plugin_id("my-plugin"), "my-plugin");
  assert_eq!(plugin_id("weird name!.lua"), "weird_name_");
  assert_eq!(plugin_id("dir.plugin"), "dir_plugin");
  assert_eq!(plugin_id("под.lua"), "___");
}

// --- directory plugin loading (qbal plugin add) ---

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Fresh, unique temp directory to act as a config dir.
fn temp_config_dir() -> PathBuf {
  let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
  let dir = std::env::temp_dir().join(format!("qbal_lua_load_{}_{}", std::process::id(), n));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  dir
}

fn write_file(path: &Path, contents: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, contents).unwrap();
}

/// True if any queued effect is a successful Notify carrying `needle`.
fn has_notify(engine: &ScriptEngine, needle: &str) -> bool {
  drain(engine).into_iter().any(|e| match e {
    Action::Notify(msg, _) => msg.contains(needle),
    _ => false,
  })
}

#[test]
fn dir_plugin_main_lua_is_loaded() {
  let cfg = temp_config_dir();
  write_file(
    &cfg.join("plugins").join("foo").join("main.lua"),
    r#"qbal.notify("loaded foo", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 1);
  assert!(has_notify(&engine, "loaded foo"));
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn dir_plugin_init_lua_is_used_as_fallback() {
  let cfg = temp_config_dir();
  write_file(
    &cfg.join("plugins").join("bar").join("init.lua"),
    r#"qbal.notify("loaded bar", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 1);
  assert!(has_notify(&engine, "loaded bar"));
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn dir_plugin_without_entry_point_is_skipped() {
  let cfg = temp_config_dir();
  // Directory exists but has no main.lua/init.lua, plus a hidden dir that must be ignored.
  std::fs::create_dir_all(cfg.join("plugins").join("empty")).unwrap();
  write_file(
    &cfg.join("plugins").join(".hidden").join("main.lua"),
    r#"qbal.notify("should not load", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 0);
  assert!(drain(&engine).is_empty());
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn dir_plugin_can_require_sibling_module() {
  let cfg = temp_config_dir();
  let plugin = cfg.join("plugins").join("qux");
  write_file(
    &plugin.join("helper.lua"),
    r#"return { msg = "from helper" }"#,
  );
  write_file(
    &plugin.join("main.lua"),
    r#"
      local helper = require("helper")
      qbal.notify(helper.msg, 1)
    "#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  // A successful load proves `require` resolved the sibling module via package.path.
  assert_eq!(loaded, 1);
  assert!(has_notify(&engine, "from helper"));
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn single_file_and_directory_plugins_both_load() {
  let cfg = temp_config_dir();
  write_file(
    &cfg.join("plugins").join("flat.lua"),
    r#"qbal.notify("flat", 1)"#,
  );
  write_file(
    &cfg.join("plugins").join("nested").join("main.lua"),
    r#"qbal.notify("nested", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 2);
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn directory_named_with_lua_extension_loads_once_without_error() {
  // A directory literally named `weird.lua` must be treated only as a directory plugin,
  // not also fed to the single-file path (which would raise a spurious load error).
  let cfg = temp_config_dir();
  write_file(
    &cfg.join("plugins").join("weird.lua").join("main.lua"),
    r#"qbal.notify("weird ok", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 1);
  let effects = drain(&engine);
  assert!(
    !effects
      .iter()
      .any(|e| matches!(e, Action::NotifyError(_, _))),
    "a .lua-named directory must not produce a load error"
  );
  std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn hidden_single_file_plugin_is_skipped() {
  // Hidden files (e.g. `._foo.lua` cruft copied from a Mac) must be ignored, matching the directory branch.
  let cfg = temp_config_dir();
  write_file(
    &cfg.join("plugins").join(".secret.lua"),
    r#"qbal.notify("should not load", 1)"#,
  );

  let mut engine = ScriptEngine::new().unwrap();
  let loaded = engine.load_user_scripts(&cfg);

  assert_eq!(loaded, 0);
  assert!(drain(&engine).is_empty());
  std::fs::remove_dir_all(&cfg).unwrap();
}

// --- plugin storage ---

#[cfg(test)]
mod storage_tests {
  use super::*;

  /// Engine with a temp config dir registered (empty plugins dir is fine).
  fn engine_with_dir(cfg: &Path) -> ScriptEngine {
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_user_scripts(cfg);
    engine
  }

  #[test]
  fn storage_round_trip_within_one_engine() {
    let cfg = temp_config_dir();
    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source(
        "a.lua",
        r#"
          qbal.storage_set("count", 42)
          qbal.storage_set("nested", { x = 1, tags = {"a", "b"} })
          qbal.notify(qbal.storage_get("count") .. ":" .. qbal.storage_get("nested").tags[2], 1)
        "#,
      )
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "42:b"),
      _ => panic!("expected storage round-trip notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_flushes_and_reloads_in_new_engine() {
    let cfg = temp_config_dir();
    {
      let mut engine = engine_with_dir(&cfg);
      engine
        .load_source("a.lua", r#"qbal.storage_set("song", "Nightcall")"#)
        .unwrap();
      engine.flush_storage(true);
    }

    // The file exists under the sanitized plugin id.
    let file = cfg.join("plugin-data").join("a.json");
    assert!(file.is_file(), "expected {}", file.display());

    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source(
        "a.lua",
        r#"qbal.notify(qbal.storage_get("song") or "missing", 1)"#,
      )
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "Nightcall"),
      _ => panic!("expected persisted value notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_set_nil_deletes_key() {
    let cfg = temp_config_dir();
    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source(
        "a.lua",
        r#"
          qbal.storage_set("gone", "soon")
          qbal.storage_set("gone", nil)
          qbal.notify(tostring(qbal.storage_get("gone")) .. ":" .. #qbal.storage_keys(), 1)
        "#,
      )
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "nil:0"),
      _ => panic!("expected nil-delete notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_remove_and_keys() {
    let cfg = temp_config_dir();
    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source(
        "a.lua",
        r#"
          qbal.storage_set("one", 1)
          qbal.storage_set("two", 2)
          qbal.storage_remove("one")
          local keys = qbal.storage_keys()
          qbal.notify(#keys .. ":" .. keys[1], 1)
        "#,
      )
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "1:two"),
      _ => panic!("expected remove/keys notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_set_function_value_raises() {
    let cfg = temp_config_dir();
    let mut engine = engine_with_dir(&cfg);
    let result = engine.load_source("a.lua", r#"qbal.storage_set("f", function() end)"#);
    assert!(result.is_err(), "functions must not be storable");
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_isolated_per_plugin() {
    let cfg = temp_config_dir();
    {
      let mut engine = engine_with_dir(&cfg);
      engine
        .load_source("a.lua", r#"qbal.storage_set("who", "plugin a")"#)
        .unwrap();
      engine
        .load_source("b", r#"qbal.storage_set("who", "plugin b")"#)
        .unwrap();
      engine.flush_storage(true);
    }

    assert!(cfg.join("plugin-data").join("a.json").is_file());
    assert!(cfg.join("plugin-data").join("b.json").is_file());

    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source("b", r#"qbal.notify(qbal.storage_get("who"), 1)"#)
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "plugin b"),
      _ => panic!("expected isolated namespace notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn corrupt_storage_file_starts_empty() {
    let cfg = temp_config_dir();
    write_file(&cfg.join("plugin-data").join("a.json"), "{not json");

    let mut engine = engine_with_dir(&cfg);
    engine
      .load_source(
        "a.lua",
        r#"qbal.notify(tostring(qbal.storage_get("anything")), 1)"#,
      )
      .unwrap();
    match one(&engine) {
      Action::Notify(msg, 1) => assert_eq!(msg, "nil"),
      _ => panic!("expected empty-after-corruption notify"),
    }
    std::fs::remove_dir_all(&cfg).unwrap();
  }

  #[test]
  fn storage_outside_plugin_context_raises() {
    // current_plugin is empty outside load/callback paths; simulate by loading
    // a chunk with an empty plugin name.
    let cfg = temp_config_dir();
    let mut engine = engine_with_dir(&cfg);
    let result = engine.load_source("", r#"qbal.storage_get("x")"#);
    assert!(result.is_err());
    std::fs::remove_dir_all(&cfg).unwrap();
  }
}

// --- state events (route/device/search) + shuffle/repeat diffs ---

#[cfg(test)]
mod state_event_tests {
  use super::*;

  use crate::core::plugin_api::DeviceInfo;

  use crate::infra::scripting::events::diff_state_events;

  fn device(id: &str, active: bool) -> DeviceInfo {
    DeviceInfo {
      id: Some(id.to_string()),
      name: id.to_string(),
      kind: "computer".to_string(),
      is_active: active,
      volume_percent: Some(50),
    }
  }

  #[test]
  fn diff_shuffle_change() {
    let old = Some(playback(Some(track("uri:1", "A")), true, 0));
    let mut new_pb = playback(Some(track("uri:1", "A")), true, 0);
    new_pb.shuffle = true;
    let q = Some(vec![]);
    let events = diff_events(&old, &q, &Some(new_pb), &q);
    assert_eq!(events, vec![ScriptEvent::ShuffleChange]);
  }

  #[test]
  fn diff_repeat_change() {
    let old = Some(playback(Some(track("uri:1", "A")), true, 0));
    let mut new_pb = playback(Some(track("uri:1", "A")), true, 0);
    new_pb.repeat = "track".to_string();
    let q = Some(vec![]);
    let events = diff_events(&old, &q, &Some(new_pb), &q);
    assert_eq!(events, vec![ScriptEvent::RepeatChange]);
  }

  #[test]
  fn diff_none_to_some_is_not_shuffle_or_repeat_change() {
    let mut new_pb = playback(Some(track("uri:1", "A")), false, 0);
    new_pb.shuffle = true;
    new_pb.repeat = "context".to_string();
    let q = Some(vec![]);
    let events = diff_events(&None, &q, &Some(new_pb), &q);
    assert!(!events.contains(&ScriptEvent::ShuffleChange));
    assert!(!events.contains(&ScriptEvent::RepeatChange));
  }

  #[test]
  fn diff_state_route_change_carries_new_name() {
    let events = diff_state_events("home", "queue", &[], &[], false);
    assert_eq!(events, vec![ScriptEvent::RouteChange("queue".to_string())]);
  }

  #[test]
  fn diff_state_device_change_on_active_flip() {
    let old = [device("a", true), device("b", false)];
    let new = [device("a", false), device("b", true)];
    let events = diff_state_events("home", "home", &old, &new, false);
    assert_eq!(events, vec![ScriptEvent::DeviceChange]);
  }

  #[test]
  fn diff_state_no_events_when_identical() {
    let devs = [device("a", true)];
    assert!(diff_state_events("home", "home", &devs, &devs, false).is_empty());
  }

  #[test]
  fn diff_state_search_advance() {
    let events = diff_state_events("home", "home", &[], &[], true);
    assert_eq!(events, vec![ScriptEvent::SearchResults]);
  }

  #[test]
  fn navigate_unknown_target_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source("nav", r#"qbal.navigate("narnia")"#)
      .is_err());
  }
}

// --- custom plugin screens ---

#[cfg(test)]
mod screen_tests {
  use super::*;
  use crate::core::app::{ActiveBlock, RouteId};

  const STATS_SCREEN: &str = r#"
    qbal.register_screen("stats", {
      title = "Stats",
      on_key = function(key)
        qbal.notify("key: " .. key, 1)
      end,
      on_open = function() qbal.notify("opened", 1) end,
      on_close = function() qbal.notify("closed", 1) end,
    })
  "#;

  #[test]
  fn set_screen_rejects_unknown_widget_type_and_bad_selected() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_source("stats.lua", STATS_SCREEN).unwrap();
    assert!(engine
      .load_source(
        "stats.lua",
        r#"qbal.set_screen("stats", {{ type = "table" }})"#
      )
      .is_err());
    assert!(engine
      .load_source(
        "stats.lua",
        r#"qbal.set_screen("stats", {{ type = "list", items = {"x"}, selected = 0 }})"#
      )
      .is_err());
  }

  /// Asserts publishing `widgets` fails, and returns the error text.
  fn publish_widgets_err(widgets: &str) -> String {
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_source("stats.lua", STATS_SCREEN).unwrap();
    engine
      .load_source(
        "stats.lua",
        &format!(r#"qbal.set_screen("stats", {widgets})"#),
      )
      .expect_err("expected set_screen to fail")
      .to_string()
  }

  #[test]
  fn set_screen_rejects_a_second_cover_art_widget_anywhere_in_the_tree() {
    let err = publish_widgets_err(
      r#"{
        { type = "cover_art" },
        { type = "row", children = { { type = "cover_art" } } },
      }"#,
    );
    assert!(err.contains("at most one cover_art widget"), "got: {err}");
  }

  #[test]
  fn set_screen_rejects_unknown_cover_art_source_and_fit() {
    let err = publish_widgets_err(r#"{ { type = "cover_art", source = "album" } }"#);
    assert!(err.contains("unknown cover_art source"), "got: {err}");

    let err = publish_widgets_err(r#"{ { type = "cover_art", fit = "cover" } }"#);
    assert!(err.contains("unknown cover_art fit"), "got: {err}");
  }

  #[test]
  fn set_screen_rejects_conflicting_or_out_of_range_size_hints() {
    let err = publish_widgets_err(
      r#"{ { type = "paragraph", lines = {"x"}, height = 4, height_percent = 50 } }"#,
    );
    assert!(err.contains("at most one of"), "got: {err}");

    for percent in ["0", "101"] {
      let err = publish_widgets_err(&format!(
        r#"{{ {{ type = "paragraph", lines = {{"x"}}, width_percent = {percent} }} }}"#
      ));
      assert!(err.contains("must be between 1 and 100"), "got: {err}");
    }
  }

  #[test]
  fn set_screen_rejects_a_container_without_children() {
    let err = publish_widgets_err(r#"{ { type = "row" } }"#);
    assert!(err.contains("must have a 'children' array"), "got: {err}");
  }

  #[test]
  fn set_screen_unregistered_or_foreign_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_source("stats.lua", STATS_SCREEN).unwrap();

    // Not registered at all.
    assert!(engine
      .load_source("stats.lua", r#"qbal.set_screen("nope", {})"#)
      .is_err());

    // Registered, but owned by another plugin.
    let err = engine
      .load_source("intruder.lua", r#"qbal.show_screen("stats")"#)
      .unwrap_err()
      .to_string();
    assert!(err.contains("belongs to plugin"), "got: {err}");
  }

  #[test]
  fn duplicate_screen_name_raises() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.load_source("stats.lua", STATS_SCREEN).unwrap();
    assert!(engine
      .load_source(
        "other.lua",
        r#"qbal.register_screen("stats", { on_key = function() end })"#
      )
      .is_err());
  }

  #[test]
  fn register_screen_requires_on_key() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine
      .load_source(
        "stats.lua",
        r#"qbal.register_screen("stats", { title = "T" })"#
      )
      .is_err());
  }

  #[test]
  fn route_name_for_plugin_screen_is_prefixed() {
    use crate::core::app::Route;
    let route = Route {
      id: RouteId::PluginScreen("stats".to_string()),
      active_block: ActiveBlock::PluginScreen,
      hovered_block: ActiveBlock::PluginScreen,
    };
    assert_eq!(crate::core::plugin_api::route_name(&route), "plugin:stats");
  }
}

// --- config() read ---

#[cfg(test)]
mod config_tests {}

/// The load error of `source`, from a thread, so a time limit that fails cannot hang the run.
fn load_error_within_limit(source: &'static str) -> String {
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let mut engine = ScriptEngine::new().unwrap();
    let _ = tx.send(
      engine
        .load_source("spin", source)
        .map_err(|e| e.to_string()),
    );
  });
  rx.recv_timeout(std::time::Duration::from_secs(10))
    .expect("the time limit did not stop the plugin")
    .unwrap_err()
}

#[test]
fn a_plugin_that_loops_forever_is_stopped() {
  let err = load_error_within_limit("while true do end");
  assert!(err.contains("stopped after running"), "{err}");
  ScriptEngine::new()
    .unwrap()
    .load_source("fine", "local x = 0 for i = 1, 1000 do x = x + i end")
    .unwrap();
}

#[test]
fn a_protected_call_cannot_catch_the_time_limit() {
  for source in [
    "while true do pcall(function() while true do end end) end",
    "while true do xpcall(function() while true do end end, function() end) end",
    "while true do coroutine.resume(coroutine.create(function() while true do end end)) end",
  ] {
    let err = load_error_within_limit(source);
    assert!(err.contains("stopped after running"), "{source}: {err}");
  }
  ScriptEngine::new()
    .unwrap()
    .load_source(
      "fine",
      "assert(not pcall(error, 'x')); assert(select('#', pcall(print)) == 1)",
    )
    .unwrap();
}
