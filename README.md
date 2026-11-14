# reaper_marker_as_key.dll

マーカー名の `[key=C#:major]` などのタグを読み、開いているREAPERプロジェクトの調号へ反映するMOD。

> [!WARNING]
> 非公開の内部API `GetSetProjKeySig` を利用しています。
> 将来のREAPERのアップデートで動作しなくなる可能性があります。

## 使い方

- `reaper_marker_as_key.dll` を REAPER の `UserPlugins` フォルダにコピーします。
- Marker名に「`Intro [key=C#:major]`」のように `[key=調号]` のタグを付けます。

## ライセンス

MIT License
