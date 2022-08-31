//! aSynthe

use std::{
    time::Duration, fs::File, io::{ BufRead, BufReader }, sync::{
        Arc, atomic::{ AtomicU16, AtomicBool, Ordering, AtomicI32 },
        mpsc::{ Sender, channel }
    }, rc::Rc, cell::Cell
};

use lazy_static::lazy_static;

//use midir::{ MidiOutput, MidiOutputConnection };
use cpal::{
    traits::{ HostTrait, DeviceTrait, StreamTrait },
    default_host
};

use iui::{ controls::{
    Button, Label, HorizontalBox, LayoutStrategy, Group,
    Spinbox, Checkbox, VerticalBox, Spacer },
    prelude::*
};

mod lib;
use lib::process_fft;


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
    fn new(path: &str) -> Self {
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
        format!("{} {}", NOTE_NAMES[number - 12 * (number / 12)], (number / 12) as isize - 2)
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
    use_silent: Arc<AtomicBool>
}


/// 音程を検出するためのものを実装した構造体です。
struct Synthe {
    notes: Notes,
    tx: Sender<Note>,
    frame_rate: f32,
    silence: Option<Vec<f32>>,
    shared_data: SharedData
}

impl Synthe {
    fn new(notes: Notes, tx: Sender<Note>, frame_rate: f32) -> Self {
        Self {
            notes: notes, tx: tx, frame_rate: frame_rate, silence: None,
            shared_data: SharedData {
                min_volume: Arc::new(AtomicI32::new(-30)),
                point_times: Arc::new(AtomicU16::new(8)),
                use_window_flag: Arc::new(AtomicBool::new(false)),
                use_silent: Arc::new(AtomicBool::new(false))
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
                data.iter().map(|x| *x),
                self.shared_data.point_times.load(Ordering::SeqCst) as usize,
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
            for i in 0..RESULT_RANGE {
                self.tx.send(Note(i, values.pop().unwrap().0)).unwrap();
            };
        };
    }
}


const DEFAULT_SILENT_BUTTON_TEXT: &str = "無音データを設定する";


fn main() {
    println!("aSynthe by tasuren\nNow loading...");
    // 別スレッドとの通信用のチャンネルを作る。
    let (tx, rx) = channel();

    /*
    let output = MidiOutput::new("asynthe").unwrap();
    if output.port_count() == 0 { println!("No output port is avaliable."); return; };
    let port = &output.ports()[0];
    let output = output.connect(port, "asynthe").unwrap();
    */

    // マイクの設定を行う。
    let device = default_host().default_input_device().expect("No device is avaliable.");
    let config = device.default_input_config().expect("No device config is avaliable.");
    let mut synthe = Synthe::new(
        Notes::new("src/notes.csv"), tx,
        config.sample_rate().0 as f32
    );
    let shared_data = synthe.shared_data.clone();
    let stream = device.build_input_stream(
        &config.into(), move |data: &[f32], _| synthe.process(data),
        |e| println!("Error: {}", e)
    ).unwrap();
    stream.play().unwrap();

    // ウィンドウを作る。
    let ui = UI::init().expect("Couldn't initialize UI library.");
    let mut window = Window::new(&ui, APPLICATION_NAME, 300, 200, WindowType::NoMenubar);
    let mut hbox = HorizontalBox::new(&ui);

    // # 結果表示用のラベルを作る。
    let mut group = Group::new(&ui, "Note");
    let mut label_box = VerticalBox::new(&ui);
    let mut labels = Vec::new();
    for _ in 0..RESULT_RANGE {
        labels.push(Label::new(&ui, "_"));
        label_box.append(&ui, labels.last().unwrap().clone(), LayoutStrategy::Stretchy);
    };
    group.set_child(&ui, label_box);

    hbox.append(&ui, group, LayoutStrategy::Stretchy);
    hbox.append(&ui, Label::new(&ui, "    "), LayoutStrategy::Compact);

    // 設定用のボタン等を作る。
    let mut vbox = VerticalBox::new(&ui);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

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
    vbox.append(&ui, silent_button, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // # 最低音量入力ボックス
    let mut min_volume_entry = Spinbox::new(&ui, 0, 100);
    min_volume_entry.set_value(&ui, 62);
    let cloned_shared_data = shared_data.clone();
    min_volume_entry.on_changed(&ui, move |value|
        cloned_shared_data.min_volume.store(
            value * (0 - (-80)) - 80, Ordering::SeqCst
        ));
    vbox.append(&ui, Label::new(&ui, "検出対象になる最低音量"), LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);
    vbox.append(&ui, min_volume_entry, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // # ポイント数
    let mut point_times = Spinbox::new(&ui, 1, u16::MAX as i32);
    let cloned_shared_data = shared_data.clone();
    point_times.on_changed(&ui, move |value|
        cloned_shared_data.point_times.store(
            value as u16, Ordering::SeqCst
        ));
    vbox.append(&ui, Label::new(&ui, "ポイント数をデータの長さの何倍にするか"), LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Compact);
    vbox.append(&ui, point_times, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // # 窓関数を使うかどうかのチェックボックス
    let mut use_window_check = Checkbox::new(&ui, "窓関数を使う");
    use_window_check.set_checked(&ui, false);
    let cloned_shared_data = shared_data.clone();
    use_window_check.on_toggled(&ui, move |value|
        cloned_shared_data.use_window_flag.store(value, Ordering::SeqCst));
    vbox.append(&ui, use_window_check, LayoutStrategy::Compact);
    vbox.append(&ui, Spacer::new(&ui), LayoutStrategy::Stretchy);

    // 作ったボタン等をまとめる。
    hbox.append(&ui, vbox, LayoutStrategy::Compact);
    window.set_child(&ui, hbox);

    let is_closed = Rc::new(Cell::new(false));
    let cloned_is_closed = is_closed.clone();
    window.on_closing(&ui, move |_| cloned_is_closed.set(true));

    // ウィンドウを動かす。
    window.show(&ui);
    let mut event_loop = ui.event_loop();
    event_loop.on_tick(&ui, {
        let cloned_ui = ui.clone();
        let duration = Duration::from_secs_f32(0.05);

        move || {
            if let Ok(note) = rx.recv_timeout(duration) {
                println!("{}", note.1);
                labels[note.0].set_text(&cloned_ui, &format!(
                    "{}: {}", note.0 + 1, Notes::get_name(note.1 as usize)
                ));
            };
        }
    });
    while !is_closed.get() { event_loop.next_tick(&ui); };
}