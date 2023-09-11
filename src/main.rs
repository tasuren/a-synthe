#![cfg_attr(not(test), windows_subsystem = "windows")]
#![cfg_attr(test, windows_subsystem = "console")]

use std::{
    sync::{mpsc::channel, Arc},
    time::Duration,
};

use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use midir::MidiOutput;

mod midi;
mod misc;
mod sys;
mod ui;

use midi::MidiManager;
use misc::prelude::*;
use sys::{Note, NoteContainer, Synthesizer};
use ui::make_ui;

/// アプリの名前
const APPLICATION_NAME: &str = "aSynthe";
/// 表示する音階の個数。
const NUMBER_OF_NOTE_IN_RESULT: usize = 5;

/// イベントループの動くスレッドに何か伝えるのに使うイベント
pub enum BaseEvent<const NUMBER_OF_NOTE_IN_RESULT: usize> {
    // TODO: 下記のIssueが解決次第、ここは変更を行う。
    //   それは、Syntheに定数ジェネリクスを定め、それに`NUMBER_OF_NOTE_IN_RESULT`を設定したエイリアスをここで使うというもの。
    //   そのIssueはこれ：https://github.com/rust-lang/rust/issues/8995
    /// 音階の検出
    Synthesized(Option<[Note; NUMBER_OF_NOTE_IN_RESULT]>),
    // MIDIの出力先の変更
    UpdateMidiOutput(usize),
}
pub type Event = BaseEvent<NUMBER_OF_NOTE_IN_RESULT>;

mod logic {
    use super::{ui::update_note_monitor, MidiManager, Note};

    mod before_midi_number {
        //! 前回MIDIで送信した数値を記録するためのモジュールです。

        use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::SeqCst};

        static BEFORE_MIDI_NUMBER: AtomicU8 = AtomicU8::new(0);
        static BEFORE_MIDI_NUMBER_IS_FRESH: AtomicBool = AtomicBool::new(false);

        pub(super) fn get() -> Option<u8> {
            if BEFORE_MIDI_NUMBER_IS_FRESH.load(SeqCst) {
                Some(BEFORE_MIDI_NUMBER.load(SeqCst))
            } else {
                None
            }
        }

        pub(super) fn set(number: Option<u8>) {
            if let Some(number) = number {
                BEFORE_MIDI_NUMBER.store(number, SeqCst);
                BEFORE_MIDI_NUMBER_IS_FRESH.store(true, SeqCst);
            } else {
                BEFORE_MIDI_NUMBER_IS_FRESH.store(false, SeqCst);
            }
        }
    }

    /// 検出した音階をもとにMIDIの送信を行います。
    fn consume_midi_number(manager: &mut MidiManager, number: u8) {
        if !manager.is_avaliable() {
            return;
        };

        if let Some(before_midi_number) = before_midi_number::get() {
            if before_midi_number == number {
                // もし前回と同じ音が出ているのなら、音階を変えない。
                return;
            };

            // 前と同じじゃない音が出ているのなら、音を止める。
            manager.down_midi(before_midi_number);
        };

        // 音を出す。
        manager.up_midi(number);
        before_midi_number::set(Some(number));
    }

    /// 検出した音階を使って搭載している機能の諸々の処理をします。
    pub fn consume_notes<const N: usize>(
        midi_manager: &mut MidiManager,
        note_labels: &mut [libui::controls::Label; N],
        notes: Option<[Note; N]>,
    ) {
        if let Some(notes) = notes {
            let first_midi_number = notes[0].0;
            update_note_monitor::<N>(note_labels, notes);
            consume_midi_number(midi_manager, first_midi_number);
        } else if let Some(before_midi_number) = before_midi_number::get() {
            midi_manager.down_midi(before_midi_number);
            before_midi_number::set(None);
        };
    }
}

const CPU_SLEEP_INTERVAL: Duration = Duration::from_millis(5);

/// メインプログラムです。
fn main() {
    println!("{} by tasuren\nNow loading...", APPLICATION_NAME);

    // MIDIの用意をする。
    let midi_output = MidiOutput::new(APPLICATION_NAME)
        .context("MIDI出力の準備に失敗しました。")
        .unwrap_or_dialog_with_title(errors::INIT_ERROR);

    // マイクの設定を行う。
    let input_device = default_host()
        .default_input_device()
        .context("有効なデバイスがありません。")
        .unwrap_or_dialog_with_title(errors::INIT_ERROR);
    let input_device_config = input_device
        .default_input_config()
        .context("有効なデバイスの設定がありません。")
        .unwrap_or_dialog_with_title(errors::INIT_ERROR);

    // シンセの用意
    let mut synthesizer = Synthesizer::new(
        NoteContainer::new(),
        input_device_config.sample_rate().0 as _,
    );
    let config = Arc::clone(&synthesizer.config);

    // 録音および高速フーリエ変換の結果の送信を開始
    let (tx, rx) = channel();

    let input_stream = input_device
        .build_input_stream(
            &input_device_config.into(),
            {
                let tx = tx.clone();
                move |data: &[f32], _| {
                    let _ = tx.send(Event::Synthesized(synthesizer.synthe(data)));
                }
            },
            |e| {
                Some(e)
                    .context("デバイスとの通信が異常終了しました。")
                    .unwrap_or_dialog();
            },
            None,
        )
        .unwrap();
    input_stream.play().unwrap();

    let (ui, mut window, mut note_labels) = make_ui(
        tx,
        config,
        midi_output.ports().iter().map(|p| {
            midi_output
                .port_name(p)
                .unwrap_or_else(|_| "不明な出力先".to_string())
        }),
    );

    let mut midi_manager = MidiManager::new(midi_output);

    // ウィンドウの表示およびイベントループの開始
    window.show();
    let mut event_loop = ui.event_loop();
    println!("Started");

    while event_loop.next_tick() {
        if let Ok(event) = rx.recv_timeout(CPU_SLEEP_INTERVAL) {
            match event {
                Event::Synthesized(notes) => {
                    logic::consume_notes(&mut midi_manager, &mut note_labels, notes)
                }
                Event::UpdateMidiOutput(port_index) => {
                    midi_manager = midi_manager.set_midi_output(port_index)
                }
            };
        };
    }
}
