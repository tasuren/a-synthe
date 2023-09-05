pub mod midi {
    use std::rc::Rc;

    use midir::{MidiOutput, MidiOutputConnection};

    const NOTE_ON_MSG: u8 = 0x90;
    const NOTE_OFF_MSG: u8 = 0x80;
    const VELOCITY: u8 = 0x64;
    const DEFAULT_SILENT_BUTTON_TEXT: &str = "無音データを設定する";

    /// MIDIを管理するための構造体です。
    pub struct MidiManager {
        connection: Option<MidiOutputConnection>,
        port_index: Rc<usize>,
        real_port_index: usize,
    }

    impl MidiManager {
        /// インスタンスを作ります。
        pub fn new(output: MidiOutput) -> Self {
            Self {
                connection: if output.ports().len() > 0 {
                    Some(
                        output
                            .connect(&output.ports()[0], crate::APPLICATION_NAME)
                            .unwrap(),
                    )
                } else {
                    None
                },
                port_index: Rc::new(0),
                real_port_index: 0,
            }
        }

        /// MIDIのデータを送ります。
        pub fn send_data(&mut self, key: u8, is_on: bool) {
            self.connection
                .as_mut()
                .unwrap()
                .send(&[
                    if is_on { NOTE_ON_MSG } else { NOTE_OFF_MSG },
                    key,
                    VELOCITY,
                ])
                .unwrap();
        }

        /// MIDIの出力先の処理を行います。
        pub fn set_midi_output(mut self) -> Self {
            let port_index = *self.port_index;

            if self.real_port_index != port_index && port_index > 0 {
                if let Some(connection) = self.connection {
                    let output = connection.close();
                    let port = &output.ports()[port_index - 1];

                    self.connection = Some(output.connect(port, crate::APPLICATION_NAME).unwrap());
                    self.real_port_index = port_index;
                };
            };

            self
        }

        /// MIDIが使用可能かどうかを調べます。
        pub fn is_avaliable(&self) -> bool {
            self.connection.is_some() && *self.port_index > 0
        }
    }
}

pub mod app_meta {
    #[cfg(target_os = "macos")]
    use core_foundation::bundle::CFBundle;

    /// Bundleのパスを取得します。
    #[cfg(target_os = "macos")]
    fn get_bundle_path() -> String {
        CFBundle::main_bundle()
            .path()
            .unwrap()
            .display()
            .to_string()
    }

    /// ベースパスを取得します。通常`./src`を返します。
    /// Macではアプリ（バンドル）にした場合カレントディレクトリが`/`になってしまうので、リリースビルドの場合はアプリのリソースディレクトリへの絶対パスが返されます。
    /// Windowsのリリースビルドの場合`./`となります。
    pub fn get_base() -> String {
        #[cfg(target_os = "windows")]
        return "./".to_string();
        #[cfg(target_os = "macos")]
        return if cfg!(debug_assertions) {
            "./".to_string()
        } else {
            format!("{}/Contents/Resources/", get_bundle_path())
        };
    }
}
