//! Korean/English input-source switching for keys that come from a remote peer.
//!
//! macOS only runs input-source hotkeys (e.g. Caps Lock remapped to F18 and bound
//! to "Select the previous input source") for hardware events, so the synthetic
//! key RustDesk posts never switches 한/영. Keys that mean "switch Korean/English"
//! are taken out of the input stream here and switch the input source through TIS.

use base::message_proto::{key_event, ControlKey, KeyEvent, KeyboardMode};
use std::sync::Mutex;

static SWALLOWED_KEYS: Mutex<Vec<ToggleKey>> = Mutex::new(Vec::new());
static SUSPENDED_HOTKEYS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// Symbolic hotkeys "Select the previous / next input source" (F18 for 한/영).
const INPUT_SOURCE_HOTKEYS: [i32; 2] = [60, 61];

extern "C" {
    fn MacToggleKoreanInputSource() -> bool;
    fn CGSIsSymbolicHotKeyEnabled(hot_key: i32) -> bool;
    fn CGSSetSymbolicHotKeyEnabled(hot_key: i32, enabled: bool) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToggleKey {
    /// Map/translate mode, macOS virtual keycode.
    Keycode(u32),
    /// Legacy mode, `ControlKey::Hangul`.
    Hangul,
}

/// F18 is what Caps Lock is usually remapped to for 한/영; Lang1 (kVK_JIS_Kana)
/// is where USB HID LANG1 (the Hangul key) lands on macOS. A Windows or Linux
/// Hangul key has no macOS keycode, so it only arrives in legacy mode, as
/// `ControlKey::Hangul`.
fn toggle_key(evt: &KeyEvent) -> Option<ToggleKey> {
    const KVK_F18: u32 = 79;
    const KVK_JIS_KANA: u32 = 104;
    match (evt.mode.enum_value_or(KeyboardMode::Legacy), &evt.union) {
        (KeyboardMode::Map | KeyboardMode::Translate, Some(key_event::Union::Chr(code))) => {
            let code = code & 0x0000FFFF;
            (code == KVK_F18 || code == KVK_JIS_KANA).then_some(ToggleKey::Keycode(code))
        }
        (KeyboardMode::Legacy, Some(key_event::Union::ControlKey(ck)))
            if ck.enum_value() == Ok(ControlKey::Hangul) =>
        {
            Some(ToggleKey::Hangul)
        }
        _ => None,
    }
}

/// Returns true when `evt` was consumed as a Korean/English switch and must not
/// be simulated. Must run on the main thread.
///
/// The key-up of a consumed key-down is consumed too, and key-downs repeated
/// while the key is held switch only once. When the option is off or no Korean
/// input source is enabled, the key goes through unchanged.
pub fn handle_toggle_key(evt: &KeyEvent) -> bool {
    use base::config::keys::OPTION_ENABLE_INPUT_SOURCE_TOGGLE;
    use hbb_common::config::Config;

    let Some(key) = toggle_key(evt) else {
        return false;
    };
    let mut swallowed = SWALLOWED_KEYS.lock().unwrap();
    let held = swallowed.iter().position(|k| *k == key);
    if !evt.down {
        return match held {
            Some(i) => {
                swallowed.swap_remove(i);
                true
            }
            None => false,
        };
    }
    if held.is_some() {
        return true;
    }
    if !Config::get_bool_option(OPTION_ENABLE_INPUT_SOURCE_TOGGLE) {
        return false;
    }
    if !unsafe { MacToggleKoreanInputSource() } {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(60),
            warn,
            "No Korean input source to switch to; sending the key as is"
        );
        return false;
    }
    swallowed.push(key);
    true
}

/// Forgets held toggle keys whose key-up will never come (peer disconnected).
pub fn reset_toggle_keys() {
    SWALLOWED_KEYS.lock().unwrap().clear();
}

/// Controlling side: turns the input-source hotkeys off while a remote window
/// holds the keyboard, and back on when it lets go.
///
/// The window server matches symbolic hotkeys before the session event tap that
/// grabs keys for the remote side, so F18 (Caps Lock) would switch the local
/// input source as well as the remote one. Only hotkeys that were on are turned
/// off and only those are turned back on. Known limit: if RustDesk dies while
/// it holds the keyboard they stay off until the next grab and release, or
/// until the user logs out.
pub fn suspend_input_source_hotkeys(suspend: bool) {
    use base::config::keys::OPTION_ENABLE_PEER_HANGUL_KEY;
    use hbb_common::config::LocalConfig;

    let mut suspended = SUSPENDED_HOTKEYS.lock().unwrap();
    if !suspend {
        for hot_key in suspended.drain(..) {
            unsafe { CGSSetSymbolicHotKeyEnabled(hot_key, true) };
        }
        return;
    }
    if !LocalConfig::get_bool_option(OPTION_ENABLE_PEER_HANGUL_KEY) {
        return;
    }
    for hot_key in INPUT_SOURCE_HOTKEYS {
        if unsafe { CGSIsSymbolicHotKeyEnabled(hot_key) }
            && unsafe { CGSSetSymbolicHotKeyEnabled(hot_key, false) } == 0
        {
            suspended.push(hot_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(mode: KeyboardMode, union: key_event::Union) -> KeyEvent {
        let mut evt = KeyEvent::new();
        evt.mode = mode.into();
        evt.union = Some(union);
        evt
    }

    #[test]
    fn f18_and_lang1_switch_in_map_and_translate_mode() {
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            for code in [79, 104] {
                let evt = key_event(mode, key_event::Union::Chr(code));
                assert_eq!(toggle_key(&evt), Some(ToggleKey::Keycode(code)));
            }
        }
    }

    #[test]
    fn hangul_control_key_switches_in_legacy_mode() {
        let evt = key_event(
            KeyboardMode::Legacy,
            key_event::Union::ControlKey(ControlKey::Hangul.into()),
        );
        assert_eq!(toggle_key(&evt), Some(ToggleKey::Hangul));
    }

    #[test]
    fn other_keys_are_simulated() {
        let cases = [
            // F17, F19 and kVK_JIS_Eisu (Lang2) keep their meaning.
            key_event(KeyboardMode::Map, key_event::Union::Chr(64)),
            key_event(KeyboardMode::Map, key_event::Union::Chr(80)),
            key_event(KeyboardMode::Map, key_event::Union::Chr(102)),
            // In legacy mode a Chr is a character, not a keycode: 'O' is 79.
            key_event(KeyboardMode::Legacy, key_event::Union::Chr(79)),
            key_event(
                KeyboardMode::Legacy,
                key_event::Union::ControlKey(ControlKey::Hanja.into()),
            ),
            key_event(KeyboardMode::Translate, key_event::Union::Seq("한".to_owned())),
        ];
        for evt in cases {
            assert_eq!(toggle_key(&evt), None, "{:?}", evt);
        }
    }
}
