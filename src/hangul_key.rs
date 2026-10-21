//! Sends the local Korean/English (한/영) key as the peer's own 한/영 key.
//!
//! Each OS has a different 한/영 key: Windows uses VK_HANGUL (also reported by the
//! right Alt key on Korean layouts), Linux uses the Hangul key (KEY_HANGEUL or
//! keysym Hangul), and Macs have no such key, so users remap Caps Lock to F18 and
//! bind F18 to "Select the previous input source". Sent by position, the key
//! arrives as something the peer's input method ignores, or not at all (a Mac has
//! no keycode for Hangul). Between different OSes the key is replaced by the
//! peer's own: F18 for a Mac (the host switches through TIS, see
//! `platform::macos_input_source`), the Hangul key for Linux, and VK_HANGUL for
//! Windows, which is sent as a legacy control key because the Windows host turns
//! map-mode keys into scancodes and only the virtual key toggles the IME.

use base::message_proto::{ControlKey, KeyEvent, KeyboardMode};
use rdev::{Event, EventType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Os {
    Windows,
    Linux,
    MacOS,
}

#[cfg(target_os = "windows")]
const LOCAL_OS: Os = Os::Windows;
#[cfg(target_os = "linux")]
const LOCAL_OS: Os = Os::Linux;
#[cfg(target_os = "macos")]
const LOCAL_OS: Os = Os::MacOS;

const MAC_KVK_F18: u32 = 79;
const WIN_VK_HANGUL: u32 = 0x15;
// VK_KANA shares 0x15 with VK_HANGUL; the Japanese Kana key has scancode 0x70.
const WIN_SCANCODE_KANA: u32 = 0x70;
// KEY_HANGEUL (122) plus the X11 offset of 8.
const LINUX_KEYCODE_HANGUL: u32 = 130;
const LINUX_KEYSYM_HANGUL: u32 = 0xff31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    /// Map/translate mode keycode in the peer's code space.
    Keycode(u32),
    /// Legacy `ControlKey::Hangul`, injected as VK_HANGUL / keysym Hangul by the peer.
    LegacyHangul,
}

fn peer_os(peer: &str) -> Option<Os> {
    match peer {
        "windows" => Some(Os::Windows),
        "linux" => Some(Os::Linux),
        "macos" => Some(Os::MacOS),
        _ => None,
    }
}

/// `position_code`/`platform_code` as rdev reports them: Windows scancode/VK,
/// Linux X keycode/keysym, macOS keycode/keycode.
fn is_local_hangul_key(local: Os, position_code: u32, platform_code: u32) -> bool {
    match local {
        Os::MacOS => platform_code == MAC_KVK_F18,
        Os::Windows => platform_code == WIN_VK_HANGUL && position_code != WIN_SCANCODE_KANA,
        Os::Linux => position_code == LINUX_KEYCODE_HANGUL || platform_code == LINUX_KEYSYM_HANGUL,
    }
}

fn target(local: Os, peer: Os, mode: KeyboardMode) -> Option<Target> {
    if local == peer {
        return None;
    }
    if mode == KeyboardMode::Legacy {
        return Some(Target::LegacyHangul);
    }
    match peer {
        Os::MacOS => Some(Target::Keycode(MAC_KVK_F18)),
        Os::Linux => Some(Target::Keycode(LINUX_KEYCODE_HANGUL)),
        Os::Windows => Some(Target::LegacyHangul),
    }
}

fn convert(
    local: Os,
    peer: &str,
    position_code: u32,
    platform_code: u32,
    down: bool,
    key_event: &KeyEvent,
) -> Option<KeyEvent> {
    if !is_local_hangul_key(local, position_code, platform_code) {
        return None;
    }
    let mode = key_event.mode.enum_value_or(KeyboardMode::Legacy);
    let mut evt = key_event.clone();
    evt.down = down;
    match target(local, peer_os(peer)?, mode)? {
        Target::Keycode(code) => evt.set_chr(code),
        Target::LegacyHangul => {
            evt.mode = KeyboardMode::Legacy.into();
            evt.set_control_key(ControlKey::Hangul);
        }
    }
    Some(evt)
}

/// The key event to send instead of `event` when it is the local 한/영 key and the
/// peer runs another OS; `None` leaves the event to the normal keyboard modes.
pub fn to_peer_hangul_key(peer: &str, event: &Event, key_event: &KeyEvent) -> Option<KeyEvent> {
    use base::config::keys::OPTION_ENABLE_PEER_HANGUL_KEY;
    use hbb_common::config::LocalConfig;

    let down = match event.event_type {
        EventType::KeyPress(..) => true,
        EventType::KeyRelease(..) => false,
        _ => return None,
    };
    if !is_local_hangul_key(LOCAL_OS, event.position_code, event.platform_code) {
        return None;
    }
    if !LocalConfig::get_bool_option(OPTION_ENABLE_PEER_HANGUL_KEY) {
        return None;
    }
    let mut evt = convert(
        LOCAL_OS,
        peer,
        event.position_code,
        event.platform_code,
        down,
        key_event,
    )?;
    if evt.mode.enum_value_or(KeyboardMode::Legacy) == KeyboardMode::Legacy {
        let (alt, ctrl, shift, command) =
            crate::keyboard::client::get_modifiers_state(false, false, false, false);
        crate::keyboard::client::legacy_modifiers(&mut evt, alt, ctrl, shift, command);
    }
    Some(evt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::key_event;

    // (local OS, position code, platform code) of each OS's 한/영 key.
    const MAC_F18: (Os, u32, u32) = (Os::MacOS, 79, 79);
    const WIN_HANGUL: (Os, u32, u32) = (Os::Windows, 0x72, 0x15);
    const WIN_RALT_HANGUL: (Os, u32, u32) = (Os::Windows, 0xE038, 0x15);
    const LINUX_HANGUL: (Os, u32, u32) = (Os::Linux, 130, 0xff31);
    const LINUX_RALT_HANGUL: (Os, u32, u32) = (Os::Linux, 108, 0xff31);

    fn send(key: (Os, u32, u32), peer: &str, mode: KeyboardMode) -> Option<KeyEvent> {
        let mut key_event = KeyEvent::new();
        key_event.mode = mode.into();
        convert(key.0, peer, key.1, key.2, true, &key_event)
    }

    fn keycode(evt: Option<KeyEvent>) -> Option<(KeyboardMode, u32)> {
        let evt = evt?;
        match evt.union {
            Some(key_event::Union::Chr(code)) => Some((evt.mode.enum_value().ok()?, code)),
            _ => None,
        }
    }

    fn is_legacy_hangul(evt: Option<KeyEvent>) -> bool {
        evt.map_or(false, |evt| {
            evt.mode.enum_value() == Ok(KeyboardMode::Legacy)
                && matches!(evt.union, Some(key_event::Union::ControlKey(ck))
                    if ck.enum_value() == Ok(ControlKey::Hangul))
        })
    }

    #[test]
    fn hangul_key_becomes_the_peer_hangul_key() {
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            for key in [WIN_HANGUL, WIN_RALT_HANGUL, LINUX_HANGUL, LINUX_RALT_HANGUL] {
                assert_eq!(keycode(send(key, "macos", mode)), Some((mode, 79)), "{key:?}");
            }
            for key in [MAC_F18, WIN_HANGUL, WIN_RALT_HANGUL] {
                assert_eq!(keycode(send(key, "linux", mode)), Some((mode, 130)), "{key:?}");
            }
            for key in [MAC_F18, LINUX_HANGUL, LINUX_RALT_HANGUL] {
                assert!(is_legacy_hangul(send(key, "windows", mode)), "{key:?}");
            }
        }
        for (key, peer) in [(MAC_F18, "windows"), (WIN_HANGUL, "macos"), (LINUX_HANGUL, "macos")] {
            assert!(is_legacy_hangul(send(key, peer, KeyboardMode::Legacy)), "{key:?}");
        }
    }

    #[test]
    fn same_os_and_other_keys_are_unchanged() {
        for mode in [KeyboardMode::Map, KeyboardMode::Translate, KeyboardMode::Legacy] {
            assert!(send(MAC_F18, "macos", mode).is_none());
            assert!(send(WIN_HANGUL, "windows", mode).is_none());
            assert!(send(LINUX_HANGUL, "linux", mode).is_none());
            assert!(send(MAC_F18, "android", mode).is_none());
            // Japanese Kana (VK_KANA = 0x15), macOS F19, Linux Hanja.
            assert!(send((Os::Windows, 0x70, 0x15), "macos", mode).is_none());
            assert!(send((Os::MacOS, 80, 80), "windows", mode).is_none());
            assert!(send((Os::Linux, 131, 0xff34), "macos", mode).is_none());
        }
    }

    #[test]
    fn key_up_follows_key_down() {
        let mut key_event = KeyEvent::new();
        key_event.mode = KeyboardMode::Map.into();
        let up = convert(Os::Windows, "macos", 0x72, 0x15, false, &key_event);
        assert_eq!(up.map(|e| (e.down, e.chr())), Some((false, 79)));
    }
}
