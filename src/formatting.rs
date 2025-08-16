use std::num::ParseIntError;

use colored::{Color, ColoredString, Style, Styles};

#[derive(Debug, PartialEq, Default)]
pub struct FormatString {
    nodes: Vec<FormatStringNode>,
}
#[derive(thiserror::Error, Debug)]
pub enum FormatError {
    #[error("Trailing Bracket")]
    TrailingBracket,
    #[error("Unmatched Bracket")]
    UnmatchedBracket,
    #[error("Invalid Hex code")]
    InvalidHex(#[from] ParseIntError),
    #[error("InvalidStyle")]
    InvalidStyle,
}
#[derive(Debug, PartialEq)]
enum FormatStringNode {
    String(String),
    Formatted(FormatNode),
}
#[derive(Debug, PartialEq, Default)]
struct FormatNode {
    fg: Option<Color>,
    bg: Option<Color>,
    style: Style,
    children: Vec<FormatStringNode>,
}

#[derive(Debug, PartialEq)]
enum Token {
    OpenContent,
    CloseContent,
    OpenStyle,
    CloseStyle,
    Literal(String),
}

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}
#[derive(PartialEq)]
enum ParsePhase {
    Content,
    Style,
}

impl Parser {
    fn tokenize(input: String) -> Self {
        let mut tokens = vec![];
        let mut tmp = String::new();
        let mut escaping = false;
        input.chars().for_each(|ch| match ch {
            '(' | ')' | '[' | ']' if !escaping => {
                if !tmp.is_empty() {
                    tokens.push(Token::Literal(tmp.clone()));
                    tmp.clear();
                }
                match ch {
                    '(' => tokens.push(Token::OpenContent),
                    ')' => tokens.push(Token::CloseContent),
                    '[' => tokens.push(Token::OpenStyle),
                    ']' => tokens.push(Token::CloseStyle),
                    _ => unreachable!(),
                }
            }
            '\\' if !escaping => {
                escaping = true;
            }
            c => {
                tmp.push(c);
                if escaping {
                    escaping = false;
                }
            }
        });
        if !tmp.is_empty() {
            tokens.push(Token::Literal(tmp));
        }
        Self { tokens, pos: 0 }
    }
    fn parse(mut self) -> Result<FormatString, FormatError> {
        let mut str = FormatString::default();
        while let Some(token) = self.tokens.get(self.pos) {
            match token {
                Token::CloseContent | Token::OpenStyle | Token::CloseStyle => {
                    return Err(FormatError::UnmatchedBracket);
                }
                Token::OpenContent => str
                    .nodes
                    .push(FormatStringNode::Formatted(self.parse_formatted()?)),
                Token::Literal(lit) => str.nodes.push(FormatStringNode::String(lit.clone())),
            }
            self.pos += 1;
        }
        Ok(str)
    }
    /// Parses an `Token::OpenContent` until it finds a matching `Token::CloseStyle`
    fn parse_formatted(&mut self) -> Result<FormatNode, FormatError> {
        self.pos += 1;
        let mut res = FormatNode::default();
        let mut phase = ParsePhase::Content;
        'main: loop {
            match self.tokens.get(self.pos) {
                Some(token) => match token {
                    Token::OpenContent => match phase {
                        ParsePhase::Content => res
                            .children
                            .push(FormatStringNode::Formatted(self.parse_formatted()?)),
                        ParsePhase::Style => break Err(FormatError::UnmatchedBracket),
                    },
                    Token::CloseContent => match phase {
                        ParsePhase::Content => {
                            phase = ParsePhase::Style;
                        }
                        ParsePhase::Style => break Err(FormatError::UnmatchedBracket),
                    },
                    Token::OpenStyle => match phase {
                        ParsePhase::Content => break Err(FormatError::UnmatchedBracket),
                        ParsePhase::Style => (),
                    },
                    Token::CloseStyle => match phase {
                        ParsePhase::Content => break Err(FormatError::UnmatchedBracket),
                        ParsePhase::Style => break Ok(res),
                    },
                    Token::Literal(lit) => match phase {
                        ParsePhase::Content => {
                            res.children.push(FormatStringNode::String(lit.clone()))
                        }
                        ParsePhase::Style => {
                            let mut style = Style::default();

                            for style_str in lit.split(" ") {
                                if let Some(fg) = style_str.strip_prefix("fg:") {
                                    if let Some(hex) = fg.strip_prefix("#") {
                                        _ = res.fg.get_or_insert(Color::TrueColor {
                                            r: u8::from_str_radix(&hex[0..2], 16)?,
                                            g: u8::from_str_radix(&hex[2..4], 16)?,
                                            b: u8::from_str_radix(&hex[4..6], 16)?,
                                        })
                                    } else {
                                        _ = res.fg.get_or_insert(Color::from(fg))
                                    }
                                } else if let Some(bg) = style_str.strip_prefix("bg:") {
                                    if let Some(hex) = bg.strip_prefix("#") {
                                        _ = res.bg.get_or_insert(Color::TrueColor {
                                            r: u8::from_str_radix(&hex[0..2], 16)?,
                                            g: u8::from_str_radix(&hex[2..4], 16)?,
                                            b: u8::from_str_radix(&hex[4..6], 16)?,
                                        })
                                    } else {
                                        _ = res.bg.get_or_insert(Color::from(bg))
                                    }
                                } else {
                                    style |= match style_str.to_lowercase().as_ref() {
                                        "clear" => Styles::Clear,
                                        "bold" => Styles::Bold,
                                        "dimmed" => Styles::Dimmed,
                                        "underline" => Styles::Underline,
                                        "reversed" => Styles::Reversed,
                                        "italic" => Styles::Italic,
                                        "blink" => Styles::Blink,
                                        "hidden" => Styles::Hidden,
                                        "strikethrough" => Styles::Strikethrough,
                                        _ => break 'main Err(FormatError::InvalidStyle),
                                    };
                                }
                            }

                            res.style = style;
                        }
                    },
                },
                None => break Err(FormatError::TrailingBracket),
            }
            self.pos += 1;
        }
    }
}

impl TryFrom<String> for FormatString {
    type Error = FormatError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parser = Parser::tokenize(value);
        parser.parse()
    }
}
impl From<FormatString> for ColoredString {
    fn from(value: FormatString) -> Self {
        todo!()
    }
}

#[cfg(test)]
mod unit_tests {
    use colored::Styles;

    use super::*;

    #[test]
    fn test_tokenizer() {
        let test = "hi :3 (this is part of a format string (and another)[underline] )[bold] and this is not".to_string();
        let result = Parser::tokenize(test);
        assert_eq!(
            result.tokens,
            [
                Token::Literal("hi :3 ".to_string()),
                Token::OpenContent,
                Token::Literal("this is part of a format string ".to_string()),
                Token::OpenContent,
                Token::Literal("and another".to_string()),
                Token::CloseContent,
                Token::OpenStyle,
                Token::Literal("underline".to_string()),
                Token::CloseStyle,
                Token::Literal(" ".to_string()),
                Token::CloseContent,
                Token::OpenStyle,
                Token::Literal("bold".to_string()),
                Token::CloseStyle,
                Token::Literal(" and this is not".to_string())
            ]
        )
    }
    #[test]
    fn test_tokenizer_escaped() {
        let test = "hi :3 (this is part of a format string (and another)[underline] )[bold] and this is not \\(this is escaped\\)".to_string();
        let result = Parser::tokenize(test);
        assert_eq!(
            result.tokens,
            [
                Token::Literal("hi :3 ".to_string()),
                Token::OpenContent,
                Token::Literal("this is part of a format string ".to_string()),
                Token::OpenContent,
                Token::Literal("and another".to_string()),
                Token::CloseContent,
                Token::OpenStyle,
                Token::Literal("underline".to_string()),
                Token::CloseStyle,
                Token::Literal(" ".to_string()),
                Token::CloseContent,
                Token::OpenStyle,
                Token::Literal("bold".to_string()),
                Token::CloseStyle,
                Token::Literal(" and this is not (this is escaped)".to_string())
            ]
        )
    }

    #[test]
    fn basic_from_string() {
        let test = "hi guys".to_string();
        let test_struct = FormatString {
            nodes: vec![FormatStringNode::String("hi guys".to_string())],
        };
        let formatted = FormatString::try_from(test);
        assert_eq!(formatted.unwrap(), test_struct);
    }
    #[test]
    fn basic_from_formatted_string() {
        let test = "hi guys (this one is green)[fg:green] i am not green".to_string();
        let test_struct = FormatString {
            nodes: vec![
                FormatStringNode::String("hi guys ".to_string()),
                FormatStringNode::Formatted(FormatNode {
                    children: vec![FormatStringNode::String("this one is green".to_string())],
                    fg: Some(Color::Green),
                    ..Default::default()
                }),
                FormatStringNode::String(" i am not green".to_string()),
            ],
        };
        let formatted = FormatString::try_from(test);
        assert_eq!(formatted.unwrap(), test_struct);
    }
    #[test]
    fn basic_from_formatted_nested_string() {
        let test = "hi (guys)[underline] (this one is green( and this one is also underlined and bold!)[underline bold])[fg:green] i am not green".to_string();
        let test_struct = FormatString {
            nodes: vec![
                FormatStringNode::String("hi ".to_string()),
                FormatStringNode::Formatted(FormatNode {
                    children: vec![FormatStringNode::String("guys".to_string())],
                    style: Style::from(Styles::Underline),
                    ..Default::default()
                }),
                FormatStringNode::String(" ".to_string()),
                FormatStringNode::Formatted(FormatNode {
                    children: vec![
                        FormatStringNode::String("this one is green".to_string()),
                        FormatStringNode::Formatted(FormatNode {
                            children: vec![FormatStringNode::String(
                                " and this one is also underlined and bold!".to_string(),
                            )],
                            style: Style::from_iter([Styles::Underline, Styles::Bold]),
                            ..Default::default()
                        }),
                    ],
                    fg: Some(Color::Green),
                    ..Default::default()
                }),
                FormatStringNode::String(" i am not green".to_string()),
            ],
        };
        let formatted = FormatString::try_from(test);
        assert_eq!(formatted.unwrap(), test_struct);
    }

    #[test]
    fn parse_error_bracket_mismatch() {
        let parser1 = Parser::tokenize("this (text isnt[fg:pink] working".to_string());
        let parser2 = Parser::tokenize("this text isnt)[fg:pink] working".to_string());
        let parser3 = Parser::tokenize("this (text isnt)fg:pink] working".to_string());
        assert!(matches!(
            parser1.parse().unwrap_err(),
            FormatError::UnmatchedBracket
        ));
        assert!(matches!(
            parser2.parse().unwrap_err(),
            FormatError::UnmatchedBracket
        ));
        assert!(matches!(
            parser3.parse().unwrap_err(),
            FormatError::UnmatchedBracket
        ));
    }
    #[test]
    fn parse_error_bracket_trailing() {
        let parser = Parser::tokenize("this (text isnt working)[underline".to_string());
        let parser2 = Parser::tokenize("this (text isnt working".to_string());
        assert!(matches!(
            parser.parse().unwrap_err(),
            FormatError::TrailingBracket
        ));
        assert!(matches!(
            parser2.parse().unwrap_err(),
            FormatError::TrailingBracket
        ));
    }
    #[test]
    fn parse_error_bad_hex() {
        let mismatch = "this hex no (workie)[underline fg:#ZZAA11]".to_string();
        let parser = Parser::tokenize(mismatch);
        assert!(matches!(
            parser.parse().unwrap_err(),
            FormatError::InvalidHex(_)
        ))
    }
    #[test]
    fn parse_error_bad_style() {
        let mismatch = "this style no (workie)[cle]".to_string();
        let parser = Parser::tokenize(mismatch);
        assert!(matches!(
            parser.parse().unwrap_err(),
            FormatError::InvalidStyle
        ))
    }
}
