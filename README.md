![GitHub all releases](https://img.shields.io/github/downloads/tasuren/a-synthe/total)
# aSynthe
これは、マイクに入った音声から音程を割り出すソフトで、WindowsとMacに現在対応しているつもりです。  
あまり使えるものではありませんが、MIDIデバイスとして使うことができます。

**WARNING**  
まだ、モノラルのマイクしか対応していません。Linuxは動作未確認です。

## スクリーンショット
<img width="634" alt="aSyntheがレ/D(5)を示す様子" src="https://github.com/tasuren/a-synthe/assets/45121209/b65278fe-ec1e-4133-a7a1-6b95b708349a">

## 謝辞
このソフトウェアは様々なライブラリを元に成り立っています。それらのライセンス情報は[ここ](https://tasuren.github.io/a-synthe/licenses.html)から確認が可能です。

## ライセンス
このソフトウェアは4条項BSDライセンスの下に提供されます。

## ビルド方法
### Windows
```shell
$ cargo build --release
```
### Mac
1. `cargo install cargo-bundle`を実行してcargo-bundleをインストール
2. `cargo bundle --release`を実行
3. 以下のコードをビルドされたアプリの`info.plist`にて、`dict`キー内に以下を追記

```xml
  <key>NSMicrophoneUsageDescription</key>
  <string>音階検出のための。</string>
```
