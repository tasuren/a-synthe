use std::sync::Arc;

/// ハン窓の実装です。
/// `data`に渡す値は、一つしかスマートポインタが存在しない場合に効率が良くなります。
pub fn han_window(mut data: Arc<[f32]>) -> Arc<[f32]> {
    let mut temporary_data;
    let data = match Arc::get_mut(&mut data) {
        Some(data) => data,
        None => {
            temporary_data = data.to_vec();
            &mut temporary_data
        }
    };

    let f32_length = data.len() as f32;
    for i in 0..data.len() {
        // NOTE: 参考文献：https://cognicull.com/ja/7r5k6y75
        data[i] =
            data[i] * (0.5 * (1. - (2. * std::f32::consts::PI * i as f32 / f32_length).cos()));
    }

    Arc::from(&*data)
}

/// 騒音レベルを取得します。
pub fn get_dba(data: &[f32]) -> f32 {
    // NOTE: 参考になると思うページは以下。
    //   - 要約
    //     - 前提として二乗平均平方根（RMS）：https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q1446027909
    //     - http://hsp.tv/play/pforum.php?mode=pastwch&num=47066
    //   - デシベルについて
    //     - https://mathwords.net/decibel
    //     - 詳細：https://ja.wikipedia.org/wiki/%E3%83%87%E3%82%B7%E3%83%99%E3%83%AB
    20. * (data.iter().map(|x| x.powi(2)).sum::<f32>() / data.len() as f32)
        .sqrt()
        .log10()
}

pub mod fft {
    use std::sync::Mutex;

    use rustfft::{
        num_complex::{Complex32, ComplexFloat},
        FftPlanner,
    };

    static BUFFER: Mutex<Vec<Complex32>> = Mutex::new(Vec::new());

    pub struct ResultInfo {
        /// 計算結果の解像度
        /// これは、各値が前の値からどれだけの周波数分だけ離れているかです。
        /// 例えば、`[2, 2, 4, 5, 5, 6, 5, 3, 2, 1]`のようなバッファとなり、それの解像度が2の場合を考えてみましょう。
        /// その場合は、バッファの各値の周波数の差が12Hzということですので、各値の成分は左から純に`0, 2, 4, 6, 8, 10, 12, ...`の周波数の音の大きさとなります。
        pub resolution: f32,
        /// バッファの長さ
        pub buffer_length: usize,
    }

    /// 高速フーリエ変換を行い、各周波数あたりの音の成分の大きさを割り出します。
    ///
    /// # Arguments
    /// - `data`: 処理する音声データ
    /// - `frame_rate`: 渡した処理対象の音声データのフレームレート
    ///     返り値の解像度の計算に使われます。
    /// - `point_times`: 計算結果の規模を何倍にするか
    ///     これをするとバッファが自動で音声データの長さをこの数値で乗算した数の長さまで拡張され、そのサイズ分のフーリエ変換を行います。
    ///     つまり、フーリエ変換の精度が上がります。（その分、処理が大変になります。）
    ///     NOTE: 詳細は次のページをご確認ください：https://www.logical-arts.jp/archives/112
    /// - `result_buffer`: 計算結果を代入するバッファ
    ///     NOTE: 自動でリサイズされるので、あらかじめ大きい数を割り当てるといったことはしなくても良いです。
    #[inline(always)]
    pub fn process(
        data: &[f32],
        frame_rate: f32,
        point_times: usize,
        result_buffer: &mut Vec<f32>,
    ) -> ResultInfo {
        let original_data_length = data.len();
        let buffer_length = original_data_length * point_times;

        // バッファの初期化を行う。バッファをグローバル変数に入れとくのは、毎回リソース確保をしないようにするため。
        let mut buffer = BUFFER.lock().unwrap();
        if buffer.len() != buffer_length {
            buffer.resize_with(buffer_length, Default::default);
        };
        if buffer_length != result_buffer.len() {
            result_buffer.resize_with(buffer_length, Default::default)
        };

        // 初期化を行う。具体的には、録音したデータの設定と、前のデータの削除です。
        for (i, v) in data.iter().enumerate() {
            buffer[i].re = *v;
            if buffer[i].im != 0. {
                buffer[i].im = 0.;
            };
        }

        for i in original_data_length..buffer_length {
            if buffer[i].re != 0. {
                buffer[i].re = 0.;
            };
            if buffer[i].im != 0. {
                buffer[i].im = 0.;
            };
        }

        // 高速フーリエ変換の用意をする。
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(buffer_length);

        // 実行する。
        fft.process(&mut buffer);

        // 結果を書き込む。
        for (i, v) in buffer.iter().map(|c| c.abs()).enumerate() {
            result_buffer[i] = v;
        }

        ResultInfo {
            resolution: frame_rate as f32 / buffer_length as f32,
            buffer_length,
        }
    }
}
