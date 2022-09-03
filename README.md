# aSynthe
これはマイクに入った音声から音程を検出するソフトです。  
WindowsとMac(M1)に現在対応しています。

**WARNING**  
まだ、モノラルのマイクしか対応していません。(めんどくさかった)

## Screenshot
<img width="455" alt="screenshot" src="https://user-images.githubusercontent.com/45121209/188258254-a734da9b-8597-4956-a373-c845ee48119a.png">

## Downloads
ソフトのダウンロードは[こちら](https://github.com/tasuren/aSynthe/releases)からできます。

## Build
### Windows
`cargo build --release`の実行できます。

### Mac
1. `cargo install cargo-bundle`を実行してcargo-bundleをインストールする。
2. `cargo bundle --release`を実行します。
3. 以下のコードをビルドされたappの`info.plist`の`dict`キー内に追記します。
```xml
<key>NSMicrophoneUsageDescription</key>
<string>音程検出のための音声拾いのため。</string>
```