use std::sync::{Mutex, OnceLock};

use detours_macro::detour;
use re_utilities::{
    ThreadSuspender,
    hook_library::{HookLibraries, HookLibrary},
};

use crate::util::{self, HookLibraryExt as _};

static POSTINIT_HOOK_LIBRARIES: OnceLock<PostInitHookLibraries> = OnceLock::new();
struct PostInitHookLibraries {
    _patcher: Mutex<re_utilities::Patcher>,
    _hook_libraries: HookLibraries,
}
// Only used from the main thread, so it's fine to be Send/Sync
unsafe impl Send for PostInitHookLibraries {}
unsafe impl Sync for PostInitHookLibraries {}

pub fn install() {
    let mut patcher = re_utilities::Patcher::new();
    let hook_libraries = ThreadSuspender::for_block(|| {
        HookLibraries::new([intro_skip_hook_library(), offline_mode_hook_library()])
            .enable(&mut patcher)
    });
    let hook_libraries = match hook_libraries {
        Ok(hook_libraries) => hook_libraries,
        Err(err) => {
            util::message_box("Error in jc3boot hooks", &err.to_string());
            return;
        }
    };
    let _ = POSTINIT_HOOK_LIBRARIES.set(PostInitHookLibraries {
        _patcher: Mutex::new(patcher),
        _hook_libraries: hook_libraries,
    });
}

pub fn uninstall() {
    let pl = POSTINIT_HOOK_LIBRARIES.get().unwrap();
    let _ = ThreadSuspender::for_block(|| {
        pl._hook_libraries
            .set_enabled(&mut pl._patcher.lock().unwrap(), false)
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
extern "C" fn login_manager_login(this: *mut std::ffi::c_void, _mode: u32) -> bool {
    // Always login in offline mode
    LOGIN_MANAGER_LOGIN.get().unwrap().call(this, 1)
}
