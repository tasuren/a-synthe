これは、マイクに入った音声から音階を検出するソフトです。  
WindowsとMac（ARM）に現在対応しているつもりです。  
あまり使えるものではありませんが、MIDIデバイスとして使うことができます。  
なお、GitHubのReleasesの欄からダウンロードが可能です。

**WARNING**  
まだ、モノラルのマイクしか対応していません。

## Screenshot
<img width="455" alt="screenshot" src="https://user-images.githubusercontent.com/45121209/188258254-a734da9b-8597-4956-a373-c845ee48119a.png">

## Build
### Windows
1. `cargo build --release`でビルドする。
2. staticフォルダ
### Mac
1. `cargo install cargo-bundle`を実行してcargo-bundleをインストールする。
2. `cargo bundle --release`を実行します。
3. 以下のコードをビルドされたappの`info.plist`の`dict`キー内に追記します。

```xml
  <key>NSMicrophoneUsageDescription</key>
  <string>音階検出のための。</string>
```
### ライセンス情報
[ここ](https://tasuren.github.io/a_synthe/licenses.html)にまとめてあります。  