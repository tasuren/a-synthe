pub mod app_meta {
    #[cfg(target_os="macos")]
    use core_foundation::bundle::CFBundle;


    /// Bundleのパスを取得します。
    #[cfg(target_os="macos")]
    fn get_bundle_path() -> String {
        CFBundle::main_bundle().path().unwrap().display().to_string()
    }

    /// ベースパスを取得します。通常`./src`を返します。
    /// Macではアプリ（バンドル）にした場合カレントディレクトリが`/`になってしまうので、リリースビルドの場合はアプリのリソースディレクトリへの絶対パスが返されます。
    /// Windowsのリリースビルドの場合`./`となります。
    pub fn get_base() -> String {
        #[cfg(target_os="windows")]
        return "./".to_string();
        #[cfg(target_os="macos")]
        return if cfg!(debug_assertions) {
            "./".to_string()
        } else { format!("{}/Contents/Resources/", get_bundle_path()) }
    }
}


pub mod core {
    use rustfft::{ FftPlanner, num_complex::{ Complex, ComplexFloat } };


    /// 窓関数の実装です。
    pub fn window(data: Vec<f32>) -> Vec<f32> {
        let ln = data.len() as f32 - 1.0;
        data.into_iter().enumerate().map(|(i, x)| {
            x * (1.0/2.0 * (1.0 - ComplexFloat::cos(
                (2.0 * 3.141592 * i as f32)/ln
            )))
        }).collect::<Vec<f32>>()
    }

    /// FFTの計算を行います。
    pub fn process_fft(
        data: Vec<f32>, point_times: usize,
        frame_rate: f32, use_window: bool
    ) -> (f32, Vec<f32>) {
        let mut buffer = if use_window { window(data) } else { data }
            .iter().map(|x| Complex { re: *x, im: 0.0 })
            .collect::<Vec<Complex<f32>>>();
        let data_length = buffer.len();

        // 高速フーリエ変換を行う。
        let mut planner = FftPlanner::<f32>::new();
        let point_length = data_length * point_times;
        let fft = planner.plan_fft_forward(point_length);
        for _ in 1..point_times {
            buffer.extend(vec![Complex { re: 0.0, im: 0.0 };data_length]);
        };
        fft.process(&mut buffer);

        (
            frame_rate as f32 / point_length as f32,
            buffer[..point_length / 2 - 1]
                .iter().map(|x| x.abs()).collect()
        )
    }
}