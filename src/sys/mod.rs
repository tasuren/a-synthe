use std::{
    cmp::Ordering as CmpOrdering,
    collections::BinaryHeap,
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering::SeqCst},
        Arc,
    },
};

pub mod calculation;
pub mod note;

pub use note::{Note, NoteContainer};

/// スレッド間で共有する値を入れるための構造体
pub struct Config {
    pub min_volume: AtomicI32,
    pub point_times: AtomicU16,
    pub use_window_flag: AtomicBool,
    pub use_silent: AtomicBool,
    pub adjustment_rate: AtomicI32,
}

/// 生の音階データを格納するための構造体
#[derive(PartialEq)]
struct RawNote(u8, f32);

impl PartialOrd for RawNote {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        self.1.partial_cmp(&other.1)
    }
}
impl Eq for RawNote {}
impl Ord for RawNote {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.1.partial_cmp(&other.1).unwrap_or(CmpOrdering::Less)
    }
}

/// 音程を検出するためのものを実装した構造体
pub struct Synthesizer {
    notes: NoteContainer,
    frame_rate: f32,
    silence: Option<Arc<[f32]>>,
    buffer: Vec<f32>,
    detected_raw_notes: BinaryHeap<RawNote>,
    pub config: Arc<Config>,
}

impl Synthesizer {
    /// インスタンスを作ります。
    pub fn new(notes: NoteContainer, frame_rate: f32) -> Self {
        Self {
            notes: notes,
            frame_rate: frame_rate,
            silence: None,
            buffer: Vec::new(),
            detected_raw_notes: BinaryHeap::new(),
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
    pub fn synthe<const N: usize>(&mut self, data: Arc<[f32]>) -> Option<[Note; N]> {
        if calculation::get_dba(&data) as i32 <= self.config.min_volume.load(SeqCst) {
            return None;
        };

        // FFTで周波数の計算をする。
        let info = calculation::fft::process(
            if self.config.use_window_flag.load(SeqCst) {
                calculation::han_window(data)
            } else {
                data
            },
            self.frame_rate,
            self.config.point_times.load(SeqCst) as _,
            &mut self.buffer,
        );
        let data = &mut self.buffer;

        // 無音データの処理をする。
        if self.config.use_silent.load(SeqCst) {
            if let Some(silence) = &self.silence {
                // 無音時のデータがあるのなら、無音データのサンプルをこのときのデータから差し引く。
                for (index, value) in silence.iter().enumerate() {
                    if data[index] > *value {
                        data[index] -= value;
                    } else {
                        data[index] = 0.;
                    };
                }
            } else {
                // 無音データが設定されてないなら設定を行う。
                self.silence = Some(Arc::from(data.as_slice()));
                self.config.use_silent.store(false, SeqCst);
                return None;
            };
        } else if self.silence.is_some() {
            // もし無音データを忘れさせられたのなら、無音データのサンプルを削除する。
            self.silence = None;
        };

        // 一番音量が高い周波数の音程を探す。
        self.detected_raw_notes.clear();
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
            stack = &data[(before_frequency / info.resolution) as usize
                ..(after_frequency / info.resolution) as usize];
            value = stack.iter().sum::<f32>() / stack.len() as f32;

            if !value.is_nan() {
                self.detected_raw_notes.push(RawNote(number, value));
            };
        }

        // メインスレッドに検出した音程を送信する。
        let adjustment_rate = self.config.adjustment_rate.load(SeqCst);
        let mut result = [Note::NULL; N];
        let mut value;

        for i in 0..N {
            if let Some(raw_note) = self.detected_raw_notes.pop() {
                value = raw_note.0 as i32 + adjustment_rate;

                if value < 0 {
                    value = 0;
                };
                if value > 127 {
                    value = 127;
                };

                result[i] = Note(value as u8);
            }
        }

        Some(result)
    }
}
