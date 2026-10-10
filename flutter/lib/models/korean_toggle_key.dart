/// Which key the mobile "한/영" button sends to the remote side.
///
/// Remote input methods switch Korean/English on different keys: Windows and
/// most Linux setups (fcitx5/ibus) use Hangul, some Linux setups map F18 or
/// Right Alt, macOS often uses Ctrl+Space or a key remapped to F18. The user
/// picks the one their remote machine listens to.
enum KoreanToggleKey { hangul, f18, rightAlt, shiftSpace, ctrlSpace }

/// Local option that stores the choice (value: [KoreanToggleKey.name]).
const String kOptionKoreanToggleKey = 'korean-toggle-key';

/// USB HID usage of F18 on the keyboard page (0x07). Sent through the map
/// keyboard path because the legacy key names stop at F12.
const int kUsbHidF18 = 0x6D;

KoreanToggleKey koreanToggleKeyFromOption(String value) {
  for (final key in KoreanToggleKey.values) {
    if (key.name == value) return key;
  }
  return KoreanToggleKey.hangul;
}

String koreanToggleKeyLabel(KoreanToggleKey key) {
  switch (key) {
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
