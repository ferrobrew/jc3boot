use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use detours_macro::detour;
use re_utilities::{
    ThreadSuspender,
    hook_library::{HookLibraries, HookLibrary},
    retour::GenericDetour,
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND},
        System::{
            LibraryLoader::{DisableThreadLibraryCalls, GetModuleHandleA, GetProcAddress},
            Memory::{PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtect},
            SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
        },
        UI::WindowsAndMessaging::{HMENU, MB_OK, MessageBoxW},
    },
    core::{HSTRING, PCSTR, s},
};

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(module: HINSTANCE, reason: u32, _unk: *mut c_void) -> bool {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(module).ok();
        };
    } else if reason == DLL_PROCESS_DETACH {
        uninstall();
    }
    true
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn run(_: *mut c_void) {
    install();
}

static STARTUP_HOOK_LIBRARIES: OnceLock<StartupHookLibraries> = OnceLock::new();
struct StartupHookLibraries {
    patcher: Mutex<re_utilities::Patcher>,
    hook_libraries: HookLibraries,
}
// Only used upon DLL load/unload, so it's fine to be Send/Sync
unsafe impl Send for StartupHookLibraries {}
unsafe impl Sync for StartupHookLibraries {}

pub fn install() {
    // Unprotect everything from the start of .code to the end of .xtext
    let start_addr = 0x1431D4000usize;
    let end_addr = 0x1431D8000usize + 0x6C36000;

    let mut old: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS::default();
    unsafe {
        VirtualProtect(
            start_addr as *mut c_void,
            end_addr - start_addr,
            PAGE_EXECUTE_READWRITE,
            &mut old,
        )
        .unwrap();
    }

    let mut patcher = re_utilities::Patcher::new();
    let hook_libraries = ThreadSuspender::for_block(|| {
        HookLibraries::new([denuvo_hook_library(), create_window_ex_a_hook_library()])
            .enable(&mut patcher)
    });
    let hook_libraries = match hook_libraries {
        Ok(hook_libraries) => hook_libraries,
        Err(err) => {
            message_box("Error in jc3boot hooks", &err.to_string());
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
        shl.hook_libraries
            .set_enabled(&mut shl.patcher.lock().unwrap(), false)
    });
}

fn denuvo_hook_library() -> HookLibrary {
    HookLibrary::new()
        // Debug Registers
        .with_patch_ret_one(0x145D8DB00)
        // RtlCreateQueryDebugBuffer
        .with_patch_ret_zero(0x145D8DC30)
        // UmsInfo
        .with_patch_ret_one(0x145D8DCC0)
        // AlignmentFaultFixup
        .with_patch_ret_zero(0x145D8DD20)
        // NtGlobalFlag
        .with_patch_ret_zero(0x145D8DEB0)
        // Denuvo::ThreadHideFromDebugger2
        .with_patch_ret_zero(0x145D8DEF0)
        // Denuvo::ThreadHideFromDebugger
        .with_patch_ret_zero(0x145D8DF20)
        // DbgUiRemoteBreakin
        .with_patch_ret_one(0x145D8DF80)
        // DbgUiIssueRemoteBreakin
        .with_patch_ret_one(0x145D8E060)
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
        install_postinit();
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

static POSTINIT_HOOK_LIBRARIES: OnceLock<PostInitHookLibraries> = OnceLock::new();
struct PostInitHookLibraries {
    _patcher: Mutex<re_utilities::Patcher>,
    _hook_libraries: HookLibraries,
}
// Only used upon DLL load/unload, so it's fine to be Send/Sync
unsafe impl Send for PostInitHookLibraries {}
unsafe impl Sync for PostInitHookLibraries {}

pub fn install_postinit() {
    let mut patcher = re_utilities::Patcher::new();
    let hook_libraries = ThreadSuspender::for_block(|| {
        HookLibraries::new([intro_skip_hook_library(), offline_mode_hook_library()])
            .enable(&mut patcher)
    });
    let hook_libraries = match hook_libraries {
        Ok(hook_libraries) => hook_libraries,
        Err(err) => {
            message_box("Error in jc3boot hooks", &err.to_string());
            return;
        }
    };
    let _ = POSTINIT_HOOK_LIBRARIES.set(PostInitHookLibraries {
        _patcher: Mutex::new(patcher),
        _hook_libraries: hook_libraries,
    });
}

fn intro_skip_hook_library() -> HookLibrary {
    HookLibrary::new()
        // CTitleUi::IsIntroMovieComplete
        .with_patch_ret_one(0x144883F60)
        // CTitleUi::PlayIntroVideo
        .with_immediate_ret(0x1448AB620)
}

fn offline_mode_hook_library() -> HookLibrary {
    HookLibrary::new()
        // CLoginManager::Login
        .with_static_binder(&LOGIN_MANAGER_LOGIN_BINDER)
}

#[detour(address = 0x143_EC4_320)]
extern "C" fn login_manager_login(this: *mut c_void, _mode: u32) -> bool {
    // Always login in offline mode
    LOGIN_MANAGER_LOGIN.get().unwrap().call(this, 1)
}

trait HookLibraryExt {
    fn with_immediate_ret(self, address: usize) -> Self;
    fn with_patch_ret_zero(self, address: usize) -> Self;
    fn with_patch_ret_one(self, address: usize) -> Self;
}
impl HookLibraryExt for HookLibrary {
    fn with_immediate_ret(self, address: usize) -> Self {
        self.with_patch(address, &[0xC3])
    }
    fn with_patch_ret_zero(self, address: usize) -> Self {
        self.with_patch(address, &[0x48, 0x31, 0xC0, 0xC3])
    }
    fn with_patch_ret_one(self, address: usize) -> Self {
        self.with_patch(address, &[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, 0xC3])
    }
}

fn message_box(title: &str, message: &str) {
    unsafe {
        MessageBoxW(None, &HSTRING::from(message), &HSTRING::from(title), MB_OK);
    }
}
