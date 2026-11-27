//! The guess for text in each family VS Code guesses, and for content that suggests nothing.

use super::*;

const JAPANESE: &str =
    "日本語の文章です。今日は天気が良いので、公園へ散歩に行きました。桜の花がとても綺麗でした。";
const KOREAN: &str =
    "한국어 문장입니다. 오늘은 날씨가 좋아서 공원에 산책을 갔습니다. 꽃이 아주 예뻤습니다.";
const SIMPLIFIED: &str =
    "这是一段中文文本。今天天气很好，我们去公园散步，看到了很多美丽的花朵和高大的树木。";
const TRADITIONAL: &str =
    "這是一段繁體中文文本。今天天氣很好，我們去公園散步，看到了很多美麗的花朵和高大的樹木。";
const RUSSIAN: &str =
    "Это русский текст. Сегодня хорошая погода, и мы пошли гулять в парк, где цвели деревья.";

fn encoded(encoding: &'static encoding_rs::Encoding, text: &str) -> Vec<u8> {
    encoding.encode(&text.repeat(20)).0.into_owned()
}

#[test]
fn text_is_guessed_within_vs_code_s_set() {
    let cases = [
        (encoded(encoding_rs::UTF_8, RUSSIAN), Some("utf8")),
        (encoded(encoding_rs::SHIFT_JIS, JAPANESE), Some("shiftjis")),
        (encoded(encoding_rs::EUC_JP, JAPANESE), Some("eucjp")),
        (encoded(encoding_rs::EUC_KR, KOREAN), Some("euckr")),
        (encoded(encoding_rs::GBK, SIMPLIFIED), Some("gbk")),
        (encoded(encoding_rs::BIG5, TRADITIONAL), Some("cp950")),
        (
            encoded(encoding_rs::WINDOWS_1251, RUSSIAN),
            Some("windows1251"),
        ),
        (encoded(encoding_rs::KOI8_R, RUSSIAN), Some("koi8r")),
    ];
    for (index, (content, expected)) in cases.into_iter().enumerate() {
        assert_eq!(guess(&content, true), expected, "case {index}");
    }
}

#[test]
fn marks_decide_and_ascii_or_binary_content_suggests_nothing() {
    assert_eq!(guess(b"\xFF\xFEa\0", true), Some("utf16le"));
    assert_eq!(guess(b"a\0b\0c\0", true), Some("utf16le"));
    assert_eq!(guess(b"\xEF\xBB\xBFplain", true), Some("utf8"));
    assert_eq!(guess(b"plain ASCII text", true), None);
    assert_eq!(guess(b"ab\0\0\xE9", true), None);
    // chardetng answers VS Code does not guess map to none.
    assert_eq!(guessable(encoding_rs::WINDOWS_1256), None);
    assert_eq!(guessable(encoding_rs::ISO_2022_JP), None);
}
