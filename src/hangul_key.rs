//! Sends the local Korean/English (한/영) key as the peer's own 한/영 key.
//!
//! Each OS has a different 한/영 key: Windows uses VK_HANGUL (also reported by the
//! right Alt key on Korean layouts), Linux uses the Hangul key (KEY_HANGEUL or
//! keysym Hangul), and a Mac uses whatever its "Select the previous / next input
//! source" shortcuts are (F18 when Caps Lock is remapped, ⌃Space by default),
//! plus the LANG1 key. Sent by position, the key arrives as something the peer's
//! input method ignores, or not at all (a Mac has no keycode for Hangul). The key
//! is replaced by the peer's own: F18 for a Mac (the host switches through TIS,
//! see `platform::macos_input_source`), the Hangul key for Linux, and VK_HANGUL
//! for Windows, which is sent as a legacy control key because the Windows host
//! turns map-mode keys into scancodes and only the virtual key toggles the IME.

// The chord code is used on macOS and the single-key code on Windows/Linux; both
// are compiled everywhere so their tests run on every platform.
#![allow(dead_code)]

use base::message_proto::{ControlKey, KeyEvent, KeyboardMode};
use rdev::{Event, EventType};
use std::sync::Mutex;

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
const MAC_KVK_SPACE: u32 = 49;
const MAC_KVK_JIS_KANA: u32 = 104;
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
/// Linux X keycode/keysym. A Mac's keys are chords, see [`Chord`].
fn is_local_hangul_key(local: Os, position_code: u32, platform_code: u32) -> bool {
    match local {
        Os::MacOS => platform_code == MAC_KVK_F18,
        Os::Windows => platform_code == WIN_VK_HANGUL && position_code != WIN_SCANCODE_KANA,
        Os::Linux => position_code == LINUX_KEYCODE_HANGUL || platform_code == LINUX_KEYSYM_HANGUL,
    }
}

/// The key to send, or `None` to send the local key unchanged. Between Macs a
/// chord such as ⌃Space still becomes F18, which the remote Mac handles itself.
fn target(local: Os, peer: Os, mode: KeyboardMode) -> Option<Target> {
    if local == peer && local != Os::MacOS {
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

fn peer_toggle_event(local: Os, peer: Os, down: bool, key_event: &KeyEvent) -> Option<KeyEvent> {
    let mode = key_event.mode.enum_value_or(KeyboardMode::Legacy);
    let mut evt = key_event.clone();
    evt.down = down;
    match target(local, peer, mode)? {
        Target::Keycode(code) => evt.set_chr(code),
        Target::LegacyHangul => {
            evt.mode = KeyboardMode::Legacy.into();
            evt.set_control_key(ControlKey::Hangul);
        }
    }
    Some(evt)
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
    peer_toggle_event(local, peer_os(peer)?, down, key_event)
}

/// Modifier keys of a chord. Caps Lock and Fn are not part of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
}

impl Mods {
    /// From an NSEvent modifier mask as stored in com.apple.symbolichotkeys.
    fn from_mask(mask: i64) -> Mods {
        Mods {
            shift: mask & (1 << 17) != 0,
            ctrl: mask & (1 << 18) != 0,
            alt: mask & (1 << 19) != 0,
            command: mask & (1 << 20) != 0,
        }
    }
}

/// A macOS key plus the exact modifiers that must be down with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub keycode: u32,
    pub mods: Mods,
}

/// LANG1 (the 한/영 key of Korean and Japanese keyboards) always counts.
const LANG1: Chord = Chord {
    keycode: MAC_KVK_JIS_KANA,
    mods: Mods {
        ctrl: false,
        alt: false,
        shift: false,
        command: false,
    },
};

impl Chord {
    fn name(&self) -> String {
        let mut name = String::new();
        for (on, sign) in [
            (self.mods.ctrl, "⌃"),
            (self.mods.alt, "⌥"),
            (self.mods.shift, "⇧"),
            (self.mods.command, "⌘"),
        ] {
            if on {
                name.push_str(sign);
            }
        }
        let key = match self.keycode {
            MAC_KVK_SPACE => "Space".to_owned(),
            MAC_KVK_JIS_KANA => "LANG1".to_owned(),
            36 => "Return".to_owned(),
            48 => "Tab".to_owned(),
            64 => "F17".to_owned(),
            MAC_KVK_F18 => "F18".to_owned(),
            80 => "F19".to_owned(),
            90 => "F20".to_owned(),
            code => format!("key {}", code),
        };
        name + &key
    }
}

/// macOS defaults when the user never changed the shortcut: 60 "Select the
/// previous input source" = ⌃Space, 61 "Select next source in Input menu" = ⌃⌥Space.
fn default_hotkey(id: i32) -> Option<Chord> {
    let ctrl = Mods {
        ctrl: true,
        ..Default::default()
    };
    match id {
        60 => Some(Chord {
            keycode: MAC_KVK_SPACE,
            mods: ctrl,
        }),
        61 => Some(Chord {
            keycode: MAC_KVK_SPACE,
            mods: Mods { alt: true, ..ctrl },
        }),
        _ => None,
    }
}

/// One AppleSymbolicHotKeys entry: `None` when absent (macOS uses its default),
/// otherwise its `enabled` flag and `value.parameters` (character, virtual
/// keycode, modifier mask), each `None` when missing or not a number.
pub type SymbolicHotkeyEntry = Option<(Option<bool>, Option<Vec<i64>>)>;

/// The chord an enabled input-source hotkey stands for; `None` when it is off,
/// unset (keycode 65535) or malformed.
pub fn chord_from_symbolic_hotkey(id: i32, entry: SymbolicHotkeyEntry) -> Option<Chord> {
    let Some((enabled, params)) = entry else {
        return default_hotkey(id);
    };
    if enabled != Some(true) {
        return None;
    }
    let params = params?;
    let [_, keycode, mask] = params.as_slice() else {
        return None;
    };
    if !(0..=0x7f).contains(keycode) || *mask < 0 {
        return None;
    }
    Some(Chord {
        keycode: *keycode as u32,
        mods: Mods::from_mask(*mask),
    })
}

/// The local 한/영 chords: the enabled hotkeys, then LANG1, without duplicates.
pub fn local_chords(hotkeys: impl IntoIterator<Item = Option<Chord>>) -> Vec<Chord> {
    let mut chords = Vec::new();
    for chord in hotkeys.into_iter().flatten().chain([LANG1]) {
        if !chords.contains(&chord) {
            chords.push(chord);
        }
    }
    chords
}

static LOCAL_CHORDS: Mutex<Option<Vec<Chord>>> = Mutex::new(None);
static HELD_CHORD_KEYS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Replaces the local 한/영 chords (macOS controller); logs when they change.
pub fn set_local_chords(chords: Vec<Chord>) {
    let mut current = LOCAL_CHORDS.lock().unwrap();
    if current.as_ref() != Some(&chords) {
        let names: Vec<String> = chords.iter().map(Chord::name).collect();
        hbb_common::log::info!("Local 한/영 keys: {}", names.join(", "));
    }
    *current = Some(chords);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChordAction {
    /// Not a chord: send the key as usual.
    Pass,
    /// A repeat of a chord key that is already down.
    Swallow,
    /// Send the peer's 한/영 key down or up.
    Toggle(bool),
}

/// `held` lists the chord keys whose key-down was turned into a toggle, so their
/// repeats are swallowed and their key-up becomes the toggle's key-up.
fn chord_action(
    chords: &[Chord],
    held: &mut Vec<u32>,
    keycode: u32,
    down: bool,
    mods: Mods,
) -> ChordAction {
    if !down {
        return match held.iter().position(|k| *k == keycode) {
            Some(i) => {
                held.remove(i);
                ChordAction::Toggle(false)
            }
            None => ChordAction::Pass,
        };
    }
    if held.contains(&keycode) {
        return ChordAction::Swallow;
    }
    if chords
        .iter()
        .any(|c| c.keycode == keycode && c.mods == mods)
    {
        held.push(keycode);
        return ChordAction::Toggle(true);
    }
    ChordAction::Pass
}

/// The key events to send instead of `event` when it is the local 한/영 key;
/// `None` leaves the event to the normal keyboard modes.
///
/// `modifier_events(down)` gives the held modifier keys as key events for the
/// peer. A Mac chord's modifiers (the Control of ⌃Space) already went to the
/// peer, so they are released around the toggle and pressed again after it: a
/// Linux input method ignores Control+Hangul, and the peer's modifier state then
/// matches the keys still held here.
pub fn to_peer_hangul_key(
    peer: &str,
    event: &Event,
    key_event: &KeyEvent,
    modifier_events: &dyn Fn(bool) -> Vec<KeyEvent>,
) -> Option<Vec<KeyEvent>> {
    let down = match event.event_type {
        EventType::KeyPress(..) => true,
        EventType::KeyRelease(..) => false,
        _ => return None,
    };
    #[cfg(target_os = "macos")]
    return mac_chord_events(peer, event, key_event, down, modifier_events);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = modifier_events;
        single_key_events(peer, event, key_event, down)
    }
}

fn option_enabled() -> bool {
    use base::config::keys::OPTION_ENABLE_PEER_HANGUL_KEY;
    use hbb_common::config::LocalConfig;
    LocalConfig::get_bool_option(OPTION_ENABLE_PEER_HANGUL_KEY)
}

fn with_legacy_modifiers(mut evt: KeyEvent) -> KeyEvent {
    if evt.mode.enum_value_or(KeyboardMode::Legacy) == KeyboardMode::Legacy {
        let (alt, ctrl, shift, command) =
            crate::keyboard::client::get_modifiers_state(false, false, false, false);
        crate::keyboard::client::legacy_modifiers(&mut evt, alt, ctrl, shift, command);
    }
    evt
}

#[cfg(not(target_os = "macos"))]
fn single_key_events(
    peer: &str,
    event: &Event,
    key_event: &KeyEvent,
    down: bool,
) -> Option<Vec<KeyEvent>> {
    if !is_local_hangul_key(LOCAL_OS, event.position_code, event.platform_code) || !option_enabled()
    {
        return None;
    }
    let converted = convert(
        LOCAL_OS,
        peer,
        event.position_code,
        event.platform_code,
        down,
        key_event,
    );
    if down {
        match &converted {
            Some(evt) => hbb_common::log::info!(
                "Local 한/영 key sent to {} peer as {:?} ({:?} mode)",
                peer,
                evt.union,
                evt.mode
            ),
            None => hbb_common::log::info!("Local 한/영 key sent to {} peer unchanged", peer),
        }
    }
    Some(vec![with_legacy_modifiers(converted?)])
}

#[cfg(target_os = "macos")]
fn mac_chord_events(
    peer: &str,
    event: &Event,
    key_event: &KeyEvent,
    down: bool,
    modifier_events: &dyn Fn(bool) -> Vec<KeyEvent>,
) -> Option<Vec<KeyEvent>> {
    let peer_os = peer_os(peer)?;
    if !option_enabled() {
        return None;
    }
    let loaded = LOCAL_CHORDS.lock().unwrap().clone();
    let chords = match loaded {
        Some(chords) => chords,
        None => {
            crate::platform::macos_input_source::load_local_hangul_chords();
            LOCAL_CHORDS.lock().unwrap().clone().unwrap_or_default()
        }
    };
    let (alt, ctrl, shift, command) =
        crate::keyboard::client::get_modifiers_state(false, false, false, false);
    let mods = Mods {
        ctrl,
        alt,
        shift,
        command,
    };
    let keycode = event.platform_code;
    let action = chord_action(
        &chords,
        &mut HELD_CHORD_KEYS.lock().unwrap(),
        keycode,
        down,
        mods,
    );
    let toggle_down = match action {
        ChordAction::Pass => return None,
        ChordAction::Swallow => return Some(Vec::new()),
        ChordAction::Toggle(toggle_down) => toggle_down,
    };
    let evt = peer_toggle_event(Os::MacOS, peer_os, toggle_down, key_event)?;
    let around = toggle_down && mods != Mods::default();
    if toggle_down {
        let name = Chord { keycode, mods }.name();
        hbb_common::log::info!(
            "Local 한/영 key {} sent to {} peer as {:?} ({:?} mode)",
            name,
            peer,
            evt.union,
            evt.mode
        );
    }
    let mut events = Vec::new();
    if around {
        events.extend(modifier_events(false));
        events.push(evt);
        events.extend(modifier_events(true));
    } else {
        events.push(with_legacy_modifiers(evt));
    }
    Some(events)
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
                assert_eq!(
                    keycode(send(key, "macos", mode)),
                    Some((mode, 79)),
                    "{key:?}"
                );
            }
            for key in [MAC_F18, WIN_HANGUL, WIN_RALT_HANGUL] {
                assert_eq!(
                    keycode(send(key, "linux", mode)),
                    Some((mode, 130)),
                    "{key:?}"
                );
            }
            for key in [MAC_F18, LINUX_HANGUL, LINUX_RALT_HANGUL] {
                assert!(is_legacy_hangul(send(key, "windows", mode)), "{key:?}");
            }
        }
        for (key, peer) in [
            (MAC_F18, "windows"),
            (WIN_HANGUL, "macos"),
            (LINUX_HANGUL, "macos"),
        ] {
            assert!(
                is_legacy_hangul(send(key, peer, KeyboardMode::Legacy)),
                "{key:?}"
            );
        }
    }

    #[test]
    fn same_os_and_other_keys_are_unchanged() {
        for mode in [
            KeyboardMode::Map,
            KeyboardMode::Translate,
            KeyboardMode::Legacy,
        ] {
            assert!(send(WIN_HANGUL, "windows", mode).is_none());
            assert!(send(LINUX_HANGUL, "linux", mode).is_none());
            assert!(send(MAC_F18, "android", mode).is_none());
            // Japanese Kana (VK_KANA = 0x15), macOS F19, Linux Hanja.
            assert!(send((Os::Windows, 0x70, 0x15), "macos", mode).is_none());
            assert!(send((Os::MacOS, 80, 80), "windows", mode).is_none());
            assert!(send((Os::Linux, 131, 0xff34), "macos", mode).is_none());
        }
    }

    // Without Input Monitoring the macOS client gets keys from Flutter: USB HID
    // usage -> rdev key -> macOS keycode, as in
    // `Session::_handle_key_non_flutter_simulation`. F18 (Caps Lock) must come
    // out as keycode 79, the code a hotkey chord stores.
    #[cfg(target_os = "macos")]
    #[test]
    fn flutter_f18_on_macos_is_the_local_hangul_key() {
        let key = rdev::usb_hid_key_from_code(0x6D);
        let code = rdev::code_from_key(key).map(|c| c as u32);
        assert_eq!(code, Some(MAC_KVK_F18));
        let code = MAC_KVK_F18;
        assert!(is_local_hangul_key(Os::MacOS, code, code));
        let mut key_event = KeyEvent::new();
        key_event.mode = KeyboardMode::Map.into();
        assert_eq!(
            keycode(convert(Os::MacOS, "macos", code, code, true, &key_event)),
            Some((KeyboardMode::Map, 79))
        );
        assert_eq!(
            keycode(convert(Os::MacOS, "linux", code, code, true, &key_event)),
            Some((KeyboardMode::Map, 130))
        );
        assert!(is_legacy_hangul(convert(
            Os::MacOS,
            "windows",
            code,
            code,
            true,
            &key_event
        )));
    }

    #[test]
    fn mac_to_mac_sends_f18() {
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            assert_eq!(keycode(send(MAC_F18, "macos", mode)), Some((mode, 79)));
        }
        assert!(is_legacy_hangul(send(
            MAC_F18,
            "macos",
            KeyboardMode::Legacy
        )));
    }

    const CTRL: Mods = Mods {
        ctrl: true,
        alt: false,
        shift: false,
        command: false,
    };
    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
        command: false,
    };

    #[test]
    fn symbolic_hotkeys_become_chords() {
        let entry = |enabled, params: &[i64]| Some((enabled, Some(params.to_vec())));
        // F18 (Fn flag set by macOS for function keys) and ⌃Space.
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(true), &[65535, 79, 8388608])),
            Some(Chord {
                keycode: 79,
                mods: NONE
            })
        );
        assert_eq!(
            chord_from_symbolic_hotkey(61, entry(Some(true), &[32, 49, 262144])),
            Some(Chord {
                keycode: 49,
                mods: CTRL
            })
        );
        // ⌥⌘Space, Caps Lock bit ignored.
        assert_eq!(
            chord_from_symbolic_hotkey(61, entry(Some(true), &[32, 49, 1572864 | 65536])),
            Some(Chord {
                keycode: 49,
                mods: Mods {
                    alt: true,
                    command: true,
                    ..NONE
                }
            })
        );
        // Off, unset, malformed.
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(false), &[32, 49, 262144])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(None, &[32, 49, 262144])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(true), &[65535, 65535, 0])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(true), &[32, 49])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(true), &[32, 49, 0, 0])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, entry(Some(true), &[32, 49, -1])),
            None
        );
        assert_eq!(
            chord_from_symbolic_hotkey(60, Some((Some(true), None))),
            None
        );
        // Absent: the macOS default.
        assert_eq!(
            chord_from_symbolic_hotkey(60, None),
            Some(Chord {
                keycode: 49,
                mods: CTRL
            })
        );
    }

    #[test]
    fn lang1_is_always_a_chord_and_names_are_readable() {
        let f18 = Chord {
            keycode: 79,
            mods: NONE,
        };
        let ctrl_space = Chord {
            keycode: 49,
            mods: CTRL,
        };
        let chords = local_chords([Some(f18), Some(ctrl_space), None, Some(f18)]);
        assert_eq!(chords, vec![f18, ctrl_space, LANG1]);
        assert_eq!(local_chords([None, None]), vec![LANG1]);
        let names: Vec<String> = chords.iter().map(Chord::name).collect();
        assert_eq!(names, ["F18", "⌃Space", "LANG1"]);
    }

    #[test]
    fn chord_needs_its_exact_modifiers_and_toggles_once_per_press() {
        let chords = local_chords([Some(Chord {
            keycode: 49,
            mods: CTRL,
        })]);
        let mut held = Vec::new();
        let shift_ctrl = Mods {
            shift: true,
            ..CTRL
        };
        // Typing Space, Shift+Space and Ctrl+Shift+Space is not 한/영.
        for mods in [
            NONE,
            Mods {
                shift: true,
                ..NONE
            },
            shift_ctrl,
        ] {
            assert_eq!(
                chord_action(&chords, &mut held, 49, true, mods),
                ChordAction::Pass
            );
            assert_eq!(
                chord_action(&chords, &mut held, 49, false, mods),
                ChordAction::Pass
            );
        }
        // Ctrl with another key.
        assert_eq!(
            chord_action(&chords, &mut held, 8, true, CTRL),
            ChordAction::Pass
        );
        // Ctrl+Space, held (autorepeat), released.
        assert_eq!(
            chord_action(&chords, &mut held, 49, true, CTRL),
            ChordAction::Toggle(true)
        );
        assert_eq!(
            chord_action(&chords, &mut held, 49, true, CTRL),
            ChordAction::Swallow
        );
        assert_eq!(
            chord_action(&chords, &mut held, 49, false, CTRL),
            ChordAction::Toggle(false)
        );
        // Control let go before Space: the key-up still ends the toggle.
        assert_eq!(
            chord_action(&chords, &mut held, 49, true, CTRL),
            ChordAction::Toggle(true)
        );
        assert_eq!(
            chord_action(&chords, &mut held, 49, false, NONE),
            ChordAction::Toggle(false)
        );
        // LANG1 alone.
        assert_eq!(
            chord_action(&chords, &mut held, 104, true, NONE),
            ChordAction::Toggle(true)
        );
        assert_eq!(
            chord_action(&chords, &mut held, 104, false, NONE),
            ChordAction::Toggle(false)
        );
        assert!(held.is_empty());
    }

    #[test]
    fn chord_toggle_is_the_peer_hangul_key() {
        let mut key_event = KeyEvent::new();
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            key_event.mode = mode.into();
            for (peer, code) in [(Os::MacOS, 79), (Os::Linux, 130)] {
                assert_eq!(
                    keycode(peer_toggle_event(Os::MacOS, peer, true, &key_event)),
                    Some((mode, code))
                );
            }
            assert!(is_legacy_hangul(peer_toggle_event(
                Os::MacOS,
                Os::Windows,
                true,
                &key_event
            )));
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
