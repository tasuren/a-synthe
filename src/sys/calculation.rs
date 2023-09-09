use rustfft::{
    num_complex::{Complex, ComplexFloat},
    FftPlanner,
};


/// ハン窓の実装です。
pub fn window(data: Vec<f32>) -> Vec<f32> {
    let len = data.len() as f32 - 1.;
    // NOTE: 参考文献：https://cognicull.com/ja/7r5k6y75
    data.into_iter()
        .enumerate()
        .map(|(i, x)| x * (
            0.5 * (1. - ComplexFloat::cos(
                (2. * std::f32::consts::PI * i as f32) / len
            ))
        ))
        .collect::<Vec<f32>>()
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

/// FFTの計算を行います。
#[inline(always)]
pub fn process_fft(
    data: Vec<f32>,
    point_times: usize,
    frame_rate: f32,
    use_window: bool,
) -> (f32, Vec<f32>) {
    // NOTE: 窓関数を使う理由に関してはここが参考になると思う：https://www.logical-arts.jp/archives/124
    let mut buffer = if use_window { window(data) } else { data }
        .iter()
        .map(|x| Complex { re: *x, im: 0. }) // NOTE: 虚部は現実世界で認識できないため0。
        .collect::<Vec<Complex<f32>>>();
    let data_length = buffer.len();

    // 高速フーリエ変換を行う。
    let mut planner = FftPlanner::<f32>::new();
    let point_length = data_length * point_times;
    let fft = planner.plan_fft_forward(point_length);
    for _ in 1..point_times {
        buffer.extend(vec![Complex { re: 0.0, im: 0.0 }; data_length]);
    }
    fft.process(&mut buffer);

    (
        frame_rate as f32 / point_length as f32,
        buffer[..point_length / 2 - 1]
            .iter()
            .map(|x| x.abs())
            .collect(),
    )
}
