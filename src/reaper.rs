use reaper_low::{PluginContext, Reaper, raw};
use std::ffi::{c_int, c_void};

pub type Project = *mut raw::ReaProject;

// Non-public API, absent from reaper-rs. ABI inherited from the Zig implementation.
type GetSetKeySignature =
    unsafe extern "C" fn(Project, *mut f64, *mut i8, *mut i8, *mut c_int, bool) -> bool;

pub struct Api {
    pub public: Reaper,
    pub key_signature: GetSetKeySignature,
}

impl Api {
    pub unsafe fn load(context: PluginContext) -> Option<Self> {
        let pointer = unsafe { context.GetFunc(c"GetSetProjKeySig".as_ptr()) };
        if pointer.is_null() {
            return None;
        }
        let public = Reaper::load(context);
        let functions = public.pointers();
        functions.plugin_register?;
        functions.EnumProjects?;
        functions.EnumProjectMarkers3?;
        functions.TimeMap2_timeToQN?;
        functions.Undo_BeginBlock2?;
        functions.Undo_EndBlock2?;
        functions.MarkProjectDirty?;
        functions.UpdateArrange?;
        functions.MIDIEditor_GetActive?;
        functions.MIDIEditor_GetTake?;
        functions.MIDI_RefreshEditors?;
        functions.ShowMessageBox?;
        Some(Self {
            public,
            key_signature: unsafe {
                std::mem::transmute::<*mut c_void, GetSetKeySignature>(pointer)
            },
        })
    }
}
