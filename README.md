# reaper_marker_as_key.dll

マーカー名の `[key=C#:major]` などのタグを読み、開いているREAPERプロジェクトの調号へ反映するMOD。

> [!WARNING]
> 非公開の内部API `GetSetProjKeySig` を利用しています。
> 将来のREAPERのアップデートで動作しなくなる可能性があります。

## 使い方

- `reaper_marker_as_key.dll` を REAPER の `UserPlugins` フォルダにコピーします。
- Marker名に「`Intro [key=C#:major]`」のように `[key=調号]` のタグを付けます。
- アクション `reaper_marker_as_key.dll: Set project key signatures from markers` を実行するとマーカーを反映します。

## 設定

初回ロード時に `reaper_marker_as_key.toml` ファイルを作成します。
アクション `reaper_marker_as_key.dll: Open configuration` でメモ帳から編集できます。

- スケールは音階のビットマスク、別のスケール名へのエイリアス、あるいは `.reascale` ファイルで指定します。
- 文字列は同じ `[scales]` 内の名前を参照します。エイリアスの連鎖にも対応し、未定義の名前や循環参照はエラーになります。
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

## ライセンス

MIT License
