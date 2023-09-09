pub mod note;
pub mod calculation;

pub use note::{Note, NoteContainer};


use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering::SeqCst},
    Arc,
};

/// スレッド間で共有する値を入れるための構造体です。
pub struct Config {
    pub min_volume: AtomicI32,
    pub point_times: AtomicU16,
    pub use_window_flag: AtomicBool,
    pub use_silent: AtomicBool,
    pub adjustment_rate: AtomicI32,
}

/// 音程を検出するためのものを実装した構造体です。
pub struct Synthesizer {
    notes: NoteContainer,
    frame_rate: f32,
    silence: Option<Vec<f32>>,
    pub config: Arc<Config>,
}

impl Synthesizer {
    /// インスタンスを作ります。
    pub fn new(notes: NoteContainer, frame_rate: f32) -> Self {
        Self {
            notes: notes,
            frame_rate: frame_rate,
            silence: None,
            config: Arc::new(Config {
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
    pub fn synthe<const N: usize>(&mut self, data: &[f32]) -> Option<[Note; N]> {
        if calculation::get_dba(data) as i32 <= self.config.min_volume.load(SeqCst) {
            return None;
        };

        // FFTで周波数の計算をする。
        let (frequency_resolution, mut data) = calculation::process_fft(
            data.to_vec(),
            self.config.point_times.load(SeqCst) as usize,
            self.frame_rate,
            self.config.use_window_flag.load(SeqCst),
        );

        // 無音時のサンプルデータを設定するように言われているのなら設定をする。
        if self.config.use_silent.load(SeqCst) {
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
                self.config.use_silent.store(false, SeqCst);
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
        let adjustment_rate = self.config.adjustment_rate.load(SeqCst);
        let mut result = [Note::NULL; N];
        let mut value;

        for i in 0..N {
            value = values.pop().unwrap().0 as i32 + adjustment_rate;

            if value < 0 {
                value = 0;
            };
            if value > 127 {
                value = 127;
            };

            result[i] = Note(value as u8);
        }

        Some(result)
    }
}
