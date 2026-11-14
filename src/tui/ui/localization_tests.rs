//! Rendering regressions shared by the sidebar/help and main-screen migrations.

use super::help::padded_help_cell;
use unicode_width::UnicodeWidthStr;

#[test]
fn help_cell_truncation_preserves_english_and_counts_chinese_display_columns() {
  for width in 0..48 {
    for english in ["", "Description", "<Ctrl+f>", "Increase volume by 10%"] {
      let old = if width == 0 {
        String::new()
      } else if english.chars().count() > width {
        format!("{}…", english.chars().take(width - 1).collect::<String>())
      } else {
        english.to_string()
      };
      assert_eq!(padded_help_cell(english, width), format!("{old:<width$}"));
    }
    for chinese in ["随机播放", "循環播放", "播放清單 Mix", "音量增加 10%"] {
      assert_eq!(padded_help_cell(chinese, width).width(), width);
    }
  }
  assert_eq!(padded_help_cell("音量增加", 1), "…");
  assert_eq!(padded_help_cell("音量增加", 2), "… ");
  assert_eq!(padded_help_cell("音量增加", 4), "音… ");
}
