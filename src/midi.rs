use std::{cell::Cell, rc::Rc};

use midir::{MidiOutput, MidiOutputConnection};

const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;
const VELOCITY: u8 = 0x64;

/// MIDIを管理するための構造体です。
pub struct MidiManager {
    connection: Option<MidiOutputConnection>,
    pub port_index: Rc<Cell<usize>>,
    real_port_index: usize,
}

impl MidiManager {
    /// インスタンスを作ります。
    pub fn new(midi_output: MidiOutput) -> Self {
        let port = &midi_output.ports()[0];

        Self {
            connection: if midi_output.ports().len() > 0 {
                Some(midi_output.connect(port, crate::APPLICATION_NAME).unwrap())
            } else {
                None
            },
            port_index: Rc::new(Cell::new(0)),
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

    /// 指定したキーでMIDIを有効にします。
    pub fn up_midi(&mut self, key: u8) {
        self.send_data(key, true)
    }

    /// 指定したキーでMIDIを無効にします。
    pub fn down_midi(&mut self, key: u8) {
        self.send_data(key, false)
    }

    /// MIDIの出力先の処理を行います。
    pub fn set_midi_output(mut self, port_index: usize) -> Self {
        self.port_index.replace(port_index);

        if self.real_port_index != port_index && port_index > 0 {
            if let Some(connection) = self.connection {
                let midi_output = connection.close();
                let port = &midi_output.ports()[port_index - 1];

                self.connection = Some(midi_output.connect(port, crate::APPLICATION_NAME).unwrap());
                self.real_port_index = port_index;
            };
        };

        self
    }

    /// MIDIが使用可能かどうかを調べます。
    pub fn is_avaliable(&self) -> bool {
        self.connection.is_some() && self.port_index.get() > 0
    }
}
