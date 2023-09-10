/// 音程と音程に対応する周波数を格納するための構造体です。
#[derive(Default)]
pub struct NoteContainer {
    pub numbers: Vec<u8>,
    pub frequencies: Vec<f32>,
    pub before_frequencies: Vec<f32>,
    pub after_frequencies: Vec<f32>,
}

impl NoteContainer {
    /// 音程等が書き込まれたファイルから音程等を読み込みます。
    pub fn new() -> Self {
        let mut notes = Self::default();
        let mut row;

        for line in include_str!("notes.csv").lines() {
            row = line.split_whitespace();
            notes.numbers.push(row.next().unwrap().parse().unwrap());
            notes.frequencies.push(row.next().unwrap().parse().unwrap());
            notes
                .before_frequencies
                .push(row.next().unwrap().parse().unwrap());
            notes
                .after_frequencies
                .push(row.next().unwrap().parse().unwrap());
        }

        notes
    }
}

/// 音程情報を入れるための構造体です。
#[derive(Clone)]
pub struct Note(pub u8);
impl Note {
    pub const NULL: Self = Self(0);

    /// 音階の名前をまとめた配列
    const AVALIABLE_NAMES: [&str; 12] = [
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

    /// 音階の名前を文字列で取得します。
    pub fn get_name(&self) -> String {
        format!(
            "{} {}",
            Self::AVALIABLE_NAMES[(self.0 - 12 * (self.0 / 12)) as usize],
            (self.0 / 12) as isize - 1
        )
    }
}

impl Into<u8> for Note {
    fn into(self) -> u8 {
        self.0
    }
}

impl AsRef<u8> for Note {
    fn as_ref(&self) -> &u8 {
        &self.0
    }
}
