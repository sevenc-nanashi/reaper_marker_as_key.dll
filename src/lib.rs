mod config;
mod key;
mod reaper;

use key::Key;
use reaper::{Api, Project};
use reaper_low::{
    PluginContext, StaticPluginContext,
    raw::{
        ACCEL as Accel, HINSTANCE, gaccel_register_t as Gaccel, reaper_plugin_info_t as PluginInfo,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::{CStr, CString, c_int, c_void},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};

const ACTION: &CStr = c"reaper_marker_as_key.dll: Set project key signatures from markers";
const CONFIG_ACTION: &CStr = c"reaper_marker_as_key.dll: Open configuration";

struct Plugin {
    api: Api,
    gaccels: [Gaccel; 2],
    config_path: PathBuf,
    running: Cell<bool>,
}

// REAPER loads extensions and invokes Main actions on its main thread.
thread_local! {
    static PLUGIN: RefCell<Option<Box<Plugin>>> = const { RefCell::new(None) };
}

struct Change {
    position: f64,
    key: Option<Key>,
    previous: Option<Key>,
}

impl Api {
    unsafe fn collect_changes(
        &self,
        project: Project,
        scales: &BTreeMap<String, i32>,
    ) -> anyhow::Result<Vec<Change>> {
        let mut markers = Vec::new();
        for index in 0..c_int::MAX {
            let (mut region, mut position, mut end, mut name, mut number, mut color) =
                (false, 0.0, 0.0, null(), 0, 0);
            let found = unsafe {
                self.public.EnumProjectMarkers3(
                    project,
                    index,
                    &mut region,
                    &mut position,
                    &mut end,
                    &mut name,
                    &mut number,
                    &mut color,
                )
            };
            if found == 0 {
                break;
            }
            let name = unsafe { CStr::from_ptr(name) }.to_str()?;
            if let Some(key) = key::parse(name, scales)? {
                markers.push((position, key));
            }
        }
        markers.sort_by(|a, b| a.0.total_cmp(&b.0));
        if markers.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            anyhow::bail!("Multiple key markers at the same position.");
        }
        let mut changes = Vec::new();
        // The API returns the last key signature at or before the requested position.
        let mut position = f64::MAX;
        loop {
            let upper_bound = position;
            let mut previous = Key::default();
            let exists = unsafe {
                (self.key_signature)(
                    project,
                    &mut position,
                    &mut previous.root,
                    &mut previous.accidental,
                    &mut previous.scale,
                    false,
                )
            };
            if !exists {
                break;
            }
            anyhow::ensure!(
                position.is_finite() && position <= upper_bound,
                "Could not enumerate existing key signatures."
            );
            changes.push(Change {
                position,
                key: None,
                previous: Some(previous),
            });
            position = position.next_down();
        }
        for (time, key) in markers {
            changes.push(Change {
                position: unsafe { self.public.TimeMap2_timeToQN(project, time) },
                key: Some(key),
                previous: None,
            });
        }
        Ok(changes)
    }

    unsafe fn set_key(&self, project: Project, mut position: f64, key: Option<Key>) -> bool {
        unsafe {
            match key {
                Some(mut key) => (self.key_signature)(
                    project,
                    &mut position,
                    &mut key.root,
                    &mut key.accidental,
                    &mut key.scale,
                    true,
                ),
                None => (self.key_signature)(
                    project,
                    &mut position,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    true,
                ),
            }
        }
    }

    unsafe fn apply_changes(&self, project: Project, changes: &[Change]) -> anyhow::Result<()> {
        for (index, change) in changes.iter().enumerate() {
            if !unsafe { self.set_key(project, change.position, change.key) } {
                let mut restored = true;
                for change in changes[..=index].iter().rev() {
                    restored &= unsafe { self.set_key(project, change.position, change.previous) };
                }
                anyhow::ensure!(
                    restored,
                    "Could not replace key signatures at position {}. Restoring the original key signatures also failed; use Undo to recover.",
                    change.position
                );
                anyhow::bail!(
                    "Could not replace key signatures at position {}. Original key signatures were restored.",
                    change.position
                );
            }
        }
        Ok(())
    }

    unsafe fn run(&self, config_path: &Path) -> anyhow::Result<()> {
        let scales = config::load(config_path)?;
        let project = unsafe { self.public.EnumProjects(-1, null_mut(), 0) };
        anyhow::ensure!(!project.is_null(), "No project is open.");
        let changes = unsafe { self.collect_changes(project, &scales) }?;
        if changes.is_empty() {
            unsafe {
                self.public
                    .Help_Set(c"No key signature changes needed".as_ptr(), true);
            }
            return Ok(());
        }

        unsafe { self.public.Undo_BeginBlock2(project) };

        let result = unsafe { self.apply_changes(project, &changes) };
        unsafe {
            self.public.MarkProjectDirty(project);
            self.public.Undo_EndBlock2(project, ACTION.as_ptr(), -1);
            self.public.UpdateArrange();
            let editor = self.public.MIDIEditor_GetActive();
            if !editor.is_null() {
                let take = self.public.MIDIEditor_GetTake(editor);
                if !take.is_null() {
                    self.public.MIDI_RefreshEditors(take);
                }
            }
        }
        result?;
        let applied = changes.iter().filter(|change| change.key.is_some()).count();
        let message = format!(
            "Removed {} key signatures and applied {} from markers.",
            changes.len() - applied,
            applied
        );
        unsafe {
            let message = CString::new(message)?;
            self.public.Help_Set(message.as_ptr(), true);
        }
        Ok(())
    }

    fn report(&self, message: &str) {
        let message = CString::new(message.replace('\0', "\\0")).expect("NULs escaped");
        unsafe {
            self.public
                .ShowMessageBox(message.as_ptr(), ACTION.as_ptr(), 0);
        }
    }
}

unsafe extern "C" fn hook_command(command: c_int, _: c_int) -> bool {
    PLUGIN.with(|slot| {
        let slot = slot.borrow();
        let Some(plugin) = slot.as_ref() else {
            return false;
        };
        let Some(index) = plugin
            .gaccels
            .iter()
            .position(|gaccel| command == c_int::from(gaccel.accel.cmd))
        else {
            return false;
        };
        if plugin.running.replace(true) {
            return true;
        }
        let result = match index {
            0 => unsafe { plugin.api.run(&plugin.config_path) },
            1 => config::open(&plugin.config_path),
            _ => unreachable!("registered action index"),
        };
        if let Err(error) = result {
            plugin.api.report(&format!("Error: {error:#}"));
        }
        plugin.running.set(false);
        true
    })
}

/// # Safety
/// Called by REAPER on its main thread with a valid SDK info pointer, or null on unload.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ReaperPluginEntry(instance: HINSTANCE, info: *mut PluginInfo) -> c_int {
    unsafe { entry(instance, info) }
}

unsafe fn entry(instance: HINSTANCE, info: *mut PluginInfo) -> c_int {
    PLUGIN.with(|slot| {
        let mut slot = slot.borrow_mut();
        if info.is_null() {
            if let Some(mut plugin) = slot.take() {
                unsafe {
                    plugin
                        .api
                        .public
                        .plugin_register(c"-hookcommand".as_ptr(), hook_command as *mut c_void);
                    for gaccel in &mut plugin.gaccels {
                        plugin
                            .api
                            .public
                            .plugin_register(c"-gaccel".as_ptr(), (gaccel as *mut Gaccel).cast());
                    }
                }
            }
            return 0;
        }
        if slot.is_some() {
            return 0;
        }
        let info = unsafe { *info };
        let Ok(context) = (unsafe {
            PluginContext::from_extension_plugin(
                instance,
                info,
                StaticPluginContext {
                    h_instance: instance,
                    get_swell_func: None,
                },
            )
        }) else {
            return 0;
        };
        let Some(api) = (unsafe { Api::load(context) }) else {
            return 0;
        };
        let config_path = match config::path() {
            Ok(path) => path,
            Err(error) => {
                api.report(&format!("Error: {error:#}"));
                return 0;
            }
        };
        if let Err(error) = config::ensure_file(&config_path) {
            api.report(&format!("Error: {error:#}"));
        }
        let public = api.public;
        let mut gaccels = [Gaccel::default(); 2];
        for (gaccel, (id, description)) in gaccels.iter_mut().zip([
            (c"SNMarkerAsKey", ACTION),
            (c"SNMarkerAsKeyConfig", CONFIG_ACTION),
        ]) {
            let command = unsafe {
                public.plugin_register(c"command_id".as_ptr(), id.as_ptr().cast_mut().cast())
            };
            let Ok(command @ 1..) = u16::try_from(command) else {
                return 0;
            };
            *gaccel = Gaccel {
                accel: Accel {
                    fVirt: 0,
                    key: 0,
                    cmd: command,
                },
                desc: description.as_ptr(),
            };
        }
        let mut plugin = Box::new(Plugin {
            api,
            gaccels,
            config_path,
            running: Cell::new(false),
        });
        unsafe {
            for index in 0..plugin.gaccels.len() {
                if public.plugin_register(
                    c"gaccel".as_ptr(),
                    (&mut plugin.gaccels[index] as *mut Gaccel).cast(),
                ) == 0
                {
                    for gaccel in &mut plugin.gaccels[..index] {
                        public.plugin_register(c"-gaccel".as_ptr(), (gaccel as *mut Gaccel).cast());
                    }
                    return 0;
                }
            }
            if public.plugin_register(c"hookcommand".as_ptr(), hook_command as *mut c_void) == 0 {
                for gaccel in &mut plugin.gaccels {
                    public.plugin_register(c"-gaccel".as_ptr(), (gaccel as *mut Gaccel).cast());
                }
                return 0;
            }
        }
        *slot = Some(plugin);
        1
    })
}
