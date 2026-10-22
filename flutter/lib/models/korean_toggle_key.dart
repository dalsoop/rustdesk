/// Which key the mobile "한/영" button sends to the remote side.
///
/// Remote input methods switch Korean/English on different keys: Windows and
/// most Linux setups (fcitx5/ibus) use Hangul, some Linux setups map F18 or
/// Right Alt, macOS often uses Ctrl+Space or a key remapped to F18. [auto]
/// picks the key by the remote OS, the same one the desktop client's
/// "send my Korean/English key as the remote computer's" setting sends; the
/// others are a manual override.
enum KoreanToggleKey { auto, hangul, f18, rightAlt, shiftSpace, ctrlSpace }

/// Local option that stores the choice (value: [KoreanToggleKey.name]).
const String kOptionKoreanToggleKey = 'korean-toggle-key';

/// USB HID usage of F18 on the keyboard page (0x07). Sent through the map
/// keyboard path because the legacy key names stop at F12.
const int kUsbHidF18 = 0x6D;

KoreanToggleKey koreanToggleKeyFromOption(String value) {
  for (final key in KoreanToggleKey.values) {
    if (key.name == value) return key;
  }
  return KoreanToggleKey.auto;
}

/// The key to send for [choice] to a peer reporting [peerPlatform]
/// ("Mac OS", "Windows", "Linux", ...). Never returns [KoreanToggleKey.auto].
///
/// A Mac switches on F18 (Caps Lock remapped; the host switches the input
/// source itself), Windows and Linux on the Hangul key (VK_HANGUL,
/// KEY_HANGEUL). On Linux the soft keyboard's F19/F24 mode keys
/// (hangul_dubeolsik.dart) stay as they are; this is only the button.
KoreanToggleKey resolveKoreanToggleKey(
    KoreanToggleKey choice, String peerPlatform) {
  if (choice != KoreanToggleKey.auto) return choice;
  return peerPlatform == 'Mac OS' ? KoreanToggleKey.f18 : KoreanToggleKey.hangul;
}

String koreanToggleKeyLabel(KoreanToggleKey key) {
  switch (key) {
    case KoreanToggleKey.auto:
      return '자동 (맥은 F18, 윈도우·리눅스는 Hangul)';
    case KoreanToggleKey.hangul:
      return '한/영 키 (Hangul)';
    case KoreanToggleKey.f18:
      return 'F18';
    case KoreanToggleKey.rightAlt:
      return '오른쪽 Alt';
    case KoreanToggleKey.shiftSpace:
      return 'Shift + Space';
    case KoreanToggleKey.ctrlSpace:
      return 'Ctrl + Space';
  }
}
