use std::{
    fs::File,
    io::{BufRead, BufReader},
};

pub mod calculation;

const NOTE_NAMES: [&str; 12] = [
    "ド/C",
    "ド♯/C♯",
    "レ/D",
    "レ♯/D♯",
    "ミ/E",
    "ファ/F",
    "ファ♯/F♯",
    "ソ/G",
    "ソ♯/G♯",
    "ラ/A",
    "ラ♯/A♯",
    "シ/B",
];

/// 音程と音程に対応する周波数を格納するための構造体です。
#[derive(Default)]
pub struct Notes {
    numbers: Vec<u8>,
    frequencies: Vec<f32>,
    before_frequencies: Vec<f32>,
    after_frequencies: Vec<f32>,
}

impl Notes {
    /// 音程等が書き込まれたファイルから音程等を読み込みます。
    pub fn new(path: String) -> Self {
        let mut notes = Self::default();
        let (mut line, mut raw);

        for tentative in BufReader::new(File::open(path).unwrap()).lines() {
            raw = tentative.unwrap();
            line = raw.split_whitespace();
            notes.numbers.push(line.next().unwrap().parse().unwrap());
            notes
                .frequencies
                .push(line.next().unwrap().parse().unwrap());
            notes
                .before_frequencies
                .push(line.next().unwrap().parse().unwrap());
            notes
                .after_frequencies
                .push(line.next().unwrap().parse().unwrap());
        }

        notes
    }

    /// 音程の名前を取得します。
    pub fn get_name(number: usize) -> String {
        format!(
            "{} {}",
            NOTE_NAMES[number - 12 * (number / 12)],
            (number / 12) as isize - 1
        )
    }
}

/// 音程情報を入れるための構造体です。
pub struct Note(usize, u8);
impl Note {
    pub const NULL: Self = Self(0, 0);
}

use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering::SeqCst},
    Arc,
};

/// スレッド間で共有する値を入れるための構造体です。
pub struct SharedData {
    pub min_volume: AtomicI32,
    pub point_times: AtomicU16,
    pub use_window_flag: AtomicBool,
    pub use_silent: AtomicBool,
    pub adjustment_rate: AtomicI32,
}

/// 音程を検出するためのものを実装した構造体です。
pub struct Synthe {
    notes: Notes,
    frame_rate: f32,
    silence: Option<Vec<f32>>,
    shared_data: Arc<SharedData>,
}

impl Synthe {
    const RESULT_RANGE: usize = 5;

    /// インスタンスを作ります。
    fn new(notes: Notes, frame_rate: f32) -> Self {
        Self {
            notes: notes,
            frame_rate: frame_rate,
            silence: None,
            shared_data: Arc::new(SharedData {
                min_volume: AtomicI32::new(-30),
                point_times: AtomicU16::new(8),
                use_window_flag: AtomicBool::new(false),
                use_silent: AtomicBool::new(false),
                adjustment_rate: AtomicI32::new(0),
            }),
        }
    }

    /// 音程検出の処理を行います。
    #[inline]
    fn process(&mut self, data: &[f32]) -> Option<[Note; Self::RESULT_RANGE]> {
        // 音量を調べる。
        let volume = 20.0
            * (data.iter().map(|x| x.powi(2)).sum::<f32>() / data.len() as f32)
                .sqrt()
                .log10();

        if volume as i32 <= self.shared_data.min_volume.load(SeqCst) {
            return None;
        };

        // FFTで周波数の計算をする。
        let (frequency_resolution, mut data) = calculation::process_fft(
            data.to_vec(),
            self.shared_data.point_times.load(SeqCst) as usize,
            self.frame_rate,
            self.shared_data.use_window_flag.load(SeqCst),
        );

        // 無音時のサンプルデータを設定するように言われているのなら設定をする。
        if self.shared_data.use_silent.load(SeqCst) {
            if let Some(silence) = &self.silence {
                // 無音時のデータがあるのなら、無音データを消す。
                for (index, value) in silence.iter().enumerate() {
                    if data[index] > *value {
                        data[index] -= value;
                    } else {
                        data[index] = 0.0;
                    };
                }
            } else {
                self.silence = Some(data);
                self.shared_data.use_silent.store(false, SeqCst);
                return None;
            };
        } else if self.silence.is_some() {
            self.silence = None;
        };

        // 一番音量が高い周波数の音程を探す。
        let mut values = Vec::new();
        let (mut stack, mut value);

        for (number, before_frequency, after_frequency) in self
            .notes
            .numbers
            .iter()
            .zip(
                self.notes
                    .before_frequencies
                    .iter()
                    .zip(self.notes.after_frequencies.iter()),
            )
            .map(|(number, (bf, af))| (*number, *bf, *af))
        {
            stack = &data[(before_frequency / frequency_resolution) as usize
                ..(after_frequency / frequency_resolution) as usize];
            value = stack.iter().sum::<f32>() / stack.len() as f32;

            if !value.is_nan() {
                values.push((number, value));
            };
        }

        values.sort_by(|(_, x), (_, y)| x.partial_cmp(y).unwrap());

        // メインスレッドに検出した音程を送信する。
        let adjustment_rate = self.shared_data.adjustment_rate.load(SeqCst);
        let mut result = [Note::NULL; Self::RESULT_RANGE];
        let mut value;

        for i in 0..Self::RESULT_RANGE {
            value = values.pop().unwrap().0 as i32 + adjustment_rate;

            if value < 0 {
                value = 0;
            };
            if value > 127 {
                value = 127;
            };

            result[i] = Note(i, value as u8);
        }

        Some(result)
    }
}
