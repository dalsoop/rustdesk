import 'package:flutter_hbb/models/korean_toggle_key.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('every choice round-trips through its option value', () {
    for (final key in KoreanToggleKey.values) {
      expect(koreanToggleKeyFromOption(key.name), key);
    }
  });

  test('empty or unknown option falls back to automatic', () {
    expect(koreanToggleKeyFromOption(''), KoreanToggleKey.auto);
    expect(koreanToggleKeyFromOption('f99'), KoreanToggleKey.auto);
  });

  test('automatic sends F18 to a Mac and Hangul to Windows and Linux', () {
    expect(resolveKoreanToggleKey(KoreanToggleKey.auto, 'Mac OS'),
        KoreanToggleKey.f18);
    expect(resolveKoreanToggleKey(KoreanToggleKey.auto, 'Windows'),
        KoreanToggleKey.hangul);
    expect(resolveKoreanToggleKey(KoreanToggleKey.auto, 'Linux'),
        KoreanToggleKey.hangul);
  });

  test('a manual choice is kept for every peer', () {
    for (final peer in ['Mac OS', 'Windows', 'Linux']) {
      expect(resolveKoreanToggleKey(KoreanToggleKey.ctrlSpace, peer),
          KoreanToggleKey.ctrlSpace);
      expect(resolveKoreanToggleKey(KoreanToggleKey.hangul, peer),
          KoreanToggleKey.hangul);
    }
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
