//! A `{` left open in styles.css nests every later rule inside it, and the browser then ignores
//! all of them. An upstream merge left `.p2-address` open, which unstyled the whole player, the
//! transcript search, and everything else after it.

use wasm_bindgen_test::wasm_bindgen_test;

const STYLES: &str = include_str!("../../static/styles.css");

/// Line numbers of every `{` that is never closed. Comments and quoted strings are skipped. A `}`
/// with nothing open is ignored: the browser drops it with the rule it sits in.
fn unclosed_blocks(css: &str) -> Vec<usize> {
    let mut open = Vec::new();
    let mut line = 1;
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                    }
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            '"' | '\'' => {
                while let Some(s) = chars.next() {
                    match s {
                        '\\' => {
                            chars.next();
                        }
                        '\n' => {
                            line += 1;
                            break;
                        }
                        s if s == c => break,
                        _ => {}
                    }
                }
            }
            '{' => open.push(line),
            '}' => {
                open.pop();
            }
            _ => {}
        }
    }
    open
}

#[wasm_bindgen_test]
fn every_rule_in_the_stylesheet_is_closed() {
    assert_eq!(unclosed_blocks(STYLES), Vec::<usize>::new(), "web/static/styles.css: `{{` never closed on these lines");
}

#[wasm_bindgen_test]
fn unclosed_blocks_skip_comments_and_strings() {
    assert_eq!(unclosed_blocks(".a {\n  color: red;\n.b { }\n"), vec![1]);
    assert_eq!(unclosed_blocks(".a { }\n}\n.b {\n"), vec![3]);
    assert!(unclosed_blocks("/* .a { */\n.b::after { content: \"{\"; }\n.c { content: '\\'{'; }\n").is_empty());
}
