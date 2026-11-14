[English](./README.en.md) | **日本語**

# reaper_marker_as_key.dll

マーカー名の `[key=C#:major]` などのタグを読み、開いているREAPERプロジェクトの調号へ反映するMOD。

> [!WARNING]
> 非公開の内部API `GetSetProjKeySig` を利用しています。
> 将来のREAPERのアップデートで動作しなくなる可能性があります。

## インストール

Windows x64版REAPERに対応しています。

### ReaPack

`Extensions > ReaPack > Import repositories...` から次のURLを登録します。

```text
https://github.com/sevenc-nanashi/reaper_marker_as_key.dll/releases/latest/download/index.xml
```

`Browse packages...` で `reaper_marker_as_key` をインストールし、REAPERを再起動してください。
このURLは初回リリースの公開後に利用できます。インデックスには最新リリースのみを掲載します。
設定TOMLはDLLが初回ロード時に生成するため、ReaPackでの更新時にも既存の設定を保持します。

### 手動

[Releases](https://github.com/sevenc-nanashi/reaper_marker_as_key.dll/releases/latest) から
`reaper_marker_as_key.dll` をダウンロードし、REAPERの `UserPlugins` フォルダーにコピーして再起動してください。

## 使い方

- Marker名に「`Intro [key=C#:major]`」のように `[key=調号]` のタグを付けます。
- アクション `reaper_marker_as_key.dll: Set project key signatures from markers` を実行するとマーカーを反映します。

## 設定

初回ロード時に `reaper_marker_as_key.toml` ファイルを作成します。
アクション `reaper_marker_as_key.dll: Open configuration` でメモ帳から編集できます。

- スケールは音階のビットマスク、別のスケール名へのエイリアス、あるいは `.reascale` ファイルで指定します。
- ビットマスクは12bitの整数で、LSBが主音、MSBが第7音に対応します。例えばメジャースケールは `0b101010110101` です。
- 文字列は同じ `[scales]` 内の名前を参照します。
- `{ path = "...", scale = "..." }` の形式で `.reascale` ファイルを指定できます。
- キーがそのままスケール名として使われます。
- デフォルトは[./reaper_marker_as_key.toml](./reaper_marker_as_key.toml)を参照してください。

```toml
[scales]
major = 0b101010110101
minor = 0b010110101101
maj = "major"
another = { path = "scales/custom.reascale", scale = "Dorian" }
```

## リリース手順

1. `Cargo.toml` の `version` と `Cargo.lock` を更新します。
2. 変更を取り込んだコミットに、そのバージョンと一致するタグ（例: `v0.1.0`）を付け、GitHubへ公開します。
3. `Release` workflowがフォーマットとClippyを確認し、Windows x64用DLLをビルドします。
4. DLL、ライセンス、ReaPackの `index.xml` を添付したGitHub Releaseが自動公開されます。

通常の `GITHUB_TOKEN` を使うため、追加のシークレットは不要です。安定版の `vMAJOR.MINOR.PATCH` タグに対応します。

## ライセンス

このMODはMITライセンスの下で公開されています。
