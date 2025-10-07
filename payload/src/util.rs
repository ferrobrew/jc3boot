use re_utilities::hook_library::HookLibrary;
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW},
    core::HSTRING,
};

pub trait HookLibraryExt {
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

pub fn message_box(title: &str, message: &str) {
    unsafe {
        MessageBoxW(None, &HSTRING::from(message), &HSTRING::from(title), MB_OK);
    }
}
