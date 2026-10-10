import 'package:flutter_hbb/models/korean_toggle_key.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('every choice round-trips through its option value', () {
    for (final key in KoreanToggleKey.values) {
      expect(koreanToggleKeyFromOption(key.name), key);
    }
  });

  test('empty or unknown option falls back to the Hangul key', () {
    expect(koreanToggleKeyFromOption(''), KoreanToggleKey.hangul);
    expect(koreanToggleKeyFromOption('f99'), KoreanToggleKey.hangul);
  });

  test('every choice has a label', () {
    for (final key in KoreanToggleKey.values) {
      expect(koreanToggleKeyLabel(key), isNotEmpty);
    }
  });

  test('F18 is usage 0x6D on the keyboard page', () {
    expect(kUsbHidF18, 0x6D);
  });
}
