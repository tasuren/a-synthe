#![cfg_attr(not(test), windows_subsystem = "windows")]
#![cfg_attr(test, windows_subsystem = "console")]

use std::{
    time::Duration, fs::File, io::{ BufRead, BufReader }, sync::{
        Arc, atomic::{ AtomicU16, AtomicBool, Ordering, AtomicI32 },
        mpsc::{ Sender, channel }
    }, rc::Rc, cell::Cell,
};

use midir::{ MidiOutput, MidiOutputConnection };
use cpal::{
    traits::{ HostTrait, DeviceTrait, StreamTrait },
    default_host
};

use libui::{
    menus::Menu, controls::{
        Button, Label, HorizontalBox, LayoutStrategy, Group,
        Spinbox, Checkbox, VerticalBox, Combobox
    }, prelude::*
};
use native_dialog::{ MessageDialog, MessageType };

mod sys;
use sys::{ core::process_fft, app_meta::get_base };


const APPLICATION_NAME: &str = "aSynthe";
const RESULT_RANGE: usize = 5;
const NOTE_NAMES: [&str; 12] = [
    "ド/C", "ド♯/C♯", "レ/D", "レ♯/D♯", "ミ/E", "ファ/F",
    "ファ♯/F♯", "ソ/G", "ソ♯/G♯", "ラ/A", "ラ♯/A♯", "シ/B"
];


/// 音程と音程に対応する周波数を格納するための構造体です。
#[derive(Default)]
struct Notes {
    numbers: Vec<u8>,
    frequencies: Vec<f32>,
    before_frequencies: Vec<f32>,
    after_frequencies: Vec<f32>
}

impl Notes {
    /// 音程等が書き込まれたファイルから音程等を読み込みます。
    fn new(path: String) -> Self {
        let mut notes = Self::default();
        let (mut line, mut raw);
        for tentative in BufReader::new(File::open(path).unwrap()).lines() {
            raw = tentative.unwrap(); line = raw.split_whitespace();
            notes.numbers.push(line.next().unwrap().parse().unwrap());
            notes.frequencies.push(line.next().unwrap().parse().unwrap());
            notes.before_frequencies.push(line.next().unwrap().parse().unwrap());
            notes.after_frequencies.push(line.next().unwrap().parse().unwrap());
        };
        notes
    }

    /// 音程の名前を取得します。
    fn get_name(number: usize) -> String {
        format!("{} {}", NOTE_NAMES[number - 12 * (number / 12)], (number / 12) as isize - 1)
    }
}


/// 音程情報を入れるための構造体です。
struct Note(usize, u8);


/// スレッド間で共有する値を入れるための構造体です。
struct SharedData {
    min_volume: AtomicI32,
    point_times: AtomicU16,
    use_window_flag:AtomicBool,
    use_silent: AtomicBool,
    adjustment_rate: AtomicI32
}


/// 音程を検出するためのものを実装した構造体です。
struct Synthe {
    notes: Notes,
    tx: Sender<Option<Note>>,
    frame_rate: f32,
    silence: Option<Vec<f32>>,
    shared_data: Arc<SharedData>
}

impl Synthe {
    /// インスタンスを作ります。
    fn new(notes: Notes, tx: Sender<Option<Note>>, frame_rate: f32) -> Self {
        Self {
            notes: notes, tx: tx, frame_rate: frame_rate, silence: None,
            shared_data: Arc::new(SharedData {
                min_volume: AtomicI32::new(-30),
                point_times: AtomicU16::new(8),
                use_window_flag: AtomicBool::new(false),
                use_silent: AtomicBool::new(false),
                adjustment_rate: AtomicI32::new(0)
            })
        }
    }

    /// 音程検出の処理を行います。
    fn process(&mut self, data: &[f32]) {
        // 音量を調べる。
        let volume = 20.0 * (data.iter().map(
            |x| x.powi(2)).sum::<f32>() / data.len() as f32
        ).sqrt().log10();

        if volume as i32 > self.shared_data.min_volume.load(Ordering::SeqCst) {
            // FFTで周波数の計算をする。
            let (frequency_resolution, mut data) = process_fft(
                data.to_vec(), self.shared_data.point_times.load(Ordering::SeqCst) as usize,
                self.frame_rate, self.shared_data.use_window_flag.load(Ordering::SeqCst)
            );

            // 無音時のサンプルデータを設定するように言われているのなら設定をする。
            if self.shared_data.use_silent.load(Ordering::SeqCst) {
                if let Some(silence) = &self.silence {
                    // 無音時のデータがあるのなら、無音データを消す。
                    for (index, value) in silence.iter().enumerate() {
                        if data[index] > *value {
                            data[index] -= value;
                        } else { data[index] = 0.0; };
                    };
                } else {
                    self.silence = Some(data);
                    self.shared_data.use_silent.store(false, Ordering::SeqCst);
                    return;
                };
            } else if self.silence.is_some() { self.silence = None; };

            // 一番音量が高い周波数の音程を探す。
            let mut values = Vec::new();
            let (mut stack, mut value);
            for (number, before_frequency, after_frequency) in self.notes.numbers.iter().zip(
                self.notes.before_frequencies.iter().zip(self.notes.after_frequencies.iter())
            ).map(|(number, (bf, af))| (*number, *bf, *af)) {
                stack = &data[(before_frequency / frequency_resolution) as usize
                    ..(after_frequency / frequency_resolution) as usize];
                value = stack.iter().sum::<f32>() / stack.len() as f32;
                if !value.is_nan() { values.push((number, value)); };
            };
            values.sort_by(|(_, x), (_, y)| x.partial_cmp(y).unwrap());

            // ウィンドウを動かしているメインスレッドに検出した音程を送信する。
            let adjustment_rate = self.shared_data.adjustment_rate.load(Ordering::SeqCst);
            let mut value;
            for i in 0..RESULT_RANGE {
                value = values.pop().unwrap().0 as i32 + adjustment_rate;
                if value < 0 { value = 0; };
                if value > 127 { value = 127; };
                let _ = self.tx.send(Some(Note(i, value as u8)));
            };
        } else { let _ = self.tx.send(None); };
    }
}


const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;
const VELOCITY: u8 = 0x64;
const DEFAULT_SILENT_BUTTON_TEXT: &str = "無音データを設定する";


/// MIDIを管理するための構造体です。
struct MidiManager {
    connection: Option<MidiOutputConnection>,
    port_index: Rc<Cell<usize>>,
    real_port_index: usize
}

impl MidiManager {
    /// インスタンスを作ります。
    fn new(output: MidiOutput) -> Self {
        Self {
            connection: if output.ports().len() > 0 {
                let port = &output.ports()[0];
                Some(output.connect(port, APPLICATION_NAME).unwrap())
            } else { None },
            port_index: Rc::new(Cell::new(0)),
            real_port_index: 0
        }
    }

    /// MIDIのデータを送ります。
    fn send_data(&mut self, key: u8, is_on: bool) {
        self.connection.as_mut().unwrap().send(&[
            if is_on { NOTE_ON_MSG }
            else { NOTE_OFF_MSG },
            key, VELOCITY
        ]).unwrap();
    }

    /// MIDIの出力先の処理を行います。
    fn set_midi_output(mut self) -> Self {
        let port_index = self.port_index.get();
        if self.real_port_index != port_index && port_index > 0 {
            if let Some(connection) = self.connection {
                let output = connection.close();
                let port = &output.ports()[port_index - 1];
                self.connection = Some(output.connect(port, APPLICATION_NAME).unwrap());
                self.real_port_index = port_index;
            };
        };
        self
    }

    /// MIDIが使用可能かどうかを調べます。
    fn is_avaliable(&self) -> bool { self.connection.is_some() && self.port_index.get() > 0 }
}


/// メインプログラムです。
fn main() {
    println!("{} by tasuren\nNow loading...", APPLICATION_NAME);

    // 別スレッドとの通信用のチャンネルを作る。
    let (tx, rx) = channel();

    // MIDIの用意をする。
    let output = MidiOutput::new(APPLICATION_NAME).unwrap();

    // マイクの設定を行う。
    let device = default_host().default_input_device().expect("No device is avaliable.");
    let config = device.default_input_config().expect("No device config is avaliable.");
    let mut synthe = Synthe::new(
        Notes::new(format!("{}src/notes.csv", get_base())),
        tx, config.sample_rate().0 as f32
    );
    let shared_data = synthe.shared_data.clone();
    let stream = device.build_input_stream(
        &config.into(), move |data: &[f32], _| synthe.process(data),
        |e| println!("Error: {}", e), None
    ).unwrap();
    stream.play().unwrap();

    let ui = UI::init().expect("Couldn't initialize UI library.");
    let is_closed = Rc::new(Cell::new(false));

    // メニューを作る。
    let menu = Menu::new("メニュー");
    let info_item = menu.append_item("情報");
    info_item.on_clicked(|_, _| MessageDialog::new()
        .set_type(MessageType::Info)
        .set_title("情報")
        .set_text(&format!(
            "aSynthe v{}\n(c) 2022 tasuren\n\nリポジトリ：https://github.com/tasuren/aSynthe\n{}",
            env!("CARGO_PKG_VERSION"), "ライセンス情報：https://tasuren.github.io/aSynthe"
        ))
        .show_alert().unwrap());
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
    };
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
    use_window_check.on_toggled(&ui, move |value|
        cloned_shared_data.use_window_flag.store(value, Ordering::SeqCst));
    row_hbox.append(use_window_check, LayoutStrategy::Compact);
    row_hbox.append(Label::new("　"), LayoutStrategy::Stretchy);

    // 無音データを設定するボタン
    let mut silent_button = Button::new(DEFAULT_SILENT_BUTTON_TEXT);
    let cloned_shared_data = shared_data.clone();
    silent_button.on_clicked(move |_button|
        if &_button.text() == DEFAULT_SILENT_BUTTON_TEXT {
            cloned_shared_data.use_silent.store(true, Ordering::SeqCst);
            _button.set_text("無音データを消す");
        } else {
            cloned_shared_data.use_silent.store(false, Ordering::SeqCst);
            _button.set_text(DEFAULT_SILENT_BUTTON_TEXT);
        }
    );
    row_hbox.append(silent_button, LayoutStrategy::Compact);

    vbox.append(row_hbox, LayoutStrategy::Compact);

    // 最低音量入力ボックス
    let mut min_volume_entry = Spinbox::new(0, 100);
    min_volume_entry.set_value(62);
    let cloned_shared_data = shared_data.clone();
    min_volume_entry.on_changed(move |value|
        cloned_shared_data.min_volume.store(
            (value as f32 / 100.0 * 80.0 - 80.0) as _,
            Ordering::SeqCst
        ));
    vbox.append(Label::new("検出対象になる最低音量"), LayoutStrategy::Compact);
    vbox.append(min_volume_entry, LayoutStrategy::Compact);

    // ポイント数
    let mut point_times = Spinbox::new(1, u16::MAX as _);
    point_times.set_value(9);
    let cloned_shared_data = shared_data.clone();
    point_times.on_changed(move |value|
        cloned_shared_data.point_times.store(
            value as u16, Ordering::SeqCst
        ));
    vbox.append(Label::new("ポイント数をデータの長さの何倍にするか"), LayoutStrategy::Compact);
    vbox.append(point_times, LayoutStrategy::Compact);

    let mut row_hbox = HorizontalBox::new();

    // 調整
    let mut adjustment_rate_box = VerticalBox::new();
    adjustment_rate_box.append(Label::new("音程調整"), LayoutStrategy::Compact);
    let mut adjustment_rate = Spinbox::new(-127, 127);
    adjustment_rate.set_value(0);
    let cloned_shared_data = shared_data.clone();
    adjustment_rate.on_changed(move |value|
        cloned_shared_data.adjustment_rate.store(
            value, Ordering::SeqCst
        ));
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
        midi_output_select.append(&output.port_name(port)
            .unwrap_or("不明な出力先".to_string()));
    };
    // MidiManagerを用意する。
    let mut midi_manager = MidiManager::new(output);
    // MIDI出力先選択の設定を行う。
    if port_count == 0 { midi_output_select_box.disable(); }
    else {
        let cloned_port_index = midi_manager.port_index.clone();
        midi_output_select.on_selected(&ui, move |index| {
            let index = index as usize;
            if index > port_count {
                MessageDialog::new()
                    .set_title(APPLICATION_NAME)
                    .set_text("そのMIDIの出力先が見つかりませんでした。")
                    .set_type(MessageType::Error)
                    .show_alert().unwrap();
            } else { cloned_port_index.set(index); };
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
                    "{}: {}", note.0 + 1, Notes::get_name(note.1 as usize)
                ));

                // MIDI出力が有効な場合は、出力を行う。
                if note.0 == 0 && midi_manager.is_avaliable() {
                    if let Some(before_number) = before_midi_number {
                        if before_number == note.1 { continue; };
                        midi_manager.send_data(before_number, false);
                    };
                    midi_manager.send_data(note.1, true);
                    before_midi_number = Some(note.1);
                } else { continue; };
            } else if let Some(before_number) = before_midi_number {
                if midi_manager.is_avaliable() {
                    // 何も音が鳴っていないのなら前ならした音を無効にする。
                    midi_manager.send_data(before_number, false);
                    before_midi_number = None;
                };
            };
        };
    };
}