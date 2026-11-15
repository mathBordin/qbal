//! The device the sound reaches, and the format it runs at.
//!
//! Two sources, read on a background thread every [`POLL_EVERY`] while qbal
//! plays, so neither the tick nor a draw touches the file system or spawns a
//! process:
//!
//! - **PipeWire** (`pw-dump`, from `pipewire-utils`) names the running output:
//!   speakers, the analog jack, a USB DAC, HDMI, or a Bluetooth device and the
//!   codec it encodes to (`api.bluez5.codec`).
//! - **The kernel** publishes each open playback stream's parameters in
//!   `/proc/asound/cardN/pcmMp/subK/hw_params`: the exact format a sound card
//!   (built-in, USB) runs at. Bluetooth has no such card.
//!
//! Without `pw-dump` the kernel's card format still shows, as a plain "Card".

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::core::stream_format::{
  alsa_bits, parse_hw_params, DeviceKind, HardwareFormat, OutputDevice,
};

/// How often the output is re-read.
const POLL_EVERY: Duration = Duration::from_secs(2);

struct Shared {
  latest: Mutex<Option<OutputDevice>>,
  /// Whether qbal is playing: the thread reads nothing while it is not.
  active: AtomicBool,
}

/// Polls the running output on its own thread; the thread ends once the probe
/// is dropped.
pub struct HardwareProbe {
  shared: Arc<Shared>,
}

impl HardwareProbe {
  pub fn start() -> Self {
    let shared = Arc::new(Shared {
      latest: Mutex::new(None),
      active: AtomicBool::new(false),
    });
    let weak: Weak<Shared> = Arc::downgrade(&shared);
    let spawned = std::thread::Builder::new()
      .name("qbal-output-probe".into())
      .spawn(move || loop {
        let Some(shared) = weak.upgrade() else {
          return;
        };
        let found = if shared.active.load(Ordering::Relaxed) {
          read_output()
        } else {
          None
        };
        if let Ok(mut slot) = shared.latest.lock() {
          *slot = found;
        }
        drop(shared);
        std::thread::sleep(POLL_EVERY);
      });
    if let Err(e) = spawned {
      log::warn!("could not start the output device probe: {e}");
    }
    HardwareProbe { shared }
  }

  /// Tell the probe whether qbal is playing.
  pub fn set_active(&self, active: bool) {
    self.shared.active.store(active, Ordering::Relaxed);
  }

  /// The output read last; `None` when nothing is playing.
  pub fn latest(&self) -> Option<OutputDevice> {
    self.shared.latest.lock().ok().and_then(|slot| slot.clone())
  }
}

/// Read PipeWire's running sink and the kernel's running card, and merge them.
fn read_output() -> Option<OutputDevice> {
  let sink = pw_dump().and_then(|json| running_sink(&json));
  let card = read_running(Path::new("/proc/asound"));
  merge(sink, card)
}

/// `pw-dump`'s JSON, or `None` when it is missing or fails. `timeout` keeps a
/// stuck PipeWire from stalling the probe.
fn pw_dump() -> Option<Value> {
  let output = std::process::Command::new("timeout")
    .args(["3", "pw-dump"])
    .stdin(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  serde_json::from_slice(&output.stdout).ok()
}

/// What PipeWire says about the running hardware sink.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sink {
  kind: DeviceKind,
  name: Option<String>,
  /// The format PipeWire negotiated with the sink.
  format: Option<HardwareFormat>,
}

/// The first running `Audio/Sink` node backed by a sound card or a Bluetooth
/// device (virtual sinks such as an equalizer are skipped), described from its
/// node and its device object.
fn running_sink(dump: &Value) -> Option<Sink> {
  let objects = dump.as_array()?;
  let node = objects.iter().find(|object| {
    let info = &object["info"];
    let props = &info["props"];
    object["type"]
      .as_str()
      .is_some_and(|t| t.ends_with("Interface:Node"))
      && props["media.class"].as_str() == Some("Audio/Sink")
      && info["state"].as_str() == Some("running")
      && matches!(props["device.api"].as_str(), Some("alsa" | "bluez5"))
  })?;
  let props = &node["info"]["props"];
  // A number in `pw-dump`; accept the string some tools print too.
  let device_id = props["device.id"]
    .as_u64()
    .or_else(|| props["device.id"].as_str()?.parse().ok());
  let device = device_id.and_then(|id| {
    objects.iter().find(|object| {
      object["id"].as_u64() == Some(id)
        && object["type"]
          .as_str()
          .is_some_and(|t| t.ends_with("Interface:Device"))
    })
  });
  let device_props = device.map(|d| &d["info"]["props"]);
  let text = |value: &Value| value.as_str().map(str::to_string).filter(|s| !s.is_empty());
  let name = device_props
    .and_then(|p| text(&p["device.description"]))
    .or_else(|| text(&props["node.description"]));

  let kind = if props["device.api"].as_str() == Some("bluez5") {
    DeviceKind::Bluetooth {
      codec: text(&props["api.bluez5.codec"]),
    }
  } else if device_props.and_then(|p| p["device.bus"].as_str()) == Some("usb") {
    DeviceKind::UsbDac
  } else {
    let route = device.and_then(|d| {
      d["info"]["params"]["Route"]
        .as_array()?
        .iter()
        .find(|route| route["direction"].as_str() == Some("Output"))
    });
    let route_name = route
      .and_then(|r| r["name"].as_str())
      .unwrap_or_default()
      .to_lowercase();
    if route_name.contains("speaker") {
      DeviceKind::Speakers
    } else if route_name.contains("hdmi") || route_name.contains("iec958") {
      DeviceKind::Hdmi
    } else if route_name.starts_with("analog") {
      DeviceKind::Wired
    } else {
      DeviceKind::Card
    }
  };

  let format = node["info"]["params"]["Format"]
    .as_array()
    .and_then(|formats| formats.first())
    .and_then(|f| {
      let name = f["format"].as_str()?;
      let (bits, float) = alsa_bits(name)?;
      Some(HardwareFormat {
        format: name.to_string(),
        bits,
        float,
        sample_rate: u32::try_from(f["rate"].as_u64()?).ok()?,
        channels: u16::try_from(f["channels"].as_u64()?).ok()?,
        card: None,
      })
    });
  Some(Sink { kind, name, format })
}

/// One description from both sources. For a sound card the kernel's format
/// wins (it is what the card runs at); Bluetooth has no card, so PipeWire's
/// stands. Without PipeWire, the card alone is a plain "Card".
fn merge(sink: Option<Sink>, card: Option<HardwareFormat>) -> Option<OutputDevice> {
  match sink {
    Some(Sink {
      kind: kind @ DeviceKind::Bluetooth { .. },
      name,
      format,
    }) => Some(OutputDevice { kind, name, format }),
    Some(Sink { kind, name, format }) => Some(OutputDevice {
      kind,
      name,
      format: card.or(format),
    }),
    None => card.map(|card| OutputDevice {
      kind: DeviceKind::Card,
      name: card.card.clone(),
      format: Some(card),
    }),
  }
}

/// The first running playback stream under `root` (normally `/proc/asound`).
fn read_running(root: &Path) -> Option<HardwareFormat> {
  let mut cards: Vec<_> = std::fs::read_dir(root)
    .ok()?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| {
      path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("card"))
    })
    .collect();
  cards.sort();
  cards.into_iter().find_map(|card| {
    let id = std::fs::read_to_string(card.join("id")).ok();
    let id = id.as_deref().map(str::trim);
    let mut streams: Vec<_> = std::fs::read_dir(&card)
      .ok()?
      .flatten()
      .map(|entry| entry.path())
      .filter(|path| {
        path
          .file_name()
          .and_then(|name| name.to_str())
          .is_some_and(|name| name.starts_with("pcm") && name.ends_with('p'))
      })
      .collect();
    streams.sort();
    streams.into_iter().find_map(|pcm| {
      let mut subs: Vec<_> = std::fs::read_dir(&pcm)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("hw_params"))
        .filter(|path| path.is_file())
        .collect();
      subs.sort();
      subs.into_iter().find_map(|file| {
        let text = std::fs::read_to_string(file).ok()?;
        parse_hw_params(&text, id)
      })
    })
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
  }

  #[test]
  fn the_first_running_playback_stream_wins_and_capture_is_ignored() {
    let root = tempfile::tempdir().unwrap();
    let running = "format: S32_LE\nchannels: 2\nrate: 44100 (44100/1)\n";
    write(root.path(), "card0/id", "PCH\n");
    write(root.path(), "card0/pcm0p/sub0/hw_params", "closed\n");
    write(root.path(), "card0/pcm0c/sub0/hw_params", running);
    write(root.path(), "card1/id", "DAC\n");
    write(root.path(), "card1/pcm0p/sub0/hw_params", running);
    let found = read_running(root.path()).unwrap();
    assert_eq!(found.card.as_deref(), Some("DAC"));
    assert_eq!(found.sample_rate, 44_100);
    assert_eq!(found.bits, 32);
  }

  /// A trimmed `pw-dump`: one device object and one sink node per output.
  fn dump(node_props: &str, device_props: &str, route: &str, state: &str) -> Value {
    serde_json::from_str(&format!(
      r#"[
        {{ "id": 40, "type": "PipeWire:Interface:Device",
           "info": {{ "props": {{ {device_props} }},
                      "params": {{ "Route": [
                        {{ "direction": "Input", "name": "analog-input-mic" }},
                        {{ "direction": "Output", "name": "{route}", "description": "x" }} ] }} }} }},
        {{ "id": 30, "type": "PipeWire:Interface:Node",
           "info": {{ "state": "running", "props": {{ "media.class": "Audio/Sink",
                      "node.description": "Virtual EQ", "device.api": null }} }} }},
        {{ "id": 41, "type": "PipeWire:Interface:Node",
           "info": {{ "state": "{state}",
                      "props": {{ "media.class": "Audio/Sink", "device.id": "40", {node_props} }},
                      "params": {{ "Format": [ {{ "mediaType": "audio", "mediaSubtype": "raw",
                        "format": "S32LE", "rate": 96000, "channels": 2 }} ] }} }} }}
      ]"#
    ))
    .unwrap()
  }

  fn alsa(route: &str, bus: &str) -> Value {
    dump(
      r#""device.api": "alsa", "node.description": "Built-in Audio Analog Stereo""#,
      &format!(r#""device.bus": "{bus}", "device.description": "Built-in Audio""#),
      route,
      "running",
    )
  }

  #[test]
  fn pipewire_names_speakers_the_wired_jack_hdmi_and_usb_dacs() {
    let kind = |dump: Value| running_sink(&dump).unwrap().kind;
    assert_eq!(
      kind(alsa("analog-output-speaker", "pci")),
      DeviceKind::Speakers
    );
    assert_eq!(
      kind(alsa("analog-output-headphones", "pci")),
      DeviceKind::Wired
    );
    assert_eq!(
      kind(alsa("analog-output-lineout", "pci")),
      DeviceKind::Wired
    );
    assert_eq!(kind(alsa("hdmi-output-0", "pci")), DeviceKind::Hdmi);
    assert_eq!(kind(alsa("analog-output", "usb")), DeviceKind::UsbDac);
    let sink = running_sink(&alsa("analog-output-speaker", "pci")).unwrap();
    assert_eq!(sink.name.as_deref(), Some("Built-in Audio"));
    let format = sink.format.unwrap();
    assert_eq!((format.bits, format.sample_rate), (32, 96_000));
  }

  #[test]
  fn a_bluetooth_sink_reports_its_codec_and_name() {
    let dump = dump(
      r#""device.api": "bluez5", "api.bluez5.codec": "ldac", "api.bluez5.profile": "a2dp-sink""#,
      r#""device.bus": "bluetooth", "device.description": "WH-1000XM4""#,
      "headset-output",
      "running",
    );
    let sink = running_sink(&dump).unwrap();
    assert_eq!(
      sink.kind,
      DeviceKind::Bluetooth {
        codec: Some("ldac".into())
      }
    );
    assert_eq!(sink.name.as_deref(), Some("WH-1000XM4"));
  }

  #[test]
  fn an_idle_sink_or_a_virtual_one_is_not_the_output() {
    let idle = dump(
      r#""device.api": "alsa""#,
      r#""device.bus": "pci""#,
      "analog-output-speaker",
      "suspended",
    );
    assert_eq!(
      running_sink(&idle),
      None,
      "the running virtual EQ is skipped too"
    );
    assert_eq!(running_sink(&serde_json::json!({})), None);
  }

  #[test]
  fn the_card_format_wins_for_a_card_and_pipewire_stands_for_bluetooth() {
    let card = parse_hw_params(
      "format: S24_3LE\nchannels: 2\nrate: 44100 (44100/1)\n",
      Some("DAC"),
    );
    let usb = running_sink(&alsa("analog-output", "usb"));
    let merged = merge(usb, card.clone()).unwrap();
    assert_eq!(merged.kind, DeviceKind::UsbDac);
    assert_eq!(
      merged.format.as_ref().map(|f| (f.bits, f.sample_rate)),
      Some((24, 44_100))
    );

    let bt = Sink {
      kind: DeviceKind::Bluetooth {
        codec: Some("aac".into()),
      },
      name: Some("Buds".into()),
      format: card.clone(),
    };
    let merged = merge(Some(bt.clone()), None).unwrap();
    assert_eq!(merged.format, bt.format);

    let alone = merge(None, card).unwrap();
    assert_eq!(alone.kind, DeviceKind::Card);
    assert_eq!(alone.name.as_deref(), Some("DAC"));
    assert_eq!(merge(None, None), None);
  }

  #[test]
  fn nothing_running_or_no_asound_directory_reports_nothing() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "card0/pcm0p/sub0/hw_params", "closed\n");
    assert_eq!(read_running(root.path()), None);
    assert_eq!(read_running(&root.path().join("missing")), None);
  }
}
