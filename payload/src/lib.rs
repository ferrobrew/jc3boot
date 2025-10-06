use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use detours_macro::detour;
use re_utilities::{
    ThreadSuspender,
    hook_library::{HookLibraries, HookLibrary},
};
use windows::Win32::{
    Foundation::HINSTANCE,
    System::{
        LibraryLoader::DisableThreadLibraryCalls,
        Memory::{PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtect},
        SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
    },
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
    let hook_libraries = re_utilities::ThreadSuspender::for_block(|| {
        HookLibraries::new([
            denuvo_hook_library(),
            // intro_skip_hook_library(),
            // offline_mode_hook_library(),
        ])
        .enable(&mut patcher)
    })
    .unwrap();
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

fn intro_skip_hook_library() -> HookLibrary {
    HookLibrary::new()
        // CTitleUi::IsIntroMovieComplete
        .with_patch_ret_zero(0x144883F60)
        // CTitleUi::PlayIntroVideo
        .with_immediate_ret(0x1448AB620)
}

fn offline_mode_hook_library() -> HookLibrary {
    HookLibrary::new()
        // CGameInstance::IsOfflineMode
        .with_patch_ret_zero(0x1445A8220)
        // CGameStateFrontend::BeginLogin
        .with_static_binder(&GAME_STATE_FRONTEND_BEGIN_LOGIN_BINDER)
}

#[detour(address = 0x143_D25_650)]
extern "C" fn game_state_frontend_begin_login(this: *mut c_void, _mode: u32) {
    // Always begin login in offline mode
    GAME_STATE_FRONTEND_BEGIN_LOGIN.get().unwrap().call(this, 1);
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
