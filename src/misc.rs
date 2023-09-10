pub mod prelude {
    pub use dialog_unwrapper::prelude::*;

    pub mod errors {
        pub const INIT_ERROR: &str = "初期化エラー";
    }
}

pub mod app_meta {
    use dialog_unwrapper::rfd::{AsyncMessageDialog, MessageLevel};

    /// アプリケーションの情報を表示します。
    pub fn show_about() {
        let _ = AsyncMessageDialog::new()
            .set_title("このアプリについて")
            .set_description(&format!(
                "aSynthe v{}\n(c) 2022 Takagi Tasuku\n\nリポジトリ：https://github.com/tasuren/aSynthe\n{}",
                env!("CARGO_PKG_VERSION"), "ライセンス情報：https://tasuren.github.io/a-synthe"
            ))
            .set_level(MessageLevel::Info)
            .show();
    }
}
