# aSynthe
これはマイクに入った音声から音程を検出するソフトです。  
WindowsとMacに現在対応しています。

**WARNING**  
まだ、モノラルのマイクしか対応していません。(めんどくさかった)

## Screenshot
<img width="455" alt="screenshot" src="https://user-images.githubusercontent.com/45121209/188252781-38399117-1a78-47df-a8c3-d13ce02b35c0.png">

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