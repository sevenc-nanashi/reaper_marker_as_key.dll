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
    ffi::{CStr, CString, c_int, c_void},
    ptr::{null, null_mut},
};

const ACTION: &CStr = c"reaper_marker_as_key.dll: Set project key signatures from markers";

struct Plugin {
    api: Api,
    gaccel: Gaccel,
    running: Cell<bool>,
}

// REAPER loads extensions and invokes Main actions on its main thread.
thread_local! {
    static PLUGIN: RefCell<Option<Box<Plugin>>> = const { RefCell::new(None) };
}

struct Change {
    position: f64,
    key: Key,
    previous: Option<Key>,
}

impl Api {
    unsafe fn collect_changes(&self, project: Project) -> anyhow::Result<Vec<Change>> {
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
            if let Some(key) = key::parse(name)? {
                markers.push((position, key));
            }
        }
        markers.sort_by(|a, b| a.0.total_cmp(&b.0));
        if markers.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            anyhow::bail!("Multiple key markers at the same position.");
        }
        let mut changes = Vec::new();
        for (time, key) in markers {
            let position = unsafe { self.public.TimeMap2_timeToQN(project, time) };
            let mut previous_position = position;
            let mut previous = Key::default();
            let exists = unsafe {
                (self.key_signature)(
                    project,
                    &mut previous_position,
                    &mut previous.root,
                    &mut previous.accidental,
                    &mut previous.scale,
                    false,
                )
            };
            let previous = (exists && previous_position == position).then_some(previous);
            if previous != Some(key) {
                changes.push(Change {
                    position,
                    key,
                    previous,
                });
            }
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

    unsafe fn apply_changes(
        &self,
        project: Project,
        changes: &[Change],
    ) -> Vec<anyhow::Result<()>> {
        changes
            .iter()
            .map(|change| {
                if unsafe { self.set_key(project, change.position, Some(change.key)) } {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Could not set key signature at position {}.",
                        change.position
                    ))
                }
            })
            .collect()
    }

    unsafe fn run(&self) -> anyhow::Result<()> {
        let project = unsafe { self.public.EnumProjects(-1, null_mut(), 0) };
        anyhow::ensure!(!project.is_null(), "No project is open.");
        let changes = unsafe { self.collect_changes(project) }?;
        if changes.is_empty() {
            return Ok(());
        }

        unsafe { self.public.Undo_BeginBlock2(project) };

        let result = unsafe { self.apply_changes(project, &changes) };
        let mut restored = true;
        for (change, result) in changes.iter().zip(&result) {
            if result.is_err() {
                restored &= unsafe { self.set_key(project, change.position, change.previous) };
            }
        }
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
        let errors: Vec<String> = result
            .into_iter()
            .filter_map(|result| result.err().map(|error| error.to_string()))
            .collect();
        if !errors.is_empty() {
            anyhow::bail!("{} errors occurred:\n{}", errors.len(), errors.join("\n"));
        }
        if !restored {
            anyhow::bail!("Some changes could not be restored after an error.");
        }
        Ok(())
    }

    fn report(&self, message: &str) {
        let message = CString::new(message).expect("error messages contain no NUL");
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
        if command != c_int::from(plugin.gaccel.accel.cmd) {
            return false;
        }
        if plugin.running.replace(true) {
            return true;
        }
        let result = unsafe { plugin.api.run() };
        if let Err(error) = result {
            plugin.api.report(&format!("Error: {error}"));
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
                    plugin.api.public.plugin_register(
                        c"-gaccel".as_ptr(),
                        (&mut plugin.gaccel as *mut Gaccel).cast(),
                    );
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
        let public = api.public;
        let command = unsafe {
            public.plugin_register(
                c"command_id".as_ptr(),
                c"SNMarkerAsKey".as_ptr().cast_mut().cast(),
            )
        };
        let Ok(command @ 1..) = u16::try_from(command) else {
            return 0;
        };
        let mut plugin = Box::new(Plugin {
            api,
            gaccel: Gaccel {
                accel: Accel {
                    fVirt: 0,
                    key: 0,
                    cmd: command,
                },
                desc: ACTION.as_ptr(),
            },
            running: Cell::new(false),
        });
        let gaccel = (&mut plugin.gaccel as *mut Gaccel).cast();
        unsafe {
            if public.plugin_register(c"gaccel".as_ptr(), gaccel) == 0 {
                return 0;
            }
            if public.plugin_register(c"hookcommand".as_ptr(), hook_command as *mut c_void) == 0 {
                public.plugin_register(c"-gaccel".as_ptr(), gaccel);
                return 0;
            }
        }
        *slot = Some(plugin);
        1
    })
}
