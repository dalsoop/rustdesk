// Hangul -> Dubeolsik (2-set Korean) key sequence, for remote hosts that cannot
// type Unicode text.
//
// A Linux host on Wayland injects keys through uinput, which only knows key
// codes, so a Hangul syllable sent as text never arrives. Sending the
// Dubeolsik keys instead (안 -> d k s) lets the host's own input method
// (fcitx5, ibus) compose the syllables, the same way a physical keyboard does.
// The host's input method must be in Korean mode for the keys to compose, and
// in English mode for Latin letters. A toggle key cannot set a known state, so
// the client sends F19 when a run of Korean starts and F24 before every edit
// with Latin letters (F19 in the middle of a syllable would commit it); a host
// maps
// them to Hangul_Start and Hangul_End and binds those to its input method's
// activate and deactivate keys (fcitx5 ActivateKeys / DeactivateKeys). On a
// host without that mapping both keys do nothing and the "한/영" key bar
// button switches the mode by hand.

/// How the mobile soft keyboard sends Korean text.
enum KoreanInputMode { auto, unicode, dubeolsik }

/// Local option that stores the choice (value: [KoreanInputMode.name]).
const String kOptionKoreanInputMode = 'korean-input-mode';

KoreanInputMode koreanInputModeFromOption(String value) {
  for (final mode in KoreanInputMode.values) {
    if (mode.name == value) return mode;
  }
  return KoreanInputMode.auto;
}

String koreanInputModeLabel(KoreanInputMode mode) {
  switch (mode) {
    case KoreanInputMode.auto:
      return '자동 (리눅스는 두벌식 키)';
    case KoreanInputMode.unicode:
      return '글자 그대로';
    case KoreanInputMode.dubeolsik:
      return '두벌식 키 (원격 입력기가 조합)';
  }
}

/// USB HID usage (keyboard page 0x07) of F19, sent before Korean keys.
const int kUsbHidHangulStart = 0x6E;

/// USB HID usage (keyboard page 0x07) of F24, sent before Latin letters.
const int kUsbHidHangulEnd = 0x73;

/// How long the client waits after F19/F24 before the next keys, so the host's
/// input method has switched when they arrive.
const Duration kRemoteImeSwitchDelay = Duration(milliseconds: 150);

/// Input method state the host needs for a run of Dubeolsik keys.
enum RemoteImeMode { korean, latin }

/// The mode the host must be in to type [insert], or null when either mode
/// types it the same (digits, punctuation, spaces, nothing at all).
RemoteImeMode? remoteImeModeFor(String insert) {
  if (containsHangul(insert)) return RemoteImeMode.korean;
  if (insert.contains(RegExp('[A-Za-z]'))) return RemoteImeMode.latin;
  return null;
}

/// Whether to send Dubeolsik keys to a peer of [peerPlatform] ('Linux', ...).
bool useDubeolsikKeys(KoreanInputMode mode, String peerPlatform) {
  switch (mode) {
    case KoreanInputMode.unicode:
      return false;
    case KoreanInputMode.dubeolsik:
      return true;
    case KoreanInputMode.auto:
      return peerPlatform == 'Linux';
  }
}

// Initial, medial and final jamo of a precomposed syllable, in Unicode order.
const _initials = [
  'r', 'R', 's', 'e', 'E', 'f', 'a', 'q', 'Q', 't', //
  'T', 'd', 'w', 'W', 'c', 'z', 'x', 'v', 'g',
];
const _medials = [
  'k', 'o', 'i', 'O', 'j', 'p', 'u', 'P', 'h', 'hk', //
  'ho', 'hl', 'y', 'n', 'nj', 'np', 'nl', 'b', 'm', 'ml', 'l',
];
const _finals = [
  '', 'r', 'R', 'rt', 's', 'sw', 'sg', 'e', 'f', 'fr', //
  'fa', 'fq', 'ft', 'fx', 'fv', 'fg', 'a', 'q', 'qt', 't',
  'T', 'd', 'w', 'c', 'z', 'x', 'v', 'g',
];

// Compatibility jamo U+3131 (ㄱ) .. U+3163 (ㅣ), as typed alone.
const _compatJamo = [
  'r', 'R', 'rt', 's', 'sw', 'sg', 'e', 'E', 'f', 'fr', // ㄱ..ㄺ
  'fa', 'fq', 'ft', 'fx', 'fv', 'fg', 'a', 'q', 'Q', 'qt', // ㄻ..ㅄ
  't', 'T', 'd', 'w', 'W', 'c', 'z', 'x', 'v', 'g', // ㅅ..ㅎ
  'k', 'o', 'i', 'O', 'j', 'p', 'u', 'P', 'h', 'hk', // ㅏ..ㅘ
  'ho', 'hl', 'y', 'n', 'nj', 'np', 'nl', 'b', 'm', 'ml', // ㅙ..ㅢ
  'l', // ㅣ
];

const _syllableFirst = 0xAC00;
const _syllableLast = 0xD7A3;
const _compatFirst = 0x3131;
const _compatLast = 0x3163;

bool _isHangulCode(int code) =>
    (code >= _syllableFirst && code <= _syllableLast) ||
    (code >= _compatFirst && code <= _compatLast);

bool containsHangul(String text) => text.runes.any(_isHangulCode);

/// Replaces every Hangul syllable and jamo with its Dubeolsik keys; other
/// characters are kept as they are. Shifted keys are upper case (ㅃ -> Q).
String hangulToDubeolsikKeys(String text) {
  final out = StringBuffer();
  for (final code in text.runes) {
    if (code >= _syllableFirst && code <= _syllableLast) {
      final index = code - _syllableFirst;
      out
        ..write(_initials[index ~/ (21 * 28)])
        ..write(_medials[(index % (21 * 28)) ~/ 28])
        ..write(_finals[index % 28]);
    } else if (code >= _compatFirst && code <= _compatLast) {
      out.write(_compatJamo[code - _compatFirst]);
    } else {
      out.writeCharCode(code);
    }
  }
  return out.toString();
}

/// Turns a text edit (delete [deleted] from the end, then type [insert]) into
/// the smallest key edit: both sides become Dubeolsik keys and their common
/// key prefix is skipped. ㅇ -> 아 -> 안 therefore sends d, k, s one by one
/// instead of erasing and retyping whole syllables, which matches how the
/// host's input method composes and deletes jamo.
({int backspaces, String keys}) dubeolsikKeyEdit(
    String deleted, String insert) {
  final oldKeys = hangulToDubeolsikKeys(deleted);
  final newKeys = hangulToDubeolsikKeys(insert);
  var common = 0;
  while (common < oldKeys.length &&
      common < newKeys.length &&
      oldKeys.codeUnitAt(common) == newKeys.codeUnitAt(common)) {
    common++;
  }
  return (
    backspaces: oldKeys.length - common,
    keys: newKeys.substring(common),
  );
}
