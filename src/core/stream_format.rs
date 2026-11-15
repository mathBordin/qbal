//! How the playbar words an audio format: the sample rate in kHz, the format
//! the output device was opened with, and the format the sound card itself
//! runs at.
//!
//! Plain data and pure functions, so the playbar and the tests share one
//! wording. The labels are numbers and units ("24-bit", "96 kHz", "1411 kbps"),
//! which read the same in every language.

/// A sample rate as kHz without the unit: `96`, `44.1`.
pub fn khz(sample_rate: u32) -> String {
  if sample_rate.is_multiple_of(1000) {
    (sample_rate / 1000).to_string()
  } else {
    format!("{:.1}", sample_rate as f64 / 1000.0)
  }
}

/// The format the audio output device was opened with: what qbal hands to the
/// sound system, after rodio converted the decoded track to it. The sound
/// server behind the device (PipeWire, PulseAudio) may still convert again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFormat {
  pub sample_rate: u32,
  pub channels: u16,
  /// Bits per sample the device takes.
  pub bits: u8,
  /// Whether those samples are floating point.
  pub float: bool,
  /// The device's name, when it reports one.
  pub device: Option<String>,
}

/// The sample type: `32-bit float`, `16-bit`.
fn sample_label(bits: u8, float: bool) -> String {
  if float {
    format!("{bits}-bit float")
  } else {
    format!("{bits}-bit")
  }
}

impl OutputFormat {
  /// The label from the most to the least detailed, for the widest line that
  /// fits.
  pub fn candidates(&self) -> Vec<String> {
    let base = format!(
      "{} \u{b7} {} kHz \u{b7} {} ch",
      sample_label(self.bits, self.float),
      khz(self.sample_rate),
      self.channels
    );
    let mut all = Vec::with_capacity(3);
    if let Some(device) = self.device.as_deref().filter(|d| !d.is_empty()) {
      all.push(format!("{base} \u{b7} {device}"));
    }
    all.push(base);
    all.push(format!("{} kHz", khz(self.sample_rate)));
    all
  }
}

/// The format a sound card is running at, as the kernel reports it in
/// `/proc/asound/card*/pcm*p/sub*/hw_params`. This is the end of the chain:
/// what reaches the DAC after the sound server (PipeWire, PulseAudio) mixed
/// and converted the app's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareFormat {
  /// The kernel's sample format name, e.g. `S24_3LE`, `S32_LE`, `FLOAT_LE`.
  pub format: String,
  pub bits: u8,
  pub float: bool,
  pub sample_rate: u32,
  pub channels: u16,
  /// The card's short id (`/proc/asound/cardN/id`), e.g. `PCH`, `DAC`.
  pub card: Option<String>,
}

impl HardwareFormat {
  /// `24-bit · 96 kHz`.
  pub fn label(&self) -> String {
    format!(
      "{} \u{b7} {} kHz",
      sample_label(self.bits, self.float),
      khz(self.sample_rate)
    )
  }
}

/// Bits per sample of an ALSA or PipeWire format name: `S16_LE` and `S16LE`
/// are 16, `S24_3LE` 24, `FLOAT_LE` and `F32LE` 32 and float, `FLOAT64_LE` and
/// `F64LE` 64 and float. `None` for formats with no plain sample width (DSD,
/// IEC958 passthrough).
pub(crate) fn alsa_bits(format: &str) -> Option<(u8, bool)> {
  if let Some(rest) = format.strip_prefix("FLOAT") {
    return Some((if rest.starts_with("64") { 64 } else { 32 }, true));
  }
  if let Some(rest) = format.strip_prefix('F') {
    return Some((if rest.starts_with("64") { 64 } else { 32 }, true));
  }
  let digits = format.strip_prefix(['S', 'U'])?;
  let end = digits
    .find(|c: char| !c.is_ascii_digit())
    .unwrap_or(digits.len());
  digits[..end].parse().ok().map(|bits| (bits, false))
}

/// Parse one `hw_params` file; `None` when the stream is `closed` or the file
/// misses a field. `card` is the card's id, carried into the result.
pub fn parse_hw_params(text: &str, card: Option<&str>) -> Option<HardwareFormat> {
  let field = |name: &str| {
    text.lines().find_map(|line| {
      let (key, value) = line.split_once(':')?;
      (key.trim() == name).then(|| value.trim())
    })
  };
  let format = field("format")?;
  let (bits, float) = alsa_bits(format)?;
  // `rate: 96000 (96000/1)`: the number before the fraction.
  let sample_rate = field("rate")?.split_whitespace().next()?.parse().ok()?;
  let channels = field("channels")?.parse().ok()?;
  Some(HardwareFormat {
    format: format.to_string(),
    bits,
    float,
    sample_rate,
    channels,
    card: card.map(str::to_string).filter(|c| !c.is_empty()),
  })
}

/// What kind of output the sound reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceKind {
  /// Built-in speakers.
  Speakers,
  /// The analog jack: headphones, a headset or line out.
  Wired,
  /// A USB audio device: a USB-C or USB-A DAC, a USB headset.
  UsbDac,
  /// HDMI or DisplayPort audio.
  Hdmi,
  /// Bluetooth, with the codec PipeWire encodes to (`ldac`, `aac`, `sbc`…).
  Bluetooth { codec: Option<String> },
  /// A sound card PipeWire did not describe.
  Card,
}

/// The device the sound reaches and the format it runs at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
  pub kind: DeviceKind,
  /// The device's name, e.g. `WH-1000XM4`, `FiiO KA3`.
  pub name: Option<String>,
  /// For a sound card, the format the kernel says it runs at. For Bluetooth,
  /// the audio PipeWire feeds the encoder, of which only the rate is shown:
  /// the codec decides the quality from there.
  pub format: Option<HardwareFormat>,
}

/// The translated names of the output kinds.
#[derive(Debug, Clone, Copy)]
pub struct DeviceWords<'a> {
  pub speakers: &'a str,
  pub wired: &'a str,
  pub usb_dac: &'a str,
  pub hdmi: &'a str,
  pub bluetooth: &'a str,
  pub card: &'a str,
}

/// A Bluetooth codec as its makers spell it: `ldac` is `LDAC`, `aptx_hd`
/// `aptX HD`, `sbc_xq` `SBC-XQ`. Unknown ones are upper-cased.
pub fn bluetooth_codec_name(codec: &str) -> String {
  match codec {
    "sbc" => "SBC".into(),
    "sbc_xq" => "SBC-XQ".into(),
    "aac" | "aac_eld" => "AAC".into(),
    "aptx" => "aptX".into(),
    "aptx_hd" => "aptX HD".into(),
    "aptx_ll" | "aptx_ll_duplex" => "aptX LL".into(),
    "ldac" => "LDAC".into(),
    "lc3" | "lc3plus_h3" => "LC3".into(),
    "msbc" => "mSBC".into(),
    "cvsd" => "CVSD".into(),
    other if other.starts_with("opus") => "Opus".into(),
    other => other.to_uppercase(),
  }
}

impl OutputDevice {
  /// `Bluetooth: LDAC · 96 kHz`, `USB DAC: 24-bit · 96 kHz`, `Speakers:
  /// 24-bit · 48 kHz`; with the device name added when `named`.
  pub(crate) fn label(&self, words: &DeviceWords<'_>, named: bool) -> String {
    let (kind, detail) = match &self.kind {
      DeviceKind::Bluetooth { codec } => {
        let mut parts: Vec<String> = codec.iter().map(|c| bluetooth_codec_name(c)).collect();
        if let Some(format) = &self.format {
          parts.push(format!("{} kHz", khz(format.sample_rate)));
        }
        (words.bluetooth, parts.join(" \u{b7} "))
      }
      other => {
        let word = match other {
          DeviceKind::Speakers => words.speakers,
          DeviceKind::Wired => words.wired,
          DeviceKind::UsbDac => words.usb_dac,
          DeviceKind::Hdmi => words.hdmi,
          DeviceKind::Bluetooth { .. } | DeviceKind::Card => words.card,
        };
        (
          word,
          self
            .format
            .as_ref()
            .map(HardwareFormat::label)
            .unwrap_or_default(),
        )
      }
    };
    let mut label = if detail.is_empty() {
      kind.to_string()
    } else {
      format!("{kind}: {detail}")
    };
    if named {
      if let Some(name) = self.name.as_deref().filter(|n| !n.is_empty()) {
        label = format!("{label} \u{b7} {name}");
      }
    }
    label
  }
}

impl OutputDevice {
  /// The label with the device name, then without, for the widest that fits.
  pub fn labels(&self, words: &DeviceWords<'_>) -> Vec<String> {
    let mut all = vec![self.label(words, true), self.label(words, false)];
    all.dedup();
    all
  }
}

impl OutputFormat {
  /// [`OutputFormat::candidates`] without the device name, which on PipeWire
  /// is only the sound server's.
  pub fn candidates_without_device(&self) -> Vec<String> {
    let all = self.candidates();
    let skip = usize::from(all.len() > 2);
    all.into_iter().skip(skip).collect()
  }
}

/// The output line from the most to the least detailed: what qbal hands the
/// sound system, then the device the sound reaches and its format. The device
/// part outlives the app part as the width shrinks, because it is the one that
/// says what the listener actually hears.
pub fn output_candidates(
  output: Option<&OutputFormat>,
  device: Option<&OutputDevice>,
  words: &DeviceWords<'_>,
) -> Vec<String> {
  let app = output.map(OutputFormat::candidates).unwrap_or_default();
  let Some(device) = device else {
    return app;
  };
  let named = device.label(words, true);
  let plain = device.label(words, false);
  let mut all: Vec<String> = Vec::new();
  // The app part drops its own device name first: on PipeWire it is only the
  // sound server's ("default", "PipeWire"), while the device part names the
  // real output.
  let app_without_device: Vec<String> = output
    .map(OutputFormat::candidates_without_device)
    .unwrap_or_default();
  for app in &app_without_device {
    all.push(format!("{app} \u{2192} {named}"));
  }
  for app in &app_without_device {
    all.push(format!("{app} \u{2192} {plain}"));
  }
  all.push(named);
  all.push(plain);
  all.dedup();
  all
}

/// The first of `candidates` (widest first) that fits in `width` columns.
pub fn widest_fitting(candidates: &[String], width: usize) -> Option<&str> {
  use unicode_width::UnicodeWidthStr;
  candidates
    .iter()
    .map(String::as_str)
    .find(|c| UnicodeWidthStr::width(*c) <= width)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn output(device: Option<&str>) -> OutputFormat {
    OutputFormat {
      sample_rate: 48_000,
      channels: 2,
      bits: 32,
      float: true,
      device: device.map(str::to_string),
    }
  }

  #[test]
  fn khz_drops_the_fraction_of_round_rates() {
    assert_eq!(khz(96_000), "96");
    assert_eq!(khz(44_100), "44.1");
    assert_eq!(khz(88_200), "88.2");
    assert_eq!(khz(192_000), "192");
  }

  #[test]
  fn the_output_label_names_sample_type_rate_channels_and_device() {
    let all = output(Some("Built-in Audio")).candidates();
    assert_eq!(
      all,
      vec![
        "32-bit float \u{b7} 48 kHz \u{b7} 2 ch \u{b7} Built-in Audio".to_string(),
        "32-bit float \u{b7} 48 kHz \u{b7} 2 ch".to_string(),
        "48 kHz".to_string(),
      ]
    );
  }

  #[test]
  fn an_integer_device_has_no_float_suffix_and_a_missing_device_no_name() {
    let format = OutputFormat {
      bits: 16,
      float: false,
      ..output(None)
    };
    let all = format.candidates();
    assert_eq!(all[0], "16-bit \u{b7} 48 kHz \u{b7} 2 ch");
    assert_eq!(all.len(), 2, "no device name, no device candidate");
  }

  const HW_PARAMS: &str = "access: MMAP_INTERLEAVED\nformat: S24_3LE\nsubformat: STD\n\
    channels: 2\nrate: 96000 (96000/1)\nperiod_size: 1024\nbuffer_size: 4096\n";

  #[test]
  fn a_running_card_reports_its_sample_format_rate_and_channels() {
    let hw = parse_hw_params(HW_PARAMS, Some("DAC")).unwrap();
    assert_eq!(hw.format, "S24_3LE");
    assert_eq!((hw.bits, hw.float), (24, false));
    assert_eq!(hw.sample_rate, 96_000);
    assert_eq!(hw.channels, 2);
    assert_eq!(hw.card.as_deref(), Some("DAC"));
    assert_eq!(hw.label(), "24-bit \u{b7} 96 kHz");
  }

  #[test]
  fn a_closed_card_or_an_unknown_format_reports_nothing() {
    assert_eq!(parse_hw_params("closed\n", None), None);
    let dsd = HW_PARAMS.replace("S24_3LE", "DSD_U32_BE");
    assert_eq!(parse_hw_params(&dsd, None), None);
  }

  #[test]
  fn alsa_format_names_map_to_sample_widths() {
    assert_eq!(alsa_bits("S16_LE"), Some((16, false)));
    assert_eq!(alsa_bits("S32_LE"), Some((32, false)));
    assert_eq!(alsa_bits("U8"), Some((8, false)));
    assert_eq!(alsa_bits("FLOAT_LE"), Some((32, true)));
    assert_eq!(alsa_bits("FLOAT64_LE"), Some((64, true)));
    assert_eq!(alsa_bits("IEC958_SUBFRAME_LE"), None);
  }

  const WORDS: DeviceWords<'static> = DeviceWords {
    speakers: "Speakers",
    wired: "Wired",
    usb_dac: "USB DAC",
    hdmi: "HDMI",
    bluetooth: "Bluetooth",
    card: "Card",
  };

  fn device(kind: DeviceKind, name: Option<&str>) -> OutputDevice {
    OutputDevice {
      kind,
      name: name.map(str::to_string),
      format: parse_hw_params(HW_PARAMS, None),
    }
  }

  #[test]
  fn each_output_kind_names_itself_with_its_format() {
    let label = |kind| device(kind, None).label(&WORDS, false);
    assert_eq!(
      label(DeviceKind::Speakers),
      "Speakers: 24-bit \u{b7} 96 kHz"
    );
    assert_eq!(label(DeviceKind::Wired), "Wired: 24-bit \u{b7} 96 kHz");
    assert_eq!(label(DeviceKind::UsbDac), "USB DAC: 24-bit \u{b7} 96 kHz");
    assert_eq!(label(DeviceKind::Hdmi), "HDMI: 24-bit \u{b7} 96 kHz");
    assert_eq!(label(DeviceKind::Card), "Card: 24-bit \u{b7} 96 kHz");
    assert_eq!(
      label(DeviceKind::Bluetooth {
        codec: Some("ldac".into())
      }),
      "Bluetooth: LDAC \u{b7} 96 kHz",
      "Bluetooth shows the codec and the rate, not the PCM fed to the encoder"
    );
    let bare = OutputDevice {
      kind: DeviceKind::Bluetooth { codec: None },
      name: None,
      format: None,
    };
    assert_eq!(bare.label(&WORDS, true), "Bluetooth");
  }

  #[test]
  fn bluetooth_codecs_are_spelled_as_their_makers_do() {
    assert_eq!(bluetooth_codec_name("aptx_hd"), "aptX HD");
    assert_eq!(bluetooth_codec_name("sbc_xq"), "SBC-XQ");
    assert_eq!(bluetooth_codec_name("aac"), "AAC");
    assert_eq!(bluetooth_codec_name("opus_05"), "Opus");
    assert_eq!(bluetooth_codec_name("newcodec"), "NEWCODEC");
  }

  #[test]
  fn the_device_follows_the_app_output_and_survives_narrowing() {
    let usb = device(DeviceKind::UsbDac, Some("FiiO KA3"));
    let all = output_candidates(Some(&output(Some("PipeWire"))), Some(&usb), &WORDS);
    assert_eq!(
      all,
      vec![
        "32-bit float \u{b7} 48 kHz \u{b7} 2 ch \u{2192} USB DAC: 24-bit \u{b7} 96 kHz \u{b7} FiiO KA3",
        "48 kHz \u{2192} USB DAC: 24-bit \u{b7} 96 kHz \u{b7} FiiO KA3",
        "32-bit float \u{b7} 48 kHz \u{b7} 2 ch \u{2192} USB DAC: 24-bit \u{b7} 96 kHz",
        "48 kHz \u{2192} USB DAC: 24-bit \u{b7} 96 kHz",
        "USB DAC: 24-bit \u{b7} 96 kHz \u{b7} FiiO KA3",
        "USB DAC: 24-bit \u{b7} 96 kHz",
      ]
    );
    assert_eq!(
      output_candidates(Some(&output(None)), None, &WORDS),
      output(None).candidates()
    );
  }

  #[test]
  fn pipewire_format_names_map_to_sample_widths_too() {
    assert_eq!(alsa_bits("S16LE"), Some((16, false)));
    assert_eq!(alsa_bits("S24_32LE"), Some((24, false)));
    assert_eq!(alsa_bits("F32LE"), Some((32, true)));
    assert_eq!(alsa_bits("F32P"), Some((32, true)));
  }

  #[test]
  fn the_widest_candidate_that_fits_wins() {
    let all = vec!["long label".to_string(), "short".to_string()];
    assert_eq!(widest_fitting(&all, 20), Some("long label"));
    assert_eq!(widest_fitting(&all, 7), Some("short"));
    assert_eq!(widest_fitting(&all, 3), None);
  }
}
