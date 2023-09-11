use std::sync::{atomic::Ordering::SeqCst, mpsc::Sender, Arc};

use dialog_unwrapper::rfd::{AsyncMessageDialog, MessageLevel};
use libui::{controls::*, layout, menu, prelude::*};

use crate::misc::{app_meta, prelude::*};

mod texts {
    pub(super) const SET_SILENT_DATA: &str = "無音データを設定する";
}

/// 音階モニタの更新を行う。
pub fn update_note_monitor<const N: usize>(labels: &mut [Label; N], notes: [crate::sys::Note; N]) {
    for (i, note) in notes.into_iter().enumerate() {
        labels[i].set_text(&format!("{}: {}", i + 1, note.get_name()))
    }
}

pub fn make_ui<const NUMBER_OF_NOTE_IN_RESULT: usize>(
    event_sender: Sender<crate::Event>,
    config: Arc<crate::sys::Config>,
    midi_port_names: impl Iterator<Item = String>,
) -> (UI, Window, [Label; NUMBER_OF_NOTE_IN_RESULT]) {
    /* UIの準備 */
    let ui = UI::init()
        .context("UIの初期化に失敗しました。")
        .unwrap_or_dialog_with_title(errors::INIT_ERROR);

    ui.on_should_quit({
        let ui = ui.clone();
        move || ui.quit()
    });

    // レイアウトの作成
    layout! { &ui,
        let layout = HorizontalBox(padded: true) {
            Compact: let notes_group = Group("Notes", margined: true) {
                let notes_box = HorizontalBox(padded: false) {
                    Compact: let result_label_box = VerticalBox(padded: false) {}
                    Compact: let spacer = Spacer()
                }
            }
            Compact: let wrapped_control_box = VerticalBox(padded: true) {
                Stretchy: let top_spacer = Spacer()
                Compact: let control_box = HorizontalBox(padded: true) {
                    Stretchy: let first_control_box = VerticalBox(padded: true) {
                        Compact: let window_check_box = Checkbox("窓関数（ハン窓）を使う", checked: false)
                        Compact: let min_detection_volume_label = Label("検出対象とする最低音量")
                        Compact: let min_detection_volume_spin_box = Spinbox(0, 100)
                        Compact: let pitch_control_label = Label("音階調節")
                        Compact: let pitch_control_spin_box = Spinbox(-127, 127)
                    }
                    Stretchy: let second_control_box = VerticalBox(padded: true) {
                        Compact: let silent_data_button = Button(texts::SET_SILENT_DATA)
                        Compact: let point_length_size_label = Label("ポイント数の規模")
                        Compact: let point_length_size_spin_box = Spinbox(1, u16::MAX as _)
                        Compact: let midi_output_label = Label("MIDIの出力先")
                        Compact: let midi_output_combo_box = Combobox() {}
                    }
                }
                Compact: let bottom_spacer = Spacer()
            }
        }
    }

    /* ここからControlの設定 */

    // 結果表示用のラベルの準備
    let mut count = 0;
    let note_labels = [(); NUMBER_OF_NOTE_IN_RESULT].map(move |_| {
        count += 1;
        let label = Label::new(&format!("{count}: 　　　　　　　"));
        result_label_box.append(label.clone(), LayoutStrategy::Stretchy);
        label
    });

    // - 一列目

    // 窓関数
    window_check_box.on_toggled(&ui, {
        let config = Arc::clone(&config);
        move |value| config.use_window_flag.store(value, SeqCst)
    });

    // 最低音量
    min_detection_volume_spin_box.set_value(62);
    min_detection_volume_spin_box.on_changed({
        let config = Arc::clone(&config);
        move |value| {
            config
                .min_volume
                .store(((value as f32 / 100. - 1.) * 80.) as _, SeqCst)
        }
    });

    // 音階調節
    pitch_control_spin_box.set_value(0);
    pitch_control_spin_box.on_changed({
        let config = Arc::clone(&config);
        move |value| config.adjustment_rate.store(value, SeqCst)
    });

    // - 二列目

    // 無音データ
    silent_data_button.on_clicked({
        let config = Arc::clone(&config);
        move |button| {
            if &button.text() == texts::SET_SILENT_DATA {
                config.use_silent.store(true, SeqCst);
                button.set_text("無音データを忘れる");
            } else {
                config.use_silent.store(false, SeqCst);
                button.set_text(texts::SET_SILENT_DATA);
            }
        }
    });

    // ポイント数
    point_length_size_spin_box.set_value(9);
    point_length_size_spin_box.on_changed({
        let config = Arc::clone(&config);
        move |value| config.point_times.store(value as _, SeqCst)
    });

    // MIDIの出力先
    midi_output_combo_box.append("なし");
    for port_name in midi_port_names {
        midi_output_combo_box.append(&port_name);
    }
    midi_output_combo_box.set_selected(0);

    if midi_output_combo_box.count() == 0 {
        // もし一つもMIDIの出力先が見つからなかったのなら、そもそも使えないようにする。
        midi_output_combo_box.disable();
    };

    midi_output_combo_box
        .clone()
        .on_selected(&ui, move |index| {
            let index = index as usize;
            if index > midi_output_combo_box.count() as _ {
                let _ = AsyncMessageDialog::new()
                    .set_title(crate::APPLICATION_NAME)
                    .set_description("そのMIDIの出力先が見つかりませんでした。")
                    .set_level(MessageLevel::Error)
                    .show();
            } else {
                let _ = event_sender.send(crate::Event::UpdateMidiOutput(index as _));
            }
        });

    /* ここからウィンドウ自体に関する設定 */

    // メニューを作る。
    menu! { &ui,
        let file_menu = Menu("ファイル") {
            let quit_menu_item = MenuItem("終了")
        }
        let help_menu = Menu("ヘルプ") {
            let about_menu_item = MenuItem("このアプリについて")
        }
    }

    quit_menu_item.on_clicked({
        let ui = ui.clone();
        move |_, _| ui.quit()
    });
    about_menu_item.on_clicked(|_, _| app_meta::show_about());

    // ウィンドウを作る。
    let mut window = Window::new(
        &ui,
        crate::APPLICATION_NAME,
        300,
        200,
        WindowType::HasMenubar,
    );
    window.set_child(layout);

    (ui, window, note_labels)
}
