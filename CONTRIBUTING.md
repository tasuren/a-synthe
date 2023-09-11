# Contributing Guide
コードはrustfmtというフォーマッタで整えてください。

## ライセンス情報の出力
`cargo-about`を使って以下のコマンドで生成できます。
```shell
$ cargo about generate about.hbs > docs/licenses.html
```