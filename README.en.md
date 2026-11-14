**English** | [日本語](./README.md)

# reaper_marker_as_key.dll

A REAPER mod that reads tags like `[key=C#:major]` in marker names and applies them to the key signature of the open REAPER project.

> [!WARNING]
> This uses the non-public internal API `GetSetProjKeySig`. It may stop working with future REAPER updates.

## Installation

Windows x64 REAPER is supported.

### ReaPack

Use `Extensions > ReaPack > Import repositories...` to add:

```text
https://github.com/sevenc-nanashi/reaper_marker_as_key.dll/releases/latest/download/index.xml
```

Install `reaper_marker_as_key` from `Browse packages...`, then restart REAPER.
This URL becomes available after the first release. The index lists only the latest release.
The DLL creates the configuration TOML on first load, so ReaPack updates preserve existing settings.

### Manual

Download `reaper_marker_as_key.dll` from [Releases](https://github.com/sevenc-nanashi/reaper_marker_as_key.dll/releases/latest),
copy it to REAPER's `UserPlugins` folder, and restart REAPER.

## Usage

- Add a tag like `[key=key_signature]` to the marker name, e.g., `Intro [key=C#:major]`.
- Run the action `reaper_marker_as_key.dll: Set project key signatures from markers` to apply the markers.
- All existing project key signatures, including manually added ones, will be removed and recreated from the tags.

## Configuration

This plugin creates a `reaper_marker_as_key.toml` file on first load. You can edit it in Notepad by running the action `reaper_marker_as_key.dll: Open configuration`.

- Scales can be specified as a bitmask of notes, an alias to another scale name, or a `.reascale` file.
- The bitmask is a 12-bit integer where the LSB corresponds to the tonic and the MSB corresponds to the 7th note. For example, the major scale is `0b101010110101`.
- A string refers to another name within the same `[scales]` section.
- `.reascale` files can be specified in the form of `{ path = "...", scale = "..." }`.
- The key itself is used as the scale name.
- For defaults, refer to [./reaper_marker_as_key.toml](./reaper_marker_as_key.toml).

```toml
[scales]
major = 0b101010110101
minor = 0b010110101101
maj = "major"
another = { path = "scales/custom.reascale", scale = "Dorian" }
```

## License

This mod is released under the MIT License.
