#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyInput {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    CtrlA,
    CtrlB,
    CtrlC,
    CtrlD,
    CtrlE,
    CtrlF,
    CtrlG,
    CtrlJ,
    CtrlK,
    CtrlN,
    CtrlP,
    CtrlR,
    CtrlT,
    CtrlW,
    Char(char),
}

pub fn parse_test_keys(input: &str) -> Vec<KeyInput> {
    if input.is_empty() {
        return Vec::new();
    }

    let token_mode =
        input.contains(',') || input.chars().all(|c| c.is_ascii_uppercase() || c == '-');
    if token_mode {
        return parse_token_mode(input);
    }

    parse_raw_mode(input)
}

fn parse_token_mode(input: &str) -> Vec<KeyInput> {
    let mut keys = Vec::new();

    for token in input.split(',').map(|part| part.trim_start()) {
        if token.is_empty() {
            continue;
        }

        let up = token.to_ascii_uppercase();
        match up.as_str() {
            "UP" => keys.push(KeyInput::Up),
            "DOWN" => keys.push(KeyInput::Down),
            "LEFT" => keys.push(KeyInput::Left),
            "RIGHT" => keys.push(KeyInput::Right),
            "ENTER" => keys.push(KeyInput::Enter),
            "ESC" => keys.push(KeyInput::Escape),
            "BACKSPACE" => keys.push(KeyInput::Backspace),
            "CTRL-A" | "CTRLA" => keys.push(KeyInput::CtrlA),
            "CTRL-B" | "CTRLB" => keys.push(KeyInput::CtrlB),
            "CTRL-C" | "CTRLC" => keys.push(KeyInput::CtrlC),
            "CTRL-D" | "CTRLD" => keys.push(KeyInput::CtrlD),
            "CTRL-E" | "CTRLE" => keys.push(KeyInput::CtrlE),
            "CTRL-F" | "CTRLF" => keys.push(KeyInput::CtrlF),
            "CTRL-G" | "CTRLG" => keys.push(KeyInput::CtrlG),
            "CTRL-H" | "CTRLH" => keys.push(KeyInput::Backspace),
            // Extension for try/spec/tests/test_13_vim_nav.sh:
            // Ruby token map omits CTRL-J, but vim-style down navigation needs it.
            "CTRL-J" | "CTRLJ" => keys.push(KeyInput::CtrlJ),
            "CTRL-K" | "CTRLK" => keys.push(KeyInput::CtrlK),
            "CTRL-N" | "CTRLN" => keys.push(KeyInput::CtrlN),
            "CTRL-P" | "CTRLP" => keys.push(KeyInput::CtrlP),
            "CTRL-R" | "CTRLR" => keys.push(KeyInput::CtrlR),
            "CTRL-T" | "CTRLT" => keys.push(KeyInput::CtrlT),
            "CTRL-W" | "CTRLW" => keys.push(KeyInput::CtrlW),
            _ if starts_with_type_prefix(token) => {
                keys.extend(token[5..].chars().map(KeyInput::Char));
            }
            _ if token.chars().count() == 1 => {
                if let Some(ch) = token.chars().next() {
                    keys.push(KeyInput::Char(ch));
                }
            }
            _ => {}
        }
    }

    keys
}

fn starts_with_type_prefix(token: &str) -> bool {
    token.len() >= 5 && token[..5].eq_ignore_ascii_case("TYPE=")
}

fn parse_raw_mode(input: &str) -> Vec<KeyInput> {
    let chars: Vec<char> = input.chars().collect();
    let mut keys = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\u{1b}' && i + 2 < chars.len() && chars[i + 1] == '[' {
            match chars[i + 2] {
                'A' => keys.push(KeyInput::Up),
                'B' => keys.push(KeyInput::Down),
                'C' => keys.push(KeyInput::Right),
                'D' => keys.push(KeyInput::Left),
                _ => keys.push(KeyInput::Escape),
            }
            i += 3;
            continue;
        }

        keys.push(map_raw_char(chars[i]));
        i += 1;
    }

    keys
}

fn map_raw_char(ch: char) -> KeyInput {
    match ch {
        '\u{1b}' => KeyInput::Escape,
        '\r' => KeyInput::Enter,
        '\u{7f}' => KeyInput::Backspace,
        '\u{01}' => KeyInput::CtrlA,
        '\u{02}' => KeyInput::CtrlB,
        '\u{03}' => KeyInput::CtrlC,
        '\u{04}' => KeyInput::CtrlD,
        '\u{05}' => KeyInput::CtrlE,
        '\u{06}' => KeyInput::CtrlF,
        '\u{07}' => KeyInput::CtrlG,
        '\u{0A}' => KeyInput::CtrlJ,
        '\u{0B}' => KeyInput::CtrlK,
        '\u{0E}' => KeyInput::CtrlN,
        '\u{10}' => KeyInput::CtrlP,
        '\u{12}' => KeyInput::CtrlR,
        '\u{14}' => KeyInput::CtrlT,
        '\u{17}' => KeyInput::CtrlW,
        _ => KeyInput::Char(ch),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_test_keys, KeyInput};

    #[test]
    fn input_parse_test_keys_enter_keyword() {
        assert_eq!(parse_test_keys("ENTER"), vec![KeyInput::Enter]);
    }

    #[test]
    fn input_parse_test_keys_multiple_tokens() {
        assert_eq!(
            parse_test_keys("UP,UP,DOWN,ENTER"),
            vec![KeyInput::Up, KeyInput::Up, KeyInput::Down, KeyInput::Enter]
        );
    }

    #[test]
    fn input_parse_test_keys_type_expansion() {
        assert_eq!(
            parse_test_keys("CTRL-D,TYPE=YES,ENTER"),
            vec![
                KeyInput::CtrlD,
                KeyInput::Char('Y'),
                KeyInput::Char('E'),
                KeyInput::Char('S'),
                KeyInput::Enter,
            ]
        );
    }

    #[test]
    fn input_parse_test_keys_drops_multi_char_non_keyword() {
        assert_eq!(parse_test_keys("test,ESC"), vec![KeyInput::Escape]);
    }

    #[test]
    fn input_parse_test_keys_keeps_single_char_non_keyword() {
        assert_eq!(
            parse_test_keys("d,ESC"),
            vec![KeyInput::Char('d'), KeyInput::Escape]
        );
    }

    #[test]
    fn input_parse_test_keys_ctrl_j_extension() {
        assert_eq!(
            parse_test_keys("CTRL-J,ENTER"),
            vec![KeyInput::CtrlJ, KeyInput::Enter]
        );
    }

    #[test]
    fn input_parse_test_keys_raw_mode_chars() {
        assert_eq!(
            parse_test_keys("hello"),
            vec![
                KeyInput::Char('h'),
                KeyInput::Char('e'),
                KeyInput::Char('l'),
                KeyInput::Char('l'),
                KeyInput::Char('o')
            ]
        );
    }

    #[test]
    fn input_parse_test_keys_raw_escape_arrow_sequence() {
        assert_eq!(parse_test_keys("\u{1b}[A"), vec![KeyInput::Up]);
    }
}
