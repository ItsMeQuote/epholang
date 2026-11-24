//! EphoFMT preserves expression spelling and comment contents while normalizing
//! layout, line endings and statement separators.

use std::ops::Range;

use super::{Diagnostic, EResult, Lexed, Parser, Span, Token, TokenKind, lex_source};

enum PieceKind {
    Code,
    Colon,
    NoValueMarker,
    DiscardEquals,
    Comment,
    Separator,
}

struct Piece {
    bytes: Range<usize>,
    kind: PieceKind,
}

/// Format syntactically valid Epholang source. Static checking is independent.
pub(crate) fn format(source: &str) -> EResult<String> {
    let Lexed {
        tokens,
        comments,
        line_depths,
    } = lex_source(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()?;

    let breaks = statement_breaks(&parser.tokens);
    let mut commas = parser.trailing_commas;
    commas.sort_unstable();

    let mut after_discard = false;
    let mut pieces: Vec<_> = parser
        .tokens
        .into_iter()
        .filter(Token::has_source_text)
        .map(|token| {
            let kind = if token.symbol(";") {
                PieceKind::Separator
            } else if token.symbol(":") {
                PieceKind::Colon
            } else if token.symbol("-|") {
                PieceKind::NoValueMarker
            } else if after_discard && token.symbol("=") {
                PieceKind::DiscardEquals
            } else {
                PieceKind::Code
            };
            after_discard = token.kind == TokenKind::Discard;
            Piece {
                bytes: token.bytes,
                kind,
            }
        })
        .chain(comments.into_iter().map(|bytes| Piece {
            bytes,
            kind: PieceKind::Comment,
        }))
        .collect();
    pieces.sort_unstable_by_key(|piece| piece.bytes.start);

    let formatted = render(source, pieces, &line_depths, &breaks, &commas);
    if formatted != source {
        let result =
            lex_source(&formatted).and_then(|lexed| Parser::new(lexed.tokens).parse_program());
        if let Err(error) = result {
            return Err(Diagnostic::new(
                Span::new(1, 1, 1),
                format!("Formatting would produce invalid syntax: {}", error.message),
            ));
        }
    }
    Ok(formatted)
}

// Only the first semicolon between two statements on the same physical line
// becomes a newline. Empty and trailing separators vanish.
fn statement_breaks(tokens: &[Token]) -> Vec<usize> {
    let mut breaks = Vec::new();
    let mut code_line = None;
    let mut separator = None;
    for token in tokens.iter().filter(|token| token.has_source_text()) {
        if token.symbol(";") {
            if code_line == Some(token.span.line) {
                separator.get_or_insert(token.bytes.start);
            }
        } else {
            if let Some(byte) = separator.take() {
                if code_line == Some(token.span.line) {
                    breaks.push(byte);
                }
            }
            code_line = Some(token.span.line);
        }
    }
    breaks
}

fn render(
    source: &str,
    pieces: Vec<Piece>,
    line_depths: &[Option<usize>],
    breaks: &[usize],
    commas: &[usize],
) -> String {
    let mut output = String::with_capacity(source.len());
    let mut scanned = 0;
    let mut line = 0;
    let mut last_end = 0;
    let mut split_statement = false;
    let mut after_discard_equals = false;
    for piece in pieces {
        let start = piece.bytes.start;
        if matches!(piece.kind, PieceKind::Separator) {
            split_statement |= breaks.binary_search(&start).is_ok();
            continue;
        }

        line += source.as_bytes()[scanned..start]
            .iter()
            .filter(|&&ch| ch == b'\n')
            .count();
        scanned = start;
        let gap = &source[last_end..start];
        let line_breaks = gap.bytes().filter(|&ch| ch == b'\n').count();
        if output.is_empty() || split_statement || line_breaks > 0 {
            if !output.is_empty() {
                output.push('\n');
                if line_breaks > 1 {
                    output.push('\n');
                }
            }
            let depth = line_depths.get(line).copied().flatten().unwrap_or_else(|| {
                // Comment-only lines do not have a syntactic depth. Keep
                // their typed tabs while preserving the comment itself.
                gap.rsplit('\n')
                    .next()
                    .unwrap_or("")
                    .bytes()
                    .filter(|&ch| ch == b'\t')
                    .count()
            });
            output.extend(std::iter::repeat_n('\t', depth));
        } else if after_discard_equals
            || matches!(
                piece.kind,
                PieceKind::NoValueMarker | PieceKind::DiscardEquals
            )
            || (!matches!(piece.kind, PieceKind::Colon) && gap.contains([' ', '\t']))
        {
            output.push(' ');
        }

        output.push_str(&source[piece.bytes.clone()]);
        if commas.binary_search(&piece.bytes.end).is_ok() {
            output.push(',');
        }
        after_discard_equals = matches!(piece.kind, PieceKind::DiscardEquals);
        last_end = piece.bytes.end;
        split_statement = false;
    }

    if !output.is_empty() {
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn separates_semicolon_statements() {
        assert_eq!(
            format("str value := \"hi\"; ; Output(value)").unwrap(),
            "str value := \"hi\"\nOutput(value)\n"
        );
    }

    #[test]
    fn removes_trailing_semicolon() {
        assert_eq!(format("Output(\"hi\");").unwrap(), "Output(\"hi\")\n");
    }

    #[test]
    fn formats_indented_blocks_and_preserves_blank_lines_between_branches() {
        let source = "if ready :\r\n\tOutput(\"yes\");\r\n\r\nelse:\r\n\tOutput(\"no\")";
        assert_eq!(
            format(source).unwrap(),
            "if ready:\n\tOutput(\"yes\")\n\nelse:\n\tOutput(\"no\")\n"
        );
    }

    #[test]
    fn preserves_strings_and_nested_comments_byte_for_byte() {
        let source = concat!(
            r##"Output("a;b #{strings:Repeat("}\"#{name}", 2)}") /* x;y"##,
            "\r\n  /* nested { */ */\r\n"
        );
        let expected = source.strip_suffix("\r\n").unwrap().to_string() + "\n";
        assert_eq!(format(source).unwrap(), expected);
    }

    #[test]
    fn formats_discard_and_no_value_headers() {
        let source = "function _F()-| :\r\n\t_=1; _ /*why*/=/*value*/2\r\n_F()";
        let expected = "function _F() -|:\n\t_ = 1\n\t_ /*why*/ = /*value*/2\n_F()\n";
        assert_eq!(format(source).unwrap(), expected);
        assert_eq!(format(expected).unwrap(), expected);
        for bad in [
            "_ := F()",
            "Output(_)",
            "int _ := 1",
            "function F() - |:\n\tOutput(1)",
        ] {
            assert!(format(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn is_idempotent() {
        let once = format("function Hello() -> none:\n\tOutput(\"hi\"); return none null").unwrap();
        assert_eq!(once, format(&once).unwrap());
    }

    #[test]
    fn preserves_compact_lists_and_distinguishes_header_colons() {
        let source = "if Length(list<int>{1, 2}) == 2:\n\tOutput(list<int>{1, 2})\n";
        assert_eq!(format(source).unwrap(), source);
    }

    #[test]
    fn adds_one_comma_after_a_multiline_list_element_and_before_trivia() {
        let source = "Output(\nlist<int>{\n1,\n2 /* keep */\n}\n)\n";
        let expected = "Output(\n\tlist<int>{\n\t\t1,\n\t\t2, /* keep */\n\t}\n)\n";
        assert_eq!(format(source).unwrap(), expected);
        assert_eq!(format(expected).unwrap(), expected);

        let source = "Output(list<int>{\n\t/* empty */\n})\n";
        assert_eq!(format(source).unwrap(), source);
    }

    #[test]
    fn syntax_errors_have_source_positions() {
        for source in ["Output(1))\n", "int x :=\n", "if true {\n\tOutput(1)\n}\n"] {
            let err = format(source).unwrap_err();
            assert_eq!(err.span.line, 1, "{source}");
            assert!(err.span.column > 1, "{source}");
        }

        let err = format("if true:\n \tOutput(1)").unwrap_err();
        assert_eq!(err.span, super::Span::new(2, 1, 2));
        assert!(err.message.contains("mix tabs and spaces"));
    }
}
