//! aSynthe

#![allow(non_snake_case)]

use std::{
    time::Duration, fs::File, io::{ BufRead, BufReader }, sync::{
        Arc, atomic::{ AtomicU16, AtomicBool, Ordering, AtomicI32 },
        mpsc::{ Sender, channel }
    }, rc::Rc, cell::Cell
};

use lazy_static::lazy_static;

use midir::MidiOutput;
use cpal::{
    traits::{ HostTrait, DeviceTrait, StreamTrait },
    default_host
};

use iui::{
    menus::Menu, controls::{
        Button, Label, HorizontalBox, LayoutStrategy, Group,
        Spinbox, Checkbox, VerticalBox, Spacer
    }, prelude::*
};
use native_dialog::{ MessageDialog, MessageType };

mod lib;
use lib::{ process_fft, get_base };


const APPLICATION_NAME: &str = "aSynthe";
const RESULT_RANGE: usize = 5;
lazy_static! {
    static ref NOTE_NAMES: Vec<&'static str> = vec![
        "ド/C", "ド♯/C♯", "レ/D", "レ♯/D♯", "ミ/E", "ファ/F",
        "ファ♯/F♯", "ソ/G", "ソ♯/G♯", "ラ/A", "ラ♯/A♯", "シ/B"
    ];
}


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
#[derive(Clone)]
struct SharedData {
    min_volume: Arc<AtomicI32>,
    point_times: Arc<AtomicU16>,
    use_window_flag: Arc<AtomicBool>,
    use_silent: Arc<AtomicBool>,
    adjustment_rate: Arc<AtomicI32>
}


/// 音程を検出するためのものを実装した構造体です。
struct Synthe {
    notes: Notes,
    tx: Sender<Option<Note>>,
    frame_rate: f32,
    silence: Option<Vec<f32>>,
    shared_data: SharedData
}

impl Synthe {
    fn new(notes: Notes, tx: Sender<Option<Note>>, frame_rate: f32) -> Self {
        Self {
            notes: notes, tx: tx, frame_rate: frame_rate, silence: None,
            shared_data: SharedData {
                min_volume: Arc::new(AtomicI32::new(-30)),
                point_times: Arc::new(AtomicU16::new(8)),
                use_window_flag: Arc::new(AtomicBool::new(false)),
                use_silent: Arc::new(AtomicBool::new(false)),
                adjustment_rate: Arc::new(AtomicI32::new(0))
            }
        }
    }

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
                stack = &data[(before_frequency / frequency_resolution) as usize..(after_frequency / frequency_resolution) as usize];
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


/// メインプログラムです。
fn main() {
    println!("{} by tasuren\nNow loading...", APPLICATION_NAME);

    // 別スレッドとの通信用のチャンネルを作る。
    let (tx, rx) = channel();

    // MIDI出力の設定をする。
    let tentative = MidiOutput::new(APPLICATION_NAME).unwrap();
    let mut output;
    if tentative.port_count() == 0 {
        output = None;
    } else {
        let port = &tentative.ports()[0];
        output = Some(tentative.connect(port, APPLICATION_NAME).unwrap());
    };

    // マイクの設定を行う。
    let device = default_host().default_input_device().expect("No device is avaliable.");
    let config = device.default_input_config().expect("No device config is avaliable.");
    let mut synthe = Synthe::new(
        Notes::new(format!("{}/static/notes.csv", get_base())),
        tx, config.sample_rate().0 as f32
    );
    let shared_data = synthe.shared_data.clone();
    let stream = device.build_input_stream(
        &config.into(), move |data: &[f32], _| synthe.process(data),
        |e| println!("Error: {}", e)
    ).unwrap();
    stream.play().unwrap();

    let ui = UI::init().expect("Couldn't initialize UI library.");
    let is_closed = Rc::new(Cell::new(false));

    // メニューを作る。
    let menu = Menu::new(&ui, "メニュー");
    let info_item = menu.append_item("情報");
    info_item.on_clicked(&ui, |_, _| MessageDialog::new()
        .set_type(MessageType::Info)
        .set_title("情報")
        .set_text(&format!(
            "aSynthe v{}\n(c) 2022 tasuren\n\nリポジトリ：https://github.com/tasuren/aSynthe\n{}",
            env!("CARGO_PKG_VERSION"), "ライセンス情報：https://github.com/tasuren/aSynthe/blob/main/static/licenses.html"
        ))
        .show_alert().unwrap());
    menu.append_separator();
    let quit_item = menu.append_item("終了");
    let cloned_is_closed = is_closed.clone();
    quit_item.on_clicked(&ui, move |_, _| cloned_is_closed.set(true));

    // ウィンドウを作る。
    let mut window = Window::new(&ui, APPLICATION_NAME, 300, 200, WindowType::NoMenubar);
    let mut hbox = HorizontalBox::new(&ui);

    let cloned_is_closed = is_closed.clone();
    window.on_closing(&ui, move |_| cloned_is_closed.set(true));

    // # 結果表示用のラベルを作る。
    let mut group = Group::new(&ui, "Note");
    let mut label_box = VerticalBox::new(&ui);
    let mut labels = Vec::new();
    for _ in 0..RESULT_RANGE {
        labels.push(Label::new(&ui, "_"));
        label_box.append(&ui, labels.last().unwrap().clone(), LayoutStrategy::Stretchy);
    };
    label_box.append(&ui, Label::new(&ui, "　　　　　　　"), LayoutStrategy::Stretchy);
    group.set_child(&ui, label_box);

    hbox.append(&ui, group, LayoutStrategy::Stretchy);
    hbox.append(&ui, Label::new(&ui, "    "), LayoutStrategy::Compact);

    // 設定用のボタン等を作る。
    let mut vbox = VerticalBox::new(&ui);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    let mut row_hbox = HorizontalBox::new(&ui);

    // # 窓関数を使うかどうかのチェックボックス
    let mut use_window_check = Checkbox::new(&ui, "窓関数を使う");
    use_window_check.set_checked(&ui, false);
    let cloned_shared_data = shared_data.clone();
    use_window_check.on_toggled(&ui, move |value|
        cloned_shared_data.use_window_flag.store(value, Ordering::SeqCst));
    row_hbox.append(&ui, use_window_check, LayoutStrategy::Compact);
    row_hbox.append(&ui, Label::new(&ui, "　"), LayoutStrategy::Compact);

    // # 無音データを設定するボタン
    let mut silent_button = Button::new(&ui, DEFAULT_SILENT_BUTTON_TEXT);
    let cloned_shared_data = shared_data.clone();
    let cloned_ui = ui.clone();
    silent_button.on_clicked(&ui, move |_button|
        if &_button.text(&cloned_ui) == DEFAULT_SILENT_BUTTON_TEXT {
            cloned_shared_data.use_silent.store(true, Ordering::SeqCst);
            _button.set_text(&cloned_ui, "無音データを消す");
        } else {
            cloned_shared_data.use_silent.store(false, Ordering::SeqCst);
            _button.set_text(&cloned_ui, DEFAULT_SILENT_BUTTON_TEXT);
        }
    );
    row_hbox.append(&ui, silent_button, LayoutStrategy::Stretchy);

    vbox.append(&ui, row_hbox, LayoutStrategy::Stretchy);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);

    // # 最低音量入力ボックス
    let mut min_volume_entry = Spinbox::new(&ui, 0, 100);
    min_volume_entry.set_value(&ui, 62);
    let cloned_shared_data = shared_data.clone();
    min_volume_entry.on_changed(&ui, move |value|
        cloned_shared_data.min_volume.store(
            (value as f32 / 100.0 * 80.0 - 80.0) as _,
            Ordering::SeqCst
        ));
    vbox.append(&ui, Label::new(&ui, "検出対象になる最低音量"), LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);
    vbox.append(&ui, min_volume_entry, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);

    // # ポイント数
    let mut point_times = Spinbox::new(&ui, 1, u16::MAX as _);
    point_times.set_value(&ui, 9);
    let cloned_shared_data = shared_data.clone();
    point_times.on_changed(&ui, move |value|
        cloned_shared_data.point_times.store(
            value as u16, Ordering::SeqCst
        ));
    vbox.append(&ui, Label::new(&ui, "ポイント数をデータの長さの何倍にするか"), LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);
    vbox.append(&ui, point_times, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);

    let mut row_hbox = HorizontalBox::new(&ui);

    // # 調整
    vbox.append(&ui, Label::new(&ui, "音程調整"), LayoutStrategy::Compact);
    let mut adjustment_rate = Spinbox::new(&ui, -127, 127);
    adjustment_rate.set_value(&ui, 0);
    let cloned_shared_data = shared_data.clone();
    adjustment_rate.on_changed(&ui, move |value|
        cloned_shared_data.adjustment_rate.store(
            value, Ordering::SeqCst
        ));
    row_hbox.append(&ui, adjustment_rate, LayoutStrategy::Compact);
    row_hbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // # MIDI出力を行うかのチェックボックス
    let mut midi_output_check = Checkbox::new(&ui, "MIDI出力");
    if output.is_none() { midi_output_check.disable(&ui); };
    let cloned_midi_output_check = midi_output_check.clone();
    row_hbox.append(&ui, midi_output_check, LayoutStrategy::Stretchy);
    let mut before_midi_number = Some(0);

    vbox.append(&ui, row_hbox, LayoutStrategy::Stretchy);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // 作ったボタン等をまとめる。
    hbox.append(&ui, vbox, LayoutStrategy::Compact);
    window.set_child(&ui, hbox);

    // ウィンドウを動かす。
    window.show(&ui);
    let mut event_loop = ui.event_loop();
    event_loop.on_tick(&ui, {
        let mut send_midi = |key, is_on|
            if let Some(output) = output.as_mut() {
                output.send(
                    &[if is_on { NOTE_ON_MSG } else { NOTE_OFF_MSG }, key, VELOCITY]
                ).unwrap();
            };

            let cloned_ui = ui.clone();
        let is_midi_on = move || cloned_midi_output_check.checked(&cloned_ui);

        let duration = Duration::from_secs_f32(0.05);
        let cloned_ui = ui.clone();

        move || {
            // マイク入力を処理するスレッドから送られてくる音程情報を処理する。
            if let Ok(note) = rx.recv_timeout(duration) {
                if let Some(note) = note {
                    labels[note.0].set_text(&cloned_ui, &format!(
                        "{}: {}", note.0 + 1, Notes::get_name(note.1 as usize)
                    ));

                    // MIDI出力が有効なら出力を行う。
                    if note.0 != 0 || !is_midi_on() {
                        return;
                    };

                    if let Some(before_number) = before_midi_number {
                        send_midi(before_number, false);
                    };
                    send_midi(note.1, true);
                    before_midi_number = Some(note.1);
                } else if let Some(before_number) = before_midi_number {
                    if is_midi_on() {
                        // 何も音が鳴っていないのなら前ならした音を無効にする。
                        send_midi(before_number, false);
                        before_midi_number = None;
                    };
                };
            };
        }
    });

    // ウィンドウが閉じられるまではイベントループを動かし続ける。
    while !is_closed.get() { event_loop.next_tick(&ui); };
}