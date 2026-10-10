import 'package:flutter_hbb/models/hangul_dubeolsik.dart';
import 'package:flutter_hbb/models/soft_keyboard_input_utils.dart';
import 'package:flutter_test/flutter_test.dart';

/// Replays a phone IME's text-field states through the soft-keyboard diff and
/// the Dubeolsik key edit, and returns the keys the host would receive
/// ('<' stands for one backspace).
String replay(List<String> states) {
  final sent = StringBuffer();
  var old = '';
  for (final state in states) {
    final edit = computeSoftKeyboardEditDetailed(old, state);
    final keyEdit = dubeolsikKeyEdit(edit.deleted, edit.insert);
    sent
      ..write('<' * keyEdit.backspaces)
      ..write(keyEdit.keys);
    old = state;
  }
  return sent.toString();
}

void main() {
  group('hangulToDubeolsikKeys', () {
    test('syllables become initial, medial and final keys', () {
      expect(hangulToDubeolsikKeys('안녕하세요'), 'dkssudgktpdy');
      expect(hangulToDubeolsikKeys('한글'), 'gksrmf');
    });

    test('compound vowels and finals split into their keys', () {
      expect(hangulToDubeolsikKeys('과'), 'rhk');
      expect(hangulToDubeolsikKeys('왜'), 'dho');
      expect(hangulToDubeolsikKeys('의'), 'dml');
      expect(hangulToDubeolsikKeys('닭'), 'ekfr');
      expect(hangulToDubeolsikKeys('값'), 'rkqt');
    });

    test('shifted consonants and vowels are upper case', () {
      expect(hangulToDubeolsikKeys('빠'), 'Qk');
      expect(hangulToDubeolsikKeys('쌍'), 'Tkd');
      expect(hangulToDubeolsikKeys('예'), 'dP');
    });

    test('lone jamo and non-Hangul text', () {
      expect(hangulToDubeolsikKeys('ㅇ'), 'd');
      expect(hangulToDubeolsikKeys('ㅘ'), 'hk');
      expect(hangulToDubeolsikKeys('a1 안!'), 'a1 dks!');
    });

    test('first and last syllables of the block', () {
      expect(hangulToDubeolsikKeys('가'), 'rk');
      expect(hangulToDubeolsikKeys('힣'), 'glg');
    });
  });

  group('typing through a phone IME sends each jamo key once', () {
    test('안녕하세요', () {
      expect(
          replay([
            'ㅇ', '아', '안', '안ㄴ', '안녀', '안녕', //
            '안녕ㅎ', '안녕하', '안녕핫', '안녕하세', '안녕하셍', '안녕하세요',
          ]),
          'dkssudgktpdy');
    });

    test('a final consonant that moves to the next syllable', () {
      // 알 -> 아라: the IME pulls ㄹ into the next syllable; the host's IME
      // does the same when it sees the vowel key.
      expect(replay(['ㅇ', '아', '알', '아라']), 'dkfk');
    });

    test('backspace removes one jamo key at a time', () {
      expect(replay(['ㅇ', '아', '안', '아']), 'dks<');
      expect(replay(['ㅇ', '아', '안', '아', 'ㅇ', '']), 'dks<<<');
    });

    test('compound vowel built in two steps', () {
      expect(replay(['ㄱ', '고', '과']), 'rhk');
    });
  });

  group('mode', () {
    test('auto uses keys for Linux only', () {
      expect(useDubeolsikKeys(KoreanInputMode.auto, 'Linux'), isTrue);
      expect(useDubeolsikKeys(KoreanInputMode.auto, 'Windows'), isFalse);
      expect(useDubeolsikKeys(KoreanInputMode.unicode, 'Linux'), isFalse);
      expect(useDubeolsikKeys(KoreanInputMode.dubeolsik, 'Mac OS'), isTrue);
    });

    test('options round-trip and default to auto', () {
      for (final mode in KoreanInputMode.values) {
        expect(koreanInputModeFromOption(mode.name), mode);
      }
      expect(koreanInputModeFromOption(''), KoreanInputMode.auto);
    });
  });
}
