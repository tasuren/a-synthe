#![cfg_attr(not(test), windows_subsystem = "windows")]
#![cfg_attr(test, windows_subsystem = "console")]

use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use midir::MidiOutput;

use libui::{prelude::*, layout};

mod sys;
mod misc;

const APPLICATION_NAME: &str = "aSynthe";

/// メインプログラムです。
fn main() {
    println!("{} by tasuren\nNow loading...", APPLICATION_NAME);

    // 別スレッドとの通信用のチャンネルを作る。
    let (tx, rx) = std::sync::mpsc::channel();

    // MIDIの用意をする。
    let output = MidiOutput::new(APPLICATION_NAME).unwrap();

    // マイクの設定を行う。
    let device = default_host()
        .default_input_device()
        .expect("有効なデバイスがありません。");
    let config = device
        .default_input_config()
        .expect("有効なデバイスの設定がありません。");
    let mut synthe = Synthe::new(
        Notes::new(format!("{}src/notes.csv", get_base())),
        tx,
        config.sample_rate().0 as f32,
    );
    let shared_data = synthe.shared_data.clone();
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| synthe.process(data),
            |e| println!("Error: {}", e),
            None,
        )
        .unwrap();
    stream.play().unwrap();

    let ui = UI::init().expect("Couldn't initialize UI library.");
    let is_closed = Rc::new(Cell::new(false));

    // メニューを作る。
    let menu = Menu::new("メニュー");
    let info_item = menu.append_item("情報");
    info_item.on_clicked(|_, _| {
        MessageDialog::new()
            .set_type(MessageType::Info)
            .set_title("情報")
            .set_text(&format!(
            "aSynthe v{}\n(c) 2022 tasuren\n\nリポジトリ：https://github.com/tasuren/aSynthe\n{}",
            env!("CARGO_PKG_VERSION"), "ライセンス情報：https://tasuren.github.io/aSynthe"
        ))
            .show_alert()
            .unwrap()
    });
    menu.append_separator();
    let quit_item = menu.append_item("終了");
    let cloned_is_closed = is_closed.clone();
    quit_item.on_clicked(move |_, _| cloned_is_closed.set(true));

    // ウィンドウを作る。
    let mut window = Window::new(&ui, APPLICATION_NAME, 300, 200, WindowType::NoMenubar);
    let mut hbox = HorizontalBox::new();

    let cloned_is_closed = is_closed.clone();
    window.on_closing(&ui, move |_| cloned_is_closed.set(true));

    // 結果表示用のラベルを作る。
    let mut group = Group::new("Note");
    let mut label_box = VerticalBox::new();
    let mut labels = Vec::new();
    for _ in 0..RESULT_RANGE {
        // ここでスペースを使うのは、音程の文字列の長さの最大までGroupを引き伸ばすため。
        // じゃないと起動時にGroupが伸びることになる。
        labels.push(Label::new("　　　　　　　"));
        label_box.append(labels.last().unwrap().clone(), LayoutStrategy::Stretchy);
    }
    label_box.append(Label::new("　　　　　　　"), LayoutStrategy::Stretchy);
    group.set_child(label_box);

    hbox.append(group, LayoutStrategy::Compact);
    hbox.append(Label::new("    "), LayoutStrategy::Stretchy);

    // 設定用のボタン等を作る。
    let mut vbox = VerticalBox::new();
    vbox.append(Label::new("　"), LayoutStrategy::Compact);

    let mut row_hbox = HorizontalBox::new();

    // 窓関数を使うかどうかのチェックボックス
    let mut use_window_check = Checkbox::new("窓関数を使う");
    use_window_check.set_checked(false);
    let cloned_shared_data = shared_data.clone();
    use_window_check.on_toggled(&ui, move |value| {
        cloned_shared_data
            .use_window_flag
            .store(value, Ordering::SeqCst)
    });
    row_hbox.append(use_window_check, LayoutStrategy::Compact);
    row_hbox.append(Label::new("　"), LayoutStrategy::Stretchy);

    // 無音データを設定するボタン
    let mut silent_button = Button::new(DEFAULT_SILENT_BUTTON_TEXT);
    let cloned_shared_data = shared_data.clone();
    silent_button.on_clicked(move |_button| {
        if &_button.text() == DEFAULT_SILENT_BUTTON_TEXT {
            cloned_shared_data.use_silent.store(true, Ordering::SeqCst);
            _button.set_text("無音データを消す");
        } else {
            cloned_shared_data.use_silent.store(false, Ordering::SeqCst);
            _button.set_text(DEFAULT_SILENT_BUTTON_TEXT);
        }
    });
    row_hbox.append(silent_button, LayoutStrategy::Compact);

    vbox.append(row_hbox, LayoutStrategy::Compact);

    // 最低音量入力ボックス
    let mut min_volume_entry = Spinbox::new(0, 100);
    min_volume_entry.set_value(62);
    let cloned_shared_data = shared_data.clone();
    min_volume_entry.on_changed(move |value| {
        cloned_shared_data
            .min_volume
            .store((value as f32 / 100.0 * 80.0 - 80.0) as _, Ordering::SeqCst)
    });
    vbox.append(
        Label::new("検出対象になる最低音量"),
        LayoutStrategy::Compact,
    );
    vbox.append(min_volume_entry, LayoutStrategy::Compact);

    // ポイント数
    let mut point_times = Spinbox::new(1, u16::MAX as _);
    point_times.set_value(9);
    let cloned_shared_data = shared_data.clone();
    point_times.on_changed(move |value| {
        cloned_shared_data
            .point_times
            .store(value as u16, Ordering::SeqCst)
    });
    vbox.append(
        Label::new("ポイント数をデータの長さの何倍にするか"),
        LayoutStrategy::Compact,
    );
    vbox.append(point_times, LayoutStrategy::Compact);

    let mut row_hbox = HorizontalBox::new();

    // 調整
    let mut adjustment_rate_box = VerticalBox::new();
    adjustment_rate_box.append(Label::new("音程調整"), LayoutStrategy::Compact);
    let mut adjustment_rate = Spinbox::new(-127, 127);
    adjustment_rate.set_value(0);
    let cloned_shared_data = shared_data.clone();
    adjustment_rate.on_changed(move |value| {
        cloned_shared_data
            .adjustment_rate
            .store(value, Ordering::SeqCst)
    });
    adjustment_rate_box.append(adjustment_rate, LayoutStrategy::Compact);
    row_hbox.append(adjustment_rate_box, LayoutStrategy::Compact);
    row_hbox.append(Label::new("　"), LayoutStrategy::Stretchy);

    // MIDIの出力先の選択ボックス
    let mut midi_output_select_box = VerticalBox::new();
    midi_output_select_box.append(Label::new("MIDI出力先"), LayoutStrategy::Compact);
    let mut midi_output_select = Combobox::new();
    midi_output_select.append("なし");
    let port_count = output.ports().len();
    // MIDIの出力先をコンボボックスに追加しておく。
    for port in output.ports().iter() {
        midi_output_select.append(&output.port_name(port).unwrap_or("不明な出力先".to_string()));
    }
    // MidiManagerを用意する。
    let mut midi_manager = MidiManager::new(output);
    // MIDI出力先選択の設定を行う。
    if port_count == 0 {
        midi_output_select_box.disable();
    } else {
        let cloned_port_index = midi_manager.port_index.clone();
        midi_output_select.on_selected(&ui, move |index| {
            let index = index as usize;
            if index > port_count {
                MessageDialog::new()
                    .set_title(APPLICATION_NAME)
                    .set_text("そのMIDIの出力先が見つかりませんでした。")
                    .set_type(MessageType::Error)
                    .show_alert()
                    .unwrap();
            } else {
                cloned_port_index.set(index);
            };
        });
    };
    midi_output_select.set_selected(0);
    midi_output_select_box.append(midi_output_select, LayoutStrategy::Compact);
    row_hbox.append(midi_output_select_box, LayoutStrategy::Compact);
    let mut before_midi_number = Some(0);

    vbox.append(row_hbox, LayoutStrategy::Compact);

    // 作ったボタン等をまとめる。
    hbox.append(vbox, LayoutStrategy::Compact);
    window.set_child(hbox);

    // ウィンドウを動かす。
    window.show();
    let mut event_loop = ui.event_loop();
    event_loop.on_tick(move || {});

    let duration = Duration::from_secs_f32(0.05);

    // ウィンドウが閉じられるまではイベントループを動かし続ける。
    while !is_closed.get() {
        event_loop.next_tick();

        // マイク入力を処理するスレッドから送られてくる音程情報を処理する。
        if let Ok(note) = rx.recv_timeout(duration) {
            midi_manager = midi_manager.set_midi_output();
            if let Some(note) = note {
                labels[note.0].set_text(&format!(
                    "{}: {}",
                    note.0 + 1,
                    Notes::get_name(note.1 as usize)
                ));

                // MIDI出力が有効な場合は、出力を行う。
                if note.0 == 0 && midi_manager.is_avaliable() {
                    if let Some(before_number) = before_midi_number {
                        if before_number == note.1 {
                            continue;
                        };
                        midi_manager.send_data(before_number, false);
                    };
                    midi_manager.send_data(note.1, true);
                    before_midi_number = Some(note.1);
                } else {
                    continue;
                };
            } else if let Some(before_number) = before_midi_number {
                if midi_manager.is_avaliable() {
                    // 何も音が鳴っていないのなら前ならした音を無効にする。
                    midi_manager.send_data(before_number, false);
                    before_midi_number = None;
                };
            };
        };
    }
}
