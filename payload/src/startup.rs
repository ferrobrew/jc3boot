use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use crate::{postinit, util};
use re_utilities::{
    ThreadSuspender,
    hook_library::{HookLibraries, HookLibrary},
    retour::GenericDetour,
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND},
        System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        UI::WindowsAndMessaging::HMENU,
    },
    core::{PCSTR, s},
};

static STARTUP_HOOK_LIBRARIES: OnceLock<StartupHookLibraries> = OnceLock::new();
struct StartupHookLibraries {
    patcher: Mutex<re_utilities::Patcher>,
    hook_libraries: HookLibraries,
}
// Only used upon DLL load/unload, so it's fine to be Send/Sync
unsafe impl Send for StartupHookLibraries {}
unsafe impl Sync for StartupHookLibraries {}

pub fn install() {
    let mut patcher = re_utilities::Patcher::new();
    let hook_libraries = ThreadSuspender::for_block(|| {
        Ok(HookLibraries::new([create_window_ex_a_hook_library()]).enable(&mut patcher)?)
    });
    let hook_libraries = match hook_libraries {
        Ok(hook_libraries) => hook_libraries,
        Err(err) => {
            util::message_box("Error in jc3boot hooks", &err.to_string());
            return;
        }
    };
    let _ = STARTUP_HOOK_LIBRARIES.set(StartupHookLibraries {
        patcher: Mutex::new(patcher),
        hook_libraries,
    });
}

pub fn uninstall() {
    let shl = STARTUP_HOOK_LIBRARIES.get().unwrap();
    let _ = ThreadSuspender::for_block(|| {
        Ok(shl
            .hook_libraries
            .set_enabled(&mut shl.patcher.lock().unwrap(), false)?)
    });
}

type CreateWindowExASignature = unsafe extern "C" fn(
    u32,
    PCSTR,
    PCSTR,
    u32,
    i32,
    i32,
    i32,
    i32,
    HWND,
    HMENU,
    HINSTANCE,
    *mut c_void,
) -> HWND;
static CREATE_WINDOW_EX_A: OnceLock<GenericDetour<CreateWindowExASignature>> = OnceLock::new();

fn create_window_ex_a_hook_library() -> HookLibrary {
    HookLibrary::new()
        // CreateWindowExA
        .with_callbacks(
            || unsafe {
                if CREATE_WINDOW_EX_A.get().is_none() {
                    let module = GetModuleHandleA(s!("user32.dll"))?;
                    let address = GetProcAddress(module, s!("CreateWindowExA"));

                    #[allow(clippy::missing_transmute_annotations)]
                    let detour = GenericDetour::<CreateWindowExASignature>::new(
                        std::mem::transmute(address),
                        create_window_ex_a_hook,
                    )?;
                    CREATE_WINDOW_EX_A
                        .set(detour)
                        .expect("detour already bound");
                }

                Ok(CREATE_WINDOW_EX_A.get().unwrap().enable()?)
            },
            || unsafe {
                CREATE_WINDOW_EX_A.get().unwrap().disable()?;
                Ok(())
            },
        )
}

extern "C" fn create_window_ex_a_hook(
    dw_ex_style: u32,
    lp_class_name: PCSTR,
    lp_window_name: PCSTR,
    dw_style: u32,
    x: i32,
    y: i32,
    n_width: i32,
    n_height: i32,
    h_wnd_parent: HWND,
    h_menu: HMENU,
    h_instance: HINSTANCE,
    lp_param: *mut c_void,
) -> HWND {
    if unsafe { lp_class_name.to_string() } == Ok("JC3".to_string()) {
        postinit::install();
    }

    unsafe {
        CREATE_WINDOW_EX_A.get().unwrap().call(
            dw_ex_style,
            lp_class_name,
            lp_window_name,
            dw_style,
            x,
            y,
            n_width,
            n_height,
            h_wnd_parent,
            h_menu,
            h_instance,
            lp_param,
        )
    }
}
