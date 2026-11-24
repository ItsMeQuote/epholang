mod formatter;

use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

static OUTPUT_LINE_OPEN: AtomicBool = AtomicBool::new(false);

// ============================================================
// Spans and diagnostics
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    line: usize,
    column: usize,
    length: usize,
}

impl Span {
    fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }

    fn through(self, last: Span) -> Self {
        if self.line == last.line {
            Self {
                line: self.line,
                column: self.column,
                length: (last.column + last.length)
                    .saturating_sub(self.column)
                    .max(1),
            }
        } else {
            self
        }
    }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    span: Span,
    message: String,
}

impl Diagnostic {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

type EResult<T> = Result<T, Diagnostic>;

fn print_diagnostic(prefix: &str, path: &str, source: &str, diagnostic: &Diagnostic) {
    let span = diagnostic.span;

    eprintln!("{}: {}", prefix, diagnostic.message);
    eprintln!(" --> {}:{}:{}", path, span.line, span.column);

    let raw_line = source
        .split('\n')
        .nth(span.line.saturating_sub(1))
        .unwrap_or("");

    let raw_line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

    // A tab counts as one column in source positions,
    // but is displayed as four spaces.
    let indentation: usize = raw_line
        .chars()
        .take(span.column.saturating_sub(1))
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum();

    let underline_length: usize = raw_line
        .chars()
        .skip(span.column.saturating_sub(1))
        .take(span.length.max(1))
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum();

    let displayed_line = raw_line.replace('\t', "    ");
    let width = span.line.to_string().len();
    let gutter = " ".repeat(width);

    eprintln!("{} |", gutter);
    eprintln!("{:>width$} | {}", span.line, displayed_line, width = width);
    eprintln!(
        "{} | {}{}",
        gutter,
        " ".repeat(indentation),
        "^".repeat(underline_length.max(1))
    );
}

// ============================================================
// Lexer
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum StringPart {
    Text(String),
    Code(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Identifier(String),
    Discard,
    Integer(i64),
    Float(f64),
    String(String),
    Interpolated(Vec<StringPart>),
    Symbol(String),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    span: Span,
    bytes: Range<usize>,
}

impl Token {
    fn symbol(&self, symbol: &str) -> bool {
        matches!(&self.kind, TokenKind::Symbol(text) if text == symbol)
    }

    fn has_source_text(&self) -> bool {
        !matches!(
            self.kind,
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Eof
        )
    }
}

#[derive(Clone, Copy)]
struct SourceChar {
    ch: char,
    span: Span,
    byte: usize,
}

struct Lexed {
    tokens: Vec<Token>,
    comments: Vec<Range<usize>>,
    line_depths: Vec<Option<usize>>,
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<SourceChar>,
    pos: usize,
    eof: Span,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        let (mut line, mut column) = (1, 1);
        let chars = source
            .char_indices()
            .map(|(byte, ch)| {
                let span = Span::new(line, column, 1);
                if ch == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
                SourceChar { ch, span, byte }
            })
            .collect();
        Self {
            source,
            chars,
            pos: 0,
            eof: Span::new(line, column, 1),
        }
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).map(|c| c.ch)
    }

    fn pair(&self, first: char, second: char) -> bool {
        self.char_at(0) == Some(first) && self.char_at(1) == Some(second)
    }

    fn byte_at(&self, pos: usize) -> usize {
        self.chars.get(pos).map_or(self.source.len(), |c| c.byte)
    }

    fn token(&self, kind: TokenKind, start: usize) -> Token {
        let span = if start == self.pos {
            self.chars.get(start).map_or(self.eof, |c| c.span)
        } else {
            self.chars[start]
                .span
                .through(self.chars[self.pos - 1].span)
        };
        Token {
            kind,
            span,
            bytes: self.byte_at(start)..self.byte_at(self.pos),
        }
    }

    fn is_newline(&self) -> bool {
        self.char_at(0) == Some('\n') || self.pair('\r', '\n')
    }

    fn scan(mut self) -> EResult<Lexed> {
        let mut tokens = Vec::new();
        let mut comments = Vec::new();
        while let Some(ch) = self.char_at(0) {
            if self.is_newline() {
                let start = self.pos;
                self.pos += if ch == '\r' { 2 } else { 1 };
                tokens.push(self.token(TokenKind::Newline, start));
            } else if matches!(ch, ' ' | '\t') {
                self.pos += 1;
            } else if self.pair('/', '*') {
                let start = self.pos;
                self.scan_comment(Some(&mut tokens))?;
                comments.push(self.byte_at(start)..self.byte_at(self.pos));
            } else {
                tokens.push(self.scan_code_token(0)?);
            }
        }
        let eof = self.token(TokenKind::Eof, self.pos);
        apply_layout(self.source, tokens, comments, eof)
    }

    fn scan_comment(&mut self, mut lines: Option<&mut Vec<Token>>) -> EResult<()> {
        let opening = self.chars[self.pos]
            .span
            .through(self.chars[self.pos + 1].span);
        self.pos += 2;
        let mut depth = 1;

        while let Some(ch) = self.char_at(0) {
            if self.pair('/', '*') {
                depth += 1;
                self.pos += 2;
            } else if self.pair('*', '/') {
                depth -= 1;
                self.pos += 2;
                if depth == 0 {
                    return Ok(());
                }
            } else if self.is_newline() {
                let start = self.pos;
                self.pos += if ch == '\r' { 2 } else { 1 };
                if let Some(tokens) = lines.as_deref_mut() {
                    tokens.push(self.token(TokenKind::Newline, start));
                } else {
                    return Err(Diagnostic::new(
                        self.chars[start].span,
                        "Interpolation must be on one line",
                    ));
                }
            } else {
                self.pos += 1;
            }
        }

        Err(Diagnostic::new(opening, "Unclosed block comment"))
    }

    fn scan_string(&mut self, depth: usize) -> EResult<TokenKind> {
        let opening = self.chars[self.pos].span;
        self.pos += 1;
        let mut parts = Vec::new();
        let mut text = String::new();

        while let Some(ch) = self.char_at(0) {
            if self.is_newline() {
                return Err(Diagnostic::new(
                    self.chars[self.pos].span,
                    "Newline inside string",
                ));
            }

            match ch {
                '"' => {
                    self.pos += 1;
                    return if parts.is_empty() {
                        Ok(TokenKind::String(text))
                    } else {
                        if !text.is_empty() {
                            parts.push(StringPart::Text(text));
                        }
                        Ok(TokenKind::Interpolated(parts))
                    };
                }
                '\\' if self.char_at(1).is_some() => {
                    self.pos += 1;
                    if self.is_newline() {
                        return Err(Diagnostic::new(
                            self.chars[self.pos].span,
                            "Newline inside string",
                        ));
                    }
                    let escaped = self.chars[self.pos].ch;
                    match escaped {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        '"' | '\\' | '#' | '}' => text.push(escaped),
                        other => {
                            text.push('\\');
                            text.push(other);
                        }
                    }
                    self.pos += 1;
                }
                '#' if self.char_at(1) == Some('{') => {
                    if !text.is_empty() {
                        parts.push(StringPart::Text(std::mem::take(&mut text)));
                    }
                    let opening = self.chars[self.pos]
                        .span
                        .through(self.chars[self.pos + 1].span);
                    self.pos += 2;
                    // Bound the recursive consumers of an interpolated token as well as the lexer.
                    if depth >= 128 {
                        return Err(Diagnostic::new(
                            opening,
                            "Interpolation nesting is too deep",
                        ));
                    }
                    parts.push(StringPart::Code(
                        self.scan_interpolation(depth + 1, opening)?,
                    ));
                }
                _ => {
                    text.push(ch);
                    self.pos += 1;
                }
            }
        }

        Err(Diagnostic::new(opening, "Unclosed string"))
    }

    fn scan_interpolation(&mut self, depth: usize, opening: Span) -> EResult<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut delimiters = Delimiters::default();

        while let Some(ch) = self.char_at(0) {
            if self.is_newline() {
                return Err(Diagnostic::new(
                    self.chars[self.pos].span,
                    "Interpolation must be on one line",
                ));
            }

            if matches!(ch, ' ' | '\t') {
                self.pos += 1;
            } else if self.pair('/', '*') {
                self.scan_comment(None)?;
            } else if ch == '}' && delimiters.stack.last() != Some(&Delimiter::List) {
                if let Some(delimiter) = delimiters.stack.last() {
                    return Err(Diagnostic::new(
                        self.chars[self.pos].span,
                        format!("Expected '{}' inside interpolation", delimiter.closing()),
                    ));
                }
                if tokens.is_empty() {
                    return Err(Diagnostic::new(opening, "Empty interpolation"));
                }
                tokens.push(self.token(TokenKind::Eof, self.pos));
                self.pos += 1;
                return Ok(tokens);
            } else {
                let token = self.scan_code_token(depth)?;
                delimiters.accept(&token);
                tokens.push(token);
            }
        }

        Err(Diagnostic::new(opening, "Unclosed interpolation"))
    }

    fn scan_code_token(&mut self, depth: usize) -> EResult<Token> {
        let start = self.pos;
        let ch = self.chars[start].ch;

        let kind = if ch == '"' {
            self.scan_string(depth)?
        } else if ch.is_ascii_digit()
            || (ch == '.' && self.char_at(1).is_some_and(|c| c.is_ascii_digit()))
        {
            return self.scan_number();
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            while self
                .char_at(0)
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                self.pos += 1;
            }
            let name = &self.source[self.byte_at(start)..self.byte_at(self.pos)];
            if name == "_" {
                TokenKind::Discard
            } else {
                TokenKind::Identifier(name.into())
            }
        } else {
            let next = self.char_at(1);
            if matches!((ch, next), ('<' | '>', Some('='))) {
                self.pos += 2;
                return Err(Diagnostic::new(
                    self.token(TokenKind::Eof, start).span,
                    format!("Operator '{}=' is not supported", ch),
                ));
            }
            if matches!(
                (ch, next),
                ('=' | '!' | ':', Some('='))
                    | ('&', Some('&'))
                    | ('|', Some('|'))
                    | ('-', Some('>' | '|'))
            ) {
                self.pos += 2;
            } else if "+-*/%^(){}[],;=<>!:|".contains(ch) {
                self.pos += 1;
            } else {
                return Err(Diagnostic::new(
                    self.chars[start].span,
                    format!("Unexpected character '{}'", ch.escape_default()),
                ));
            }
            TokenKind::Symbol(self.source[self.byte_at(start)..self.byte_at(self.pos)].into())
        };

        Ok(self.token(kind, start))
    }

    fn scan_number(&mut self) -> EResult<Token> {
        let start = self.pos;
        while self.char_at(0).is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let float =
            self.char_at(0) == Some('.') && self.char_at(1).is_some_and(|c| c.is_ascii_digit());
        if float {
            self.pos += 1;
            while self.char_at(0).is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }

        let mut token = self.token(TokenKind::Eof, start);
        let text = &self.source[token.bytes.clone()];
        token.kind = if float {
            let value = text
                .parse::<f64>()
                .map_err(|_| Diagnostic::new(token.span, "Invalid floating-point number"))?;
            if !value.is_finite() {
                return Err(Diagnostic::new(
                    token.span,
                    "Floating-point number out of range",
                ));
            }
            TokenKind::Float(value)
        } else {
            let value = text
                .parse::<i64>()
                .map_err(|_| Diagnostic::new(token.span, "Integer literal out of i64 range"))?;
            TokenKind::Integer(value)
        };

        Ok(token)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delimiter {
    Paren,
    Bracket,
    List,
    Type { list_literal: bool },
}

impl Delimiter {
    fn closing(self) -> &'static str {
        match self {
            Self::Paren => ")",
            Self::Bracket => "]",
            Self::List => "}",
            Self::Type { .. } => ">",
        }
    }
}

#[derive(Default)]
struct Delimiters {
    stack: Vec<Delimiter>,
    type_start: Option<bool>,
    after_list_type: bool,
}

impl Delimiters {
    fn closes(&self, token: &Token) -> bool {
        self.stack.last().is_some_and(|d| token.symbol(d.closing()))
    }

    fn accept(&mut self, token: &Token) {
        let type_start = self.type_start.take();
        let after_list_type = std::mem::take(&mut self.after_list_type);
        if self.closes(token) {
            self.after_list_type = self.stack.pop() == Some(Delimiter::Type { list_literal: true });
        } else if token.symbol("(") {
            self.stack.push(Delimiter::Paren);
        } else if token.symbol("[") {
            self.stack.push(Delimiter::Bracket);
        } else if token.symbol("{") && after_list_type {
            self.stack.push(Delimiter::List);
        } else if token.symbol("<") {
            if let Some(list_literal) = type_start {
                self.stack.push(Delimiter::Type { list_literal });
            }
        } else if let TokenKind::Identifier(name) = &token.kind {
            self.type_start = match name.as_str() {
                "list" => Some(true),
                "ls" => Some(false),
                _ => None,
            };
        }
    }
}

fn apply_layout(
    source: &str,
    raw: Vec<Token>,
    comments: Vec<Range<usize>>,
    eof: Token,
) -> EResult<Lexed> {
    let mut tokens = Vec::with_capacity(raw.len() + 1);
    let mut levels = vec![0];
    let mut delimiters = Delimiters::default();
    let mut newline = None;
    let mut lines = Vec::new();
    let mut offset = 0;
    let mut comment_index = 0;
    let mut comment_prefix = None;
    let mut physical_lines = source.split('\n').enumerate().map(|(row, line)| {
        let start = offset;
        offset += line.len() + 1;
        let end = start
            + line
                .bytes()
                .take_while(|c| matches!(c, b' ' | b'\t'))
                .count();
        let prefix = comment_prefix.take().unwrap_or((row + 1, start..end));
        // A physical line that starts inside a comment has no independent
        // source indentation. Code following its closer uses the prefix
        // from where that comment opened.
        while let Some(comment) = comments.get(comment_index).filter(|c| c.start < offset) {
            if comment.end > offset {
                comment_prefix = Some(prefix.clone());
                break;
            }
            comment_index += 1;
        }
        prefix
    });

    let mut raw = raw.into_iter();
    while let Some(token) = raw.next() {
        if token.kind == TokenKind::Newline {
            if newline.is_none() {
                newline = Some(token);
            }
            continue;
        }

        if token.span.line > lines.len() {
            let (prefix_line, prefix) = physical_lines
                .nth(token.span.line - lines.len() - 1)
                .unwrap();
            let bytes = &source.as_bytes()[prefix.clone()];
            if bytes.contains(&b' ') {
                let message = if bytes.contains(&b'\t') {
                    "Indentation cannot mix tabs and spaces"
                } else {
                    "Indentation must use tabs"
                };
                return Err(Diagnostic::new(
                    Span::new(prefix_line, 1, bytes.len()),
                    message,
                ));
            }

            let level = *levels.last().unwrap();
            let depth = bytes.len();
            let logical_continuation = delimiters.stack.is_empty()
                && (token.symbol("&&") || token.symbol("||"))
                && depth >= level
                && tokens.last().is_some_and(|last: &Token| {
                    last.has_source_text() && !last.symbol(":") && !last.symbol(";")
                });
            let canonical_depth = if !delimiters.stack.is_empty() {
                let closing_count = std::iter::once(&token)
                    .chain(raw.as_slice())
                    .zip(delimiters.stack.iter().rev())
                    .take_while(|(next, delimiter)| {
                        next.span.line == token.span.line && next.symbol(delimiter.closing())
                    })
                    .count();
                level + delimiters.stack.len() - closing_count
            } else if logical_continuation {
                level + 1
            } else {
                delimiters = Delimiters::default();
                if !tokens.is_empty() {
                    tokens.extend(newline.take());
                }
                let layout_token = |kind| Token {
                    kind,
                    span: Span::new(prefix_line, 1, depth.max(1)),
                    bytes: prefix.start..prefix.start,
                };
                if depth > level {
                    if depth != level + 1 {
                        return Err(Diagnostic::new(
                            Span::new(prefix_line, 1, depth),
                            "Indentation must increase by one tab",
                        ));
                    }
                    levels.push(depth);
                    tokens.push(layout_token(TokenKind::Indent));
                } else {
                    while *levels.last().unwrap() > depth {
                        levels.pop();
                        tokens.push(layout_token(TokenKind::Dedent));
                    }
                    if *levels.last().unwrap() != depth {
                        return Err(Diagnostic::new(
                            Span::new(prefix_line, 1, depth.max(1)),
                            "Dedent does not match an enclosing block",
                        ));
                    }
                }
                depth
            };

            lines.resize_with(token.span.line - 1, || None);
            lines.push(Some(canonical_depth));
            newline = None;
        }

        delimiters.accept(&token);
        tokens.push(token);
    }

    if !tokens.is_empty() && delimiters.stack.is_empty() {
        tokens.extend(newline);
    }
    for _ in 1..levels.len() {
        tokens.push(Token {
            kind: TokenKind::Dedent,
            ..eof.clone()
        });
    }
    tokens.push(eof);
    Ok(Lexed {
        tokens,
        comments,
        line_depths: lines,
    })
}

fn lex_source(source: &str) -> EResult<Lexed> {
    Lexer::new(source).scan()
}

fn lex(source: &str) -> EResult<Vec<Token>> {
    Ok(lex_source(source)?.tokens)
}

// ============================================================
// Values and types
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Int(i64),
    Flt(f64),
    Str(String),
    Bool(bool),
    Null,
    Type(String),
    List(String, Vec<Value>),
}

impl Value {
    fn type_name(&self) -> String {
        match self {
            Value::Int(_) => "int".into(),
            Value::Flt(_) => "flt".into(),
            Value::Str(_) => "str".into(),
            Value::Bool(_) => "bool".into(),
            Value::Null => "none".into(),
            Value::Type(_) => "type".into(),
            Value::List(inner, _) => format!("ls<{}>", inner),
        }
    }

    fn display(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Flt(n) => n.to_string(),

            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            Value::Type(t) => t.clone(),

            Value::List(_, elements) => {
                let elements: Vec<String> = elements.iter().map(Value::display).collect();

                format!("list{{{}}}", elements.join(", "))
            }
        }
    }
}

// Only simple types and one-level lists are public in 0.2.0.
// A structural type model is prepared for a future union-type release.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Type {
    Int,
    Flt,
    Str,
    Bool,
    None,
    Meta,
    List(Box<Type>),
    Union(Vec<Type>),
}

impl Type {
    fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        let members = split_type_union(name)?;

        if members.len() > 1 {
            let mut union = Vec::new();

            for member in members {
                match Self::from_name(member)? {
                    Self::Union(nested) => {
                        for ty in nested {
                            if !union.contains(&ty) {
                                union.push(ty);
                            }
                        }
                    }
                    ty => {
                        if !union.contains(&ty) {
                            union.push(ty);
                        }
                    }
                }
            }

            return match union.len() {
                0 => None,
                1 => union.pop(),
                _ => Some(Self::Union(union)),
            };
        }

        match name {
            "int" => Some(Self::Int),
            "flt" => Some(Self::Flt),
            "str" => Some(Self::Str),
            "bool" => Some(Self::Bool),
            "none" => Some(Self::None),
            "type" => Some(Self::Meta),
            _ => {
                let inner = name.strip_prefix("ls<")?.strip_suffix('>')?;
                let inner_type = Self::from_name(inner)?;

                if inner_type.is_list_element() {
                    Some(Self::List(Box::new(inner_type)))
                } else {
                    None
                }
            }
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Int => "int".into(),
            Self::Flt => "flt".into(),
            Self::Str => "str".into(),
            Self::Bool => "bool".into(),
            Self::None => "none".into(),
            Self::Meta => "type".into(),
            Self::List(inner) => format!("ls<{}>", inner.name()),
            Self::Union(members) => members
                .iter()
                .map(Self::name)
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }

    fn is_list_element(&self) -> bool {
        match self {
            Self::Int | Self::Flt | Self::Str | Self::Bool | Self::None => true,
            Self::Union(members) => members.iter().all(Self::is_list_element),
            Self::Meta | Self::List(_) => false,
        }
    }
}

fn split_type_union(name: &str) -> Option<Vec<&str>> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut members = Vec::new();

    for (index, character) in name.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
            }
            '|' if depth == 0 => {
                members.push(name[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    if depth != 0 {
        return None;
    }

    members.push(name[start..].trim());

    if members.iter().any(|member| member.is_empty()) {
        None
    } else {
        Some(members)
    }
}

fn type_accepts(expected: &str, actual: &str) -> bool {
    let expected = match Type::from_name(expected) {
        Some(ty) => ty,
        None => return false,
    };
    let actual = match Type::from_name(actual) {
        Some(ty) => ty,
        None => return false,
    };

    let expected_members: Vec<&Type> = match &expected {
        Type::Union(members) => members.iter().collect(),
        ty => vec![ty],
    };
    let actual_members: Vec<&Type> = match &actual {
        Type::Union(members) => members.iter().collect(),
        ty => vec![ty],
    };

    actual_members
        .iter()
        .all(|actual| expected_members.contains(actual))
}

fn type_contains(name: &str, target: &Type) -> bool {
    match Type::from_name(name) {
        Some(Type::Union(members)) => members.contains(target),
        Some(ty) => &ty == target,
        None => false,
    }
}

fn list_element_type(name: &str) -> Option<String> {
    let list_type = Type::from_name(name)?;
    let members: Vec<&Type> = match &list_type {
        Type::Union(members) => members.iter().collect(),
        ty => vec![ty],
    };
    let mut element_types = Vec::new();

    for member in members {
        let Type::List(inner) = member else {
            return None;
        };

        match inner.as_ref() {
            Type::Union(nested) => {
                for ty in nested {
                    if !element_types.contains(ty) {
                        element_types.push(ty.clone());
                    }
                }
            }
            ty => {
                if !element_types.contains(ty) {
                    element_types.push(ty.clone());
                }
            }
        }
    }

    match element_types.len() {
        0 => None,
        1 => element_types.pop().map(|ty| ty.name()),
        _ => Some(Type::Union(element_types).name()),
    }
}

fn list_accepts_element(list_type: &str, element_type: &str) -> bool {
    let Some(list_type) = Type::from_name(list_type) else {
        return false;
    };
    let members: Vec<&Type> = match &list_type {
        Type::Union(members) => members.iter().collect(),
        ty => vec![ty],
    };

    !members.is_empty()
        && members.iter().all(|member| match member {
            Type::List(inner) => type_accepts(&inner.name(), element_type),
            _ => false,
        })
}

fn is_list_type(name: &str) -> bool {
    let Some(ty) = Type::from_name(name) else {
        return false;
    };
    match ty {
        Type::List(_) => true,
        Type::Union(members) => members.iter().all(|member| matches!(member, Type::List(_))),
        _ => false,
    }
}

fn valid_list_element_type(name: &str) -> bool {
    Type::from_name(name).is_some_and(|ty| ty.is_list_element())
}

fn valid_type(name: &str) -> bool {
    Type::from_name(name).is_some()
}

fn ascii_letter_byte(text: &str) -> Option<u8> {
    let bytes = text.as_bytes();

    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_lowercase()) {
        Some(bytes[0])
    } else {
        None
    }
}

fn ascii_alphabet_range(from: &str, to: &str) -> Result<String, &'static str> {
    let start = ascii_letter_byte(from)
        .ok_or("AlphabetRange() start must be exactly one ASCII letter (A-Z or a-z)")?;
    let end = ascii_letter_byte(to)
        .ok_or("AlphabetRange() end must be exactly one ASCII letter (A-Z or a-z)")?;

    if start.is_ascii_uppercase() != end.is_ascii_uppercase() {
        return Err("AlphabetRange() endpoints must use the same case");
    }

    let bytes: Box<dyn Iterator<Item = u8>> = if start <= end {
        Box::new(start..=end)
    } else {
        Box::new((end..=start).rev())
    };

    Ok(bytes.map(char::from).collect())
}

fn list_inner(name: &str) -> Option<&str> {
    name.strip_prefix("ls<")?.strip_suffix('>')
}

#[derive(Debug, Clone)]
struct Name {
    text: String,
    span: Span,
}

#[derive(Debug, Clone)]
struct TypeAnn {
    name: String,
    span: Span,
}

fn check_reserved(name: &Name) -> EResult<()> {
    let reserved = [
        "int",
        "flt",
        "str",
        "bool",
        "none",
        "type",
        "ls",
        "list",
        "Length",
        "Output",
        "Input",
        "Includes",
        "Insert",
        "Remove",
        "DeclaredTypeOf",
        "CurrentTypeOf",
        "true",
        "false",
        "null",
        "loop",
        "break",
        "skip",
        "if",
        "else",
        "return",
        "func",
        "function",
        "in",
        "import",
        "as",
    ];

    if reserved.contains(&name.text.as_str()) {
        Err(Diagnostic::new(
            name.span,
            format!(
                "'{}' is a reserved keyword and cannot be used as a name",
                name.text
            ),
        ))
    } else {
        Ok(())
    }
}

// ============================================================
// AST
// ============================================================

#[derive(Debug, Clone)]
struct Parameter {
    name: Name,
    declared_type: TypeAnn,
}

#[derive(Debug, Clone)]
enum FunctionResult<T> {
    Value(T),
    NoValue,
}

const DISCARD_POSITION: &str = "'_' is only allowed on the left of '_ = expression'";
const VALUE_REQUIRED: &str =
    "This call produces no value and cannot be used as an expression value";
const UNUSED_VALUE: &str = "Unused expression value; use '_ = expression' to discard it";
const NOTHING_TO_DISCARD: &str = "This call produces no value to discard";
const NO_VALUE_RETURN: &str = "'return' is forbidden in a function declared with '-|'";

#[derive(Debug, Clone)]
struct FunctionDef {
    name: Name,
    parameters: Vec<Parameter>,
    result: FunctionResult<TypeAnn>,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
enum InterpolatedPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
struct Expr {
    kind: ExprKind,
    span: Span,
}

impl Expr {
    fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone)]
enum ExprKind {
    Literal(Value),
    Interpolate(Vec<InterpolatedPart>),
    ListLiteral(String, Vec<Expr>),
    Variable(String),
    Unary(String, Box<Expr>),
    Binary(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
    ModuleCall {
        module: Name,
        function: Name,
        args: Vec<Expr>,
    },
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
struct Stmt {
    kind: Statement,
    span: Span,
}

#[derive(Debug, Clone)]
enum Statement {
    Import {
        path: String,
        alias: Name,
    },

    Declare {
        name: Name,
        declared_type: TypeAnn,
        value: Expr,
    },

    Assign {
        name: Name,
        value: Expr,
    },

    AssignIndex {
        name: Name,
        index: Expr,
        value: Expr,
    },

    Expression(Expr),
    Discard(Expr),

    Return {
        declared_type: TypeAnn,
        value: Expr,
    },

    Break,
    Skip,

    Function(FunctionDef),

    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },

    While {
        condition: Expr,
        body: Vec<Stmt>,
    },

    CycleIn {
        item_name: Name,
        list_expr: Expr,
        body: Vec<Stmt>,
    },
}

// ============================================================
// Parser
// ============================================================

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    trailing_commas: Vec<usize>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            trailing_commas: Vec::new(),
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn look(&self, offset: usize) -> &TokenKind {
        &self.tokens[(self.pos + offset).min(self.tokens.len() - 1)].kind
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();

        if !matches!(&token.kind, TokenKind::Eof) {
            self.pos += 1;
        }

        token
    }

    fn symbol(&self, value: &str) -> bool {
        self.current().symbol(value)
    }

    fn identifier_is(&self, value: &str) -> bool {
        matches!(
            &self.current().kind,
            TokenKind::Identifier(s) if s == value
        )
    }

    fn skip_statement_separators(&mut self) {
        while matches!(&self.current().kind, TokenKind::Newline) || self.symbol(";") {
            self.advance();
        }
    }

    fn expect_symbol(&mut self, value: &str) -> EResult<Token> {
        if self.symbol(value) {
            Ok(self.advance())
        } else {
            Err(Diagnostic::new(
                self.current().span,
                format!("Expected '{}'", value),
            ))
        }
    }

    fn expect_name(&mut self) -> EResult<Name> {
        let token = self.advance();

        match token.kind {
            TokenKind::Identifier(text) => Ok(Name {
                text,
                span: token.span,
            }),

            TokenKind::Discard => Err(Diagnostic::new(token.span, DISCARD_POSITION)),
            _ => Err(Diagnostic::new(token.span, "Expected an identifier")),
        }
    }

    fn parse_type_atom(&mut self) -> EResult<String> {
        let name = self.expect_name()?;

        if name.text == "ls" {
            let inner = self.parse_element_type()?;
            Ok(format!("ls<{}>", inner))
        } else {
            Ok(name.text)
        }
    }

    fn parse_type_union(&mut self) -> EResult<String> {
        let mut members = vec![self.parse_type_atom()?];

        while self.symbol("|") {
            self.advance();
            members.push(self.parse_type_atom()?);
        }

        let name = members.join(" | ");

        if let Some(ty) = Type::from_name(&name) {
            Ok(ty.name())
        } else {
            Ok(name)
        }
    }

    fn parse_element_type(&mut self) -> EResult<String> {
        self.expect_symbol("<")?;
        let inner_span = self.current().span;
        let inner = self.parse_type_union()?;

        if !valid_list_element_type(&inner) {
            return Err(Diagnostic::new(inner_span, "Invalid list element type"));
        }

        self.expect_symbol(">")?;
        Ok(inner)
    }

    fn parse_type(&mut self) -> EResult<TypeAnn> {
        let span = self.current().span;
        let name = self.parse_type_union()?;

        Ok(TypeAnn { name, span })
    }

    fn parse_program(&mut self) -> EResult<Vec<Stmt>> {
        self.parse_statements(false)
    }

    fn parse_statements(&mut self, in_block: bool) -> EResult<Vec<Stmt>> {
        let mut statements = Vec::new();
        self.skip_statement_separators();
        while self.current().kind != TokenKind::Eof
            && !(in_block && self.current().kind == TokenKind::Dedent)
        {
            if self.current().kind == TokenKind::Indent {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Unexpected indentation",
                ));
            }
            let stmt = self.parse_statement()?;
            let compound = matches!(
                stmt.kind,
                Statement::Function(_)
                    | Statement::If { .. }
                    | Statement::While { .. }
                    | Statement::CycleIn { .. }
            );
            statements.push(stmt);

            if !compound
                && !matches!(
                    self.current().kind,
                    TokenKind::Eof | TokenKind::Newline | TokenKind::Dedent
                )
                && !self.symbol(";")
            {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected a newline or ';'",
                ));
            }
            self.skip_statement_separators();
        }
        Ok(statements)
    }

    fn parse_block(&mut self) -> EResult<Vec<Stmt>> {
        self.expect_symbol(":")?;
        if self.current().kind != TokenKind::Newline {
            let message = if matches!(self.current().kind, TokenKind::Eof | TokenKind::Dedent) {
                "Expected an indented block"
            } else {
                "Expected a newline after ':'"
            };
            return Err(Diagnostic::new(self.current().span, message));
        }
        self.advance();
        if self.current().kind != TokenKind::Indent {
            return Err(Diagnostic::new(
                self.current().span,
                "Expected an indented block",
            ));
        }
        self.advance();

        let statements = self.parse_statements(true)?;
        if statements.is_empty() {
            return Err(Diagnostic::new(
                self.current().span,
                "Expected a statement in the block",
            ));
        }
        if self.current().kind != TokenKind::Dedent {
            return Err(Diagnostic::new(
                self.current().span,
                "Expected the end of the block",
            ));
        }
        self.advance();
        Ok(statements)
    }

    fn parse_function(&mut self) -> EResult<Statement> {
        self.advance(); // function
        let name = self.expect_name()?;
        check_reserved(&name)?;
        self.expect_symbol("(")?;

        let mut parameters: Vec<Parameter> = Vec::new();
        if !self.symbol(")") {
            loop {
                let parameter_type = self.parse_type()?;
                let parameter_name = self.expect_name()?;
                check_reserved(&parameter_name)?;
                if !valid_type(&parameter_type.name)
                    || type_contains(&parameter_type.name, &Type::None)
                    || type_contains(&parameter_type.name, &Type::Meta)
                {
                    return Err(Diagnostic::new(
                        parameter_type.span,
                        "Invalid parameter type",
                    ));
                }
                if self.symbol("=") {
                    return Err(Diagnostic::new(
                        self.current().span,
                        "Default parameter values are not allowed",
                    ));
                }
                if parameters
                    .iter()
                    .any(|p| p.name.text == parameter_name.text)
                {
                    return Err(Diagnostic::new(
                        parameter_name.span,
                        "Duplicate parameter name",
                    ));
                }
                parameters.push(Parameter {
                    name: parameter_name,
                    declared_type: parameter_type,
                });
                if self.symbol(",") {
                    self.advance();
                    if self.symbol(")") {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.expect_symbol(")")?;
        let result = if self.symbol("-|") {
            self.advance();
            FunctionResult::NoValue
        } else {
            if !self.symbol("->") {
                return Err(Diagnostic::new(
                    self.current().span,
                    "Expected '->' or '-|' after function parameters",
                ));
            }
            self.advance();
            let ty = self.parse_type()?;
            if !valid_type(&ty.name) {
                return Err(Diagnostic::new(ty.span, "Invalid return type"));
            }
            FunctionResult::Value(ty)
        };
        let body = self.parse_block()?;
        Ok(Statement::Function(FunctionDef {
            name,
            parameters,
            result,
            body,
        }))
    }

    fn parse_if(&mut self) -> EResult<Statement> {
        self.advance();

        let condition = self.parse_expression(0)?;
        let body = self.parse_block()?;

        let mut branches = vec![(condition, body)];
        let mut else_body = None;

        loop {
            if !self.identifier_is("else") {
                break;
            }

            self.advance();

            if self.identifier_is("if") {
                self.advance();

                let condition = self.parse_expression(0)?;
                let body = self.parse_block()?;

                branches.push((condition, body));
            } else {
                else_body = Some(self.parse_block()?);
                break;
            }
        }

        Ok(Statement::If {
            branches,
            else_body,
        })
    }

    fn parse_loop(&mut self) -> EResult<Statement> {
        self.advance();

        if matches!(&self.current().kind, TokenKind::Identifier(_))
            && matches!(
                self.look(1),
                TokenKind::Identifier(s) if s == "in"
            )
        {
            let item_name = self.expect_name()?;
            check_reserved(&item_name)?;

            self.advance(); // in

            let list_expr = self.parse_expression(0)?;
            let body = self.parse_block()?;

            Ok(Statement::CycleIn {
                item_name,
                list_expr,
                body,
            })
        } else {
            let condition = self.parse_expression(0)?;
            let body = self.parse_block()?;

            Ok(Statement::While { condition, body })
        }
    }

    fn parse_statement(&mut self) -> EResult<Stmt> {
        let span = self.current().span;
        let kind = self.parse_statement_kind()?;

        let span = if matches!(kind, Statement::Expression(_)) {
            span.through(self.tokens[self.pos - 1].span)
        } else {
            span
        };
        Ok(Stmt { kind, span })
    }

    fn parse_statement_kind(&mut self) -> EResult<Statement> {
        if self.current().kind == TokenKind::Discard {
            let marker = self.advance();
            if !self.symbol("=") {
                return Err(Diagnostic::new(marker.span, DISCARD_POSITION));
            }
            self.advance();
            return Ok(Statement::Discard(self.parse_expression(0)?));
        }

        if self.identifier_is("import") {
            self.advance(); // 'import'

            let path_token = self.advance();
            let path = match path_token.kind {
                TokenKind::String(s) => s,
                _ => {
                    return Err(Diagnostic::new(
                        path_token.span,
                        "Expected a string literal for module path",
                    ));
                }
            };

            if !self.identifier_is("as") {
                return Err(Diagnostic::new(
                    self.current().span,
                    "import requires an alias: import \"...\" as name",
                ));
            }
            self.advance(); // 'as'

            let alias = self.expect_name()?;
            check_reserved(&alias)?;

            return Ok(Statement::Import { path, alias });
        }

        if self.identifier_is("function") {
            return self.parse_function();
        }

        if self.identifier_is("if") {
            return self.parse_if();
        }

        if self.identifier_is("loop") {
            return self.parse_loop();
        }

        if self.identifier_is("else") {
            return Err(Diagnostic::new(
                self.current().span,
                "'else' without a preceding 'if' at this indentation",
            ));
        }

        if self.identifier_is("break") {
            self.advance();
            return Ok(Statement::Break);
        }

        if self.identifier_is("skip") {
            self.advance();
            return Ok(Statement::Skip);
        }

        if self.identifier_is("return") {
            let keyword = self.advance();
            if self.current().kind == TokenKind::Discard {
                return Err(Diagnostic::new(self.current().span, DISCARD_POSITION));
            }

            let starts_with_type = matches!(
                &self.current().kind,
                TokenKind::Identifier(s)
                    if matches!(
                        s.as_str(),
                        "int" | "flt" | "str" | "bool" | "none" | "type" | "ls"
                    )
            );

            if !starts_with_type {
                return Err(Diagnostic::new(
                    keyword.span,
                    "Expected 'return T expression' in a value-returning function. Functions declared with '-|' forbid 'return'",
                ));
            }

            let declared_type = self.parse_type()?;
            let value = self.parse_expression(0)?;

            return Ok(Statement::Return {
                declared_type,
                value,
            });
        }

        if let TokenKind::Identifier(first) = &self.current().kind {
            let known_type = matches!(
                first.as_str(),
                "int" | "flt" | "str" | "bool" | "none" | "type" | "ls"
            );
            let saved = self.pos;
            let annotation = self.parse_type();
            let is_declaration = annotation.is_ok()
                && matches!(
                    self.current().kind,
                    TokenKind::Identifier(_) | TokenKind::Discard
                )
                && (known_type || matches!(self.look(1), TokenKind::Symbol(op) if op == ":="));
            if is_declaration {
                let declared_type = annotation?;
                let name = self.expect_name()?;
                check_reserved(&name)?;
                if type_contains(&declared_type.name, &Type::Meta) {
                    return Err(Diagnostic::new(
                        declared_type.span,
                        "Cannot declare a variable of type 'type'",
                    ));
                }
                if !valid_type(&declared_type.name) {
                    return Err(Diagnostic::new(declared_type.span, "Invalid variable type"));
                }
                self.expect_symbol(":=")?;
                return Ok(Statement::Declare {
                    name,
                    declared_type,
                    value: self.parse_expression(0)?,
                });
            }
            self.pos = saved;
        }

        let expr = self.parse_expression(0)?;

        if self.symbol("=") {
            self.advance();

            let value = self.parse_expression(0)?;
            let target_span = expr.span;

            return match expr.kind {
                ExprKind::Variable(text) => Ok(Statement::Assign {
                    name: Name {
                        text,
                        span: target_span,
                    },
                    value,
                }),

                ExprKind::Index(array, index) => {
                    let array_span = array.span;

                    if let ExprKind::Variable(text) = array.kind {
                        Ok(Statement::AssignIndex {
                            name: Name {
                                text,
                                span: array_span,
                            },
                            index: *index,
                            value,
                        })
                    } else {
                        Err(Diagnostic::new(
                            array_span,
                            "Can only assign to a variable's index",
                        ))
                    }
                }

                _ => Err(Diagnostic::new(target_span, "Invalid assignment target")),
            };
        }

        Ok(Statement::Expression(expr))
    }

    fn precedence(op: &str) -> u8 {
        match op {
            "||" => 1,
            "&&" => 2,
            "==" | "!=" | "<" | ">" => 3,
            "+" | "-" => 4,
            "*" | "/" | "%" => 5,
            "^" => 6,
            _ => 0,
        }
    }

    fn parse_expression(&mut self, min_prec: u8) -> EResult<Expr> {
        let mut left = self.parse_prefix()?;

        loop {
            let op = match &self.current().kind {
                TokenKind::Symbol(s)
                    if Self::precedence(s) >= min_prec && Self::precedence(s) > 0 =>
                {
                    s.clone()
                }

                _ => break,
            };

            let op_token = self.advance();
            let prec = Self::precedence(&op);
            let next_min = if op == "^" { prec } else { prec + 1 };

            let right = self.parse_expression(next_min)?;

            left = Expr::new(
                ExprKind::Binary(Box::new(left), op, Box::new(right)),
                op_token.span,
            );
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> EResult<Expr> {
        if self.symbol("!") {
            let token = self.advance();
            let operand = self.parse_prefix()?;

            return Ok(Expr::new(
                ExprKind::Unary("!".into(), Box::new(operand)),
                token.span,
            ));
        }

        let mut expr = self.parse_primary()?;

        while self.symbol("[") {
            let bracket = self.advance();
            let index = self.parse_expression(0)?;

            self.expect_symbol("]")?;

            expr = Expr::new(
                ExprKind::Index(Box::new(expr), Box::new(index)),
                bracket.span,
            );
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> EResult<Expr> {
        let token = self.advance();
        let span = token.span;

        let kind = match token.kind {
            TokenKind::Discard => return Err(Diagnostic::new(span, DISCARD_POSITION)),
            TokenKind::Integer(n) => ExprKind::Literal(Value::Int(n)),
            TokenKind::Float(n) => ExprKind::Literal(Value::Flt(n)),
            TokenKind::String(s) => ExprKind::Literal(Value::Str(s)),

            TokenKind::Interpolated(parts) => {
                let mut result = Vec::new();

                for part in parts {
                    match part {
                        StringPart::Text(text) => {
                            result.push(InterpolatedPart::Text(text));
                        }

                        StringPart::Code(tokens) => {
                            let mut sub = Parser::new(tokens);
                            let expr = sub.parse_expression(0)?;

                            if !matches!(&sub.current().kind, TokenKind::Eof) {
                                return Err(Diagnostic::new(
                                    sub.current().span,
                                    "Unexpected token inside interpolation",
                                ));
                            }

                            result.push(InterpolatedPart::Expr(expr));
                        }
                    }
                }

                ExprKind::Interpolate(result)
            }

            TokenKind::Identifier(name) if name == "list" => {
                let inner = self.parse_element_type()?;
                let opening = self.expect_symbol("{")?;

                let mut elements = Vec::new();

                if !self.symbol("}") {
                    loop {
                        elements.push(self.parse_expression(0)?);

                        if self.symbol(",") {
                            self.advance();

                            if self.symbol("}") {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }

                let last = &self.tokens[self.pos - 1];
                if self.symbol("}")
                    && !elements.is_empty()
                    && !last.symbol(",")
                    && opening.span.line != self.current().span.line
                {
                    self.trailing_commas.push(last.bytes.end);
                }
                self.expect_symbol("}")?;
                ExprKind::ListLiteral(inner, elements)
            }

            TokenKind::Identifier(name)
                if matches!(
                    name.as_str(),
                    "int" | "flt" | "str" | "bool" | "none" | "type"
                ) =>
            {
                ExprKind::Literal(Value::Type(name))
            }

            TokenKind::Identifier(name) if name == "ls" => {
                let inner = self.parse_element_type()?;
                ExprKind::Literal(Value::Type(format!("ls<{}>", inner)))
            }

            TokenKind::Identifier(name) if name == "true" => ExprKind::Literal(Value::Bool(true)),

            TokenKind::Identifier(name) if name == "false" => ExprKind::Literal(Value::Bool(false)),

            TokenKind::Identifier(name) if name == "null" => ExprKind::Literal(Value::Null),

            TokenKind::Identifier(name) => {
                if self.symbol(":")
                    && matches!(self.look(1), TokenKind::Identifier(_) | TokenKind::Discard)
                    && self.tokens[self.pos + 1].bytes.start == self.current().bytes.end
                {
                    let colon_span = self.current().span;

                    if span.line != colon_span.line
                        || span.column + span.length != colon_span.column
                    {
                        return Err(Diagnostic::new(
                            colon_span,
                            "Module calls must not contain spaces around ':'",
                        ));
                    }

                    self.advance(); // ':'
                    let function = self.expect_name()?;

                    if function.span.line != colon_span.line
                        || colon_span.column + 1 != function.span.column
                    {
                        return Err(Diagnostic::new(
                            function.span,
                            "Module calls must not contain spaces around ':'",
                        ));
                    }

                    self.expect_symbol("(")?;

                    let mut args = Vec::new();

                    if !self.symbol(")") {
                        loop {
                            args.push(self.parse_expression(0)?);

                            if self.symbol(",") {
                                self.advance();

                                if self.symbol(")") {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    self.expect_symbol(")")?;
                    ExprKind::ModuleCall {
                        module: Name { text: name, span },
                        function,
                        args,
                    }
                } else if self.symbol("(") {
                    self.advance();

                    let mut args = Vec::new();

                    if !self.symbol(")") {
                        loop {
                            args.push(self.parse_expression(0)?);

                            if self.symbol(",") {
                                self.advance();

                                if self.symbol(")") {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    self.expect_symbol(")")?;
                    ExprKind::Call(name, args)
                } else {
                    ExprKind::Variable(name)
                }
            }

            TokenKind::Symbol(op) if op == "(" => {
                if self.symbol("-") {
                    let minus = self.advance();
                    let operand = self.parse_expression(7)?;

                    self.expect_symbol(")")?;

                    return Ok(Expr::new(
                        ExprKind::Unary("-".into(), Box::new(operand)),
                        minus.span,
                    ));
                }

                let expr = self.parse_expression(0)?;
                self.expect_symbol(")")?;

                return Ok(expr);
            }

            _ => {
                return Err(Diagnostic::new(
                    span,
                    "Expected an expression (unary minus must be wrapped in parens, e.g. (-1))",
                ));
            }
        };

        Ok(Expr::new(kind, span))
    }
}

// ============================================================
// Static checking
// ============================================================

fn stmt_guarantees_return(stmt: &Stmt) -> bool {
    match &stmt.kind {
        Statement::Return { .. } => true,

        Statement::If {
            branches,
            else_body: Some(body),
        } => {
            !branches.is_empty()
                && branches
                    .iter()
                    .all(|(_, body)| block_guarantees_return(body))
                && block_guarantees_return(body)
        }

        _ => false,
    }
}

fn block_guarantees_return(statements: &[Stmt]) -> bool {
    statements.iter().any(stmt_guarantees_return)
}

struct VarInfo {
    ty: Option<String>,
    used: bool,
    kind: &'static str,
    span: Span,
}

struct FuncInfo {
    params: Vec<String>,
    result: FunctionResult<TypeAnn>,
    used: bool,
    span: Span,
}

struct UnresolvedCall {
    caller: Option<String>,
    callee: String,
    span: Span,
}

struct Checker {
    scopes: Vec<HashMap<String, VarInfo>>,
    functions: HashMap<String, FuncInfo>,
    function_order: Vec<String>,
    calls: HashMap<String, Vec<String>>,
    unresolved_calls: Vec<UnresolvedCall>,
    current_function: Option<(String, FunctionResult<TypeAnn>)>,
    errors: Vec<Diagnostic>,
    seen_non_import: bool,
    imported_paths: Vec<String>,
    loop_depth: usize,
}

impl Checker {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            function_order: Vec::new(),
            calls: HashMap::new(),
            unresolved_calls: Vec::new(),
            current_function: None,
            errors: Vec::new(),
            seen_non_import: false,
            imported_paths: Vec::new(),
            loop_depth: 0,
        }
    }

    fn err(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(Diagnostic::new(span, message));
    }

    fn check_program(mut self, program: &[Stmt]) -> Vec<Diagnostic> {
        self.check_block(program);
        self.report_unresolved_calls();

        let module = self.scopes.pop().unwrap();
        self.report_unused(module);

        for name in self.function_order.clone() {
            let info = &self.functions[&name];

            if !info.used {
                let span = info.span;
                self.err(span, format!("Unused function '{}'", name));
            }
        }

        self.errors.sort_by_key(|e| (e.span.line, e.span.column));

        self.errors
    }

    fn report_unresolved_calls(&mut self) {
        for call in std::mem::take(&mut self.unresolved_calls) {
            let name = call.callee.as_str();
            let message = if !self.functions.contains_key(name) {
                format!("Unknown function '{}'", name)
            } else if let Some(cycle) = call.caller.as_deref().and_then(|caller| {
                self.call_path(name, caller)
                    .map(|path| format!("{} -> {}", caller, path.join(" -> ")))
            }) {
                format!(
                    "Recursive function calls are not allowed: {} ('{}' is declared after this call)",
                    cycle, name
                )
            } else {
                format!("Function '{}' must be declared before it is called", name)
            };
            self.err(call.span, message);
        }
    }

    // A legal edge always points to an earlier declaration. Every indirect
    // cycle therefore includes at least one unresolved forward edge. Look for
    // a return path only for those edges, without affecting name resolution.
    fn call_path<'a>(&'a self, from: &'a str, to: &str) -> Option<Vec<&'a str>> {
        let mut pending = VecDeque::from([from]);
        let mut predecessor = HashMap::from([(from, from)]);
        while let Some(function) = pending.pop_front() {
            if function == to {
                let mut path = vec![function];
                while *path.last().unwrap() != from {
                    path.push(predecessor[path.last().unwrap()]);
                }
                path.reverse();
                return Some(path);
            }
            for callee in self
                .calls
                .get(function)
                .into_iter()
                .flatten()
                .map(String::as_str)
            {
                if let std::collections::hash_map::Entry::Vacant(entry) = predecessor.entry(callee)
                {
                    entry.insert(function);
                    pending.push_back(callee);
                }
            }
        }
        None
    }

    fn report_unused(&mut self, scope: HashMap<String, VarInfo>) {
        let mut unused: Vec<_> = scope.into_iter().filter(|(_, info)| !info.used).collect();

        unused.sort_by_key(|(_, info)| (info.span.line, info.span.column));

        for (name, info) in unused {
            let message = match (info.kind, &self.current_function) {
                ("parameter", Some((function, _))) => {
                    format!("Unused parameter '{}' in function '{}'", name, function)
                }

                (kind, _) => format!("Unused {} '{}'", kind, name),
            };

            self.err(info.span, message);
        }
    }

    fn declare(&mut self, name: &Name, ty: Option<String>, kind: &'static str) {
        if self.functions.contains_key(&name.text) {
            self.err(
                name.span,
                format!("'{}' is already declared as a function", name.text),
            );
            return;
        }

        let last = self.scopes.len() - 1;

        if self.scopes[last].contains_key(&name.text) {
            self.err(
                name.span,
                format!("'{}' is already declared in this scope", name.text),
            );
            return;
        }

        if self.scopes[..last]
            .iter()
            .any(|scope| scope.contains_key(&name.text))
        {
            self.err(
                name.span,
                format!("'{}' is already declared in an outer scope", name.text),
            );
            return;
        }

        self.scopes[last].insert(
            name.text.clone(),
            VarInfo {
                ty,
                used: false,
                kind,
                span: name.span,
            },
        );
    }

    fn assignment_target(&mut self, name: &Name) -> Option<(usize, String)> {
        for index in (0..self.scopes.len()).rev() {
            if let Some(info) = self.scopes[index].get(&name.text) {
                if info.kind == "module" {
                    self.err(
                        name.span,
                        format!("'{}' is a module and cannot be assigned", name.text),
                    );
                    return None;
                }
                if index == 0 && self.current_function.is_some() {
                    self.err(
                        name.span,
                        format!(
                            "Cannot assign to global variable '{}' inside a function",
                            name.text
                        ),
                    );
                    return None;
                }
                return info.ty.clone().map(|ty| (index, ty));
            }
        }

        self.err(name.span, format!("Undefined variable '{}'", name.text));
        None
    }

    fn check_scoped_block(&mut self, body: &[Stmt]) {
        self.scopes.push(HashMap::new());
        self.check_block(body);

        let scope = self.scopes.pop().unwrap();
        self.report_unused(scope);
    }

    fn check_block(&mut self, statements: &[Stmt]) {
        let mut unreachable = false;

        for stmt in statements {
            if unreachable {
                if let Some((function, _)) = self.current_function.clone() {
                    self.err(
                        stmt.span,
                        format!(
                            "Unreachable statement after return in function '{}'",
                            function
                        ),
                    );
                }
            }

            self.check_stmt(stmt);

            if stmt_guarantees_return(stmt) {
                unreachable = true;
            }
        }
    }

    fn expect_bool(&mut self, expr: &Expr, what: &str) {
        if let Some(ty) = self.expr_type(expr) {
            if ty != "bool" {
                self.err(
                    expr.span,
                    format!("{} condition must be bool, got {}", what, ty),
                );
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        if let Statement::Import { path, alias } = &stmt.kind {
            if self.seen_non_import {
                self.err(
                    stmt.span,
                    "import statements must appear at the beginning of the file",
                );
            }
            if !matches!(path.as_str(), "strings" | "math" | "types") {
                self.err(
                    stmt.span,
                    format!(
                        "Unknown module '{}' (only strings, math and types are supported)",
                        path
                    ),
                );
                return;
            }
            if self.imported_paths.contains(path) {
                self.err(stmt.span, format!("Module '{}' is already imported", path));
                return;
            }
            self.declare(alias, Some(path.clone()), "module");
            self.imported_paths.push(path.clone());
            return;
        }

        self.seen_non_import = true;

        match &stmt.kind {
            Statement::Import { .. } => unreachable!(),

            Statement::Function(definition) => {
                self.check_function(definition);
            }

            Statement::Declare {
                name,
                declared_type,
                value,
            } => {
                if let Some(ty) = self.expr_type(value) {
                    if !type_accepts(&declared_type.name, &ty) {
                        self.err(
                            value.span,
                            format!(
                                "Variable '{}' expects {}, got {}",
                                name.text, declared_type.name, ty
                            ),
                        );
                    }
                }

                self.declare(name, Some(declared_type.name.clone()), "variable");
            }

            Statement::Assign { name, value } => {
                let value_type = self.expr_type(value);
                let target = self.assignment_target(name);

                if let (Some(value_type), Some((scope, target_type))) = (value_type, target) {
                    if type_accepts(&target_type, &value_type) {
                        self.scopes[scope].get_mut(&name.text).unwrap().used = true;
                    } else {
                        self.err(
                            value.span,
                            format!(
                                "Cannot assign {} to '{}' of type {}",
                                value_type, name.text, target_type
                            ),
                        );
                    }
                }
            }

            Statement::AssignIndex { name, index, value } => {
                let valid_index = self.check_index_expr(index);
                let value_type = self.expr_type(value);
                let target = self.assignment_target(name);

                if let Some((scope, target_type)) = target {
                    if list_element_type(&target_type).is_none() {
                        self.err(name.span, format!("Cannot index into {}", target_type));
                    } else if let Some(value_type) = value_type {
                        if !list_accepts_element(&target_type, &value_type) {
                            self.err(
                                value.span,
                                format!(
                                    "Cannot assign {} to element of {}",
                                    value_type, target_type
                                ),
                            );
                        } else if valid_index {
                            self.scopes[scope].get_mut(&name.text).unwrap().used = true;
                        }
                    }
                }
            }

            Statement::Expression(expr) => {
                if matches!(self.expr_result(expr), Some(FunctionResult::Value(_))) {
                    self.err(stmt.span, UNUSED_VALUE);
                }
            }

            Statement::Discard(expr) => {
                if matches!(self.expr_result(expr), Some(FunctionResult::NoValue)) {
                    self.err(expr.span, NOTHING_TO_DISCARD);
                }
            }

            Statement::Return {
                declared_type,
                value,
            } => {
                let value_type = self.expr_type(value);

                match self.current_function.clone() {
                    None => self.err(stmt.span, "return is only allowed inside a function"),

                    Some((_, FunctionResult::NoValue)) => self.err(stmt.span, NO_VALUE_RETURN),
                    Some((function, FunctionResult::Value(ty))) => {
                        let return_type = ty.name;
                        if !type_accepts(&return_type, &declared_type.name) {
                            self.err(
                                declared_type.span,
                                format!(
                                    "return declares {}, but function '{}' returns {}",
                                    declared_type.name, function, return_type
                                ),
                            );
                        }
                    }
                }

                if let Some(value_type) = value_type {
                    if !type_accepts(&declared_type.name, &value_type) {
                        self.err(
                            value.span,
                            format!(
                                "return declares {}, but the value is {}",
                                declared_type.name, value_type
                            ),
                        );
                    }
                }
            }

            Statement::Break | Statement::Skip => {
                if self.loop_depth == 0 {
                    let keyword = if matches!(&stmt.kind, Statement::Break) {
                        "break"
                    } else {
                        "skip"
                    };
                    self.err(
                        stmt.span,
                        format!("'{}' is only allowed inside a loop", keyword),
                    );
                }
            }

            Statement::If {
                branches,
                else_body,
            } => {
                for (condition, body) in branches {
                    self.expect_bool(condition, "if");
                    self.check_scoped_block(body);
                }

                if let Some(body) = else_body {
                    self.check_scoped_block(body);
                }
            }

            Statement::While { condition, body } => {
                self.expect_bool(condition, "loop");
                self.loop_depth += 1;
                self.check_scoped_block(body);
                self.loop_depth -= 1;
            }

            Statement::CycleIn {
                item_name,
                list_expr,
                body,
            } => {
                let item_type = match self.expr_type(list_expr) {
                    Some(ty) => match list_element_type(&ty) {
                        Some(inner) => Some(inner.to_string()),

                        None => {
                            self.err(
                                list_expr.span,
                                format!("loop .. in .. requires a list, got {}", ty),
                            );
                            None
                        }
                    },

                    None => None,
                };

                // Keep checking the body even if the source
                // expression has a wrong type.
                self.scopes.push(HashMap::new());
                self.declare(item_name, item_type, "loop variable");
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;

                let scope = self.scopes.pop().unwrap();
                self.report_unused(scope);
            }
        }
    }

    fn check_function(&mut self, definition: &FunctionDef) {
        let name = &definition.name;

        if self.current_function.is_some() || self.scopes.len() != 1 {
            self.err(name.span, "Functions can only be declared at the top level");
            return;
        }

        if self.functions.contains_key(&name.text) || self.scopes[0].contains_key(&name.text) {
            self.err(name.span, format!("'{}' is already declared", name.text));
            return;
        }

        self.functions.insert(
            name.text.clone(),
            FuncInfo {
                params: definition
                    .parameters
                    .iter()
                    .map(|p| p.declared_type.name.clone())
                    .collect(),
                result: definition.result.clone(),
                used: false,
                span: name.span,
            },
        );

        self.function_order.push(name.text.clone());

        self.current_function = Some((name.text.clone(), definition.result.clone()));

        let enclosing_loop_depth = self.loop_depth;
        self.loop_depth = 0;

        self.scopes.push(HashMap::new());

        for parameter in &definition.parameters {
            self.declare(
                &parameter.name,
                Some(parameter.declared_type.name.clone()),
                "parameter",
            );
        }

        self.check_block(&definition.body);

        if matches!(definition.result, FunctionResult::Value(_))
            && !block_guarantees_return(&definition.body)
        {
            self.err(
                name.span,
                format!(
                    "Function '{}' does not return a value on every path",
                    name.text
                ),
            );
        }

        let scope = self.scopes.pop().unwrap();
        self.report_unused(scope);
        self.current_function = None;
        self.loop_depth = enclosing_loop_depth;
    }

    fn check_index_expr(&mut self, index: &Expr) -> bool {
        let previous_errors = self.errors.len();
        match &index.kind {
            ExprKind::Literal(Value::Int(n)) if *n <= 0 => {
                self.err(index.span, "Index must be positive and non-zero");
            }
            ExprKind::Unary(op, inner)
                if op == "-" && matches!(&inner.kind, ExprKind::Literal(Value::Int(_))) =>
            {
                self.err(index.span, "Index must be positive and non-zero");
            }
            _ => {}
        }
        let Some(ty) = self.expr_type(index) else {
            return false;
        };
        if !type_accepts("int", &ty) {
            self.err(index.span, format!("Index must be int, got {}", ty));
        }
        self.errors.len() == previous_errors
    }

    /// Type of an operand in a context that requires a value.
    fn expr_type(&mut self, expr: &Expr) -> Option<String> {
        match self.expr_result(expr)? {
            FunctionResult::Value(ty) => Some(ty),
            FunctionResult::NoValue => {
                self.err(expr.span, VALUE_REQUIRED);
                None
            }
        }
    }

    fn expr_result(&mut self, expr: &Expr) -> Option<FunctionResult<String>> {
        // Some operations know their result type even when an operand is invalid.
        // Keep the operand's diagnostic without reporting a second usage error.
        let errors = self.errors.len();
        let unresolved = self.unresolved_calls.len();
        let result = self.infer_result(expr);
        if self.errors.len() == errors && self.unresolved_calls.len() == unresolved {
            result
        } else {
            None
        }
    }

    fn infer_result(&mut self, expr: &Expr) -> Option<FunctionResult<String>> {
        match &expr.kind {
            ExprKind::Literal(value) => Some(value.type_name()),

            ExprKind::Interpolate(parts) => {
                for part in parts {
                    if let InterpolatedPart::Expr(inner) = part {
                        self.expr_type(inner);
                    }
                }

                Some("str".into())
            }

            ExprKind::ListLiteral(inner, elements) => {
                for element in elements {
                    if let Some(ty) = self.expr_type(element) {
                        if !type_accepts(inner, &ty) {
                            self.err(
                                element.span,
                                format!(
                                    "List element type mismatch: expected {}, got {}",
                                    inner, ty
                                ),
                            );
                        }
                    }
                }

                Some(format!("ls<{}>", inner))
            }

            ExprKind::Variable(name) => {
                let mut found = None;

                for scope in self.scopes.iter_mut().rev() {
                    if let Some(info) = scope.get_mut(name) {
                        if info.kind != "module" {
                            info.used = true;
                        }
                        found = Some((info.kind, info.ty.clone()));
                        break;
                    }
                }

                match found {
                    Some(("module", _)) => {
                        self.err(expr.span, format!("'{}' is a module, not a variable", name));
                        None
                    }

                    Some((_, ty)) => ty,

                    None => {
                        self.err(expr.span, format!("Undefined variable '{}'", name));
                        None
                    }
                }
            }

            ExprKind::Unary(op, operand) => {
                let ty = self.expr_type(operand)?;

                match (op.as_str(), ty.as_str()) {
                    ("-", "int" | "flt") => Some(ty),
                    ("!", "bool") => Some("bool".into()),
                    ("-", _) => {
                        self.err(
                            expr.span,
                            format!("Unary '-' requires int or flt, got {}", ty),
                        );
                        None
                    }
                    _ => {
                        self.err(expr.span, format!("Unary '!' requires bool, got {}", ty));
                        None
                    }
                }
            }

            ExprKind::Binary(left, op, right) => {
                let a = self.expr_type(left);
                let b = self.expr_type(right);
                let (a, b) = (a?, b?);
                let numeric = |t: &str| t == "int" || t == "flt";
                let result = match op.as_str() {
                    "+" | "-" | "*" | "%" | "^" if a == b && numeric(&a) => a.as_str(),
                    "/" if a == b && numeric(&a) => "flt",
                    "<" | ">" if a == b && numeric(&a) => "bool",
                    "&&" | "||" if a == "bool" && b == "bool" => "bool",
                    "==" | "!="
                        if a == b
                            && matches!(a.as_str(), "int" | "flt" | "str" | "bool" | "type") =>
                    {
                        "bool"
                    }
                    "==" | "!=" if is_list_type(&a) && a == b => "bool",
                    _ => {
                        self.err(
                            expr.span,
                            format!("Operator '{}' cannot be applied to {} and {}", op, a, b),
                        );
                        return None;
                    }
                };
                Some(result.into())
            }

            ExprKind::Index(array, index) => {
                let array_type = self.expr_type(array);
                self.check_index_expr(index);

                let array_type = array_type?;

                if array_type == "str" {
                    Some("str".into())
                } else {
                    match list_element_type(&array_type) {
                        Some(inner) => Some(inner),

                        None => {
                            self.err(array.span, format!("Cannot index into {}", array_type));
                            None
                        }
                    }
                }
            }

            ExprKind::Call(name, args) => return self.call_type(name, args, expr.span),

            ExprKind::ModuleCall {
                module,
                function,
                args,
            } => {
                let binding = self
                    .scopes
                    .iter_mut()
                    .rev()
                    .find_map(|scope| scope.get_mut(&module.text));
                let path = match binding {
                    Some(info) if info.kind == "module" => {
                        info.used = true;
                        info.ty.clone().unwrap()
                    }
                    other => {
                        let message = if other.is_some() {
                            format!("'{}' is not a module", module.text)
                        } else {
                            format!("Undefined module '{}'", module.text)
                        };
                        self.err(module.span, message);
                        for arg in args {
                            self.expr_type(arg);
                        }
                        return None;
                    }
                };

                match path.as_str() {
                    "strings" => self.check_strings_module(module, function, args, expr.span),
                    "math" => self.check_math_module(module, function, args, expr.span),
                    "types" => self.check_types_module(module, function, args, expr.span),
                    _ => None,
                }
            }
        }
        .map(FunctionResult::Value)
    }

    fn expect_count(&mut self, name: &str, args: &[Expr], count: usize, span: Span) -> bool {
        if args.len() == count {
            return true;
        }

        self.err(
            span,
            format!(
                "{}() expects {} argument(s), got {}",
                name,
                count,
                args.len()
            ),
        );

        for arg in args {
            self.expr_type(arg);
        }

        false
    }

    fn expect_count_mod(
        &mut self,
        full_name: &str,
        args: &[Expr],
        count: usize,
        span: Span,
    ) -> bool {
        if args.len() == count {
            return true;
        }

        self.err(
            span,
            format!(
                "{} expects {} argument(s), got {}",
                full_name,
                count,
                args.len()
            ),
        );

        for arg in args {
            self.expr_type(arg);
        }

        false
    }

    fn check_strings_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let full_name = format!(
            "{}:{}() from module \"strings\"",
            module.text, function.text
        );

        let check_str = |checker: &mut Self, arg: &Expr| {
            if let Some(ty) = checker.expr_type(arg) {
                if ty != "str" {
                    checker.err(
                        arg.span,
                        format!("Argument to {} expects str, got {}", function.text, ty),
                    );
                }
            }
        };

        let check_str_not_empty = |checker: &mut Self, arg: &Expr, msg: &str| {
            if let Some(ty) = checker.expr_type(arg) {
                if ty != "str" {
                    checker.err(
                        arg.span,
                        format!("Argument to {} expects str, got {}", function.text, ty),
                    );
                } else if let ExprKind::Literal(Value::Str(text)) = &arg.kind {
                    if text.is_empty() {
                        checker.err(arg.span, msg);
                    }
                }
            }
        };

        match function.text.as_str() {
            "Trim" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Trim() cannot use an empty string as a pattern",
                    );
                }
                Some("str".into())
            }

            "Replace" => {
                if self.expect_count_mod(&full_name, args, 3, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Replace() cannot use an empty string as a pattern",
                    );
                    check_str(self, &args[2]);
                }
                Some("str".into())
            }

            "Split" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        "Split() cannot use an empty string as a separator",
                    );
                }
                Some("ls<str>".into())
            }

            "StartsWith" | "EndsWith" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str_not_empty(
                        self,
                        &args[1],
                        &format!("{}() cannot search for an empty string", function.text),
                    );
                }
                Some("bool".into())
            }

            "ToLowerCase" | "ToUpperCase" | "Capitalize" => {
                if self.expect_count_mod(&full_name, args, 1, span) {
                    check_str(self, &args[0]);
                }
                Some("str".into())
            }

            "AlphabetRange" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);
                    check_str(self, &args[1]);

                    let start = match &args[0].kind {
                        ExprKind::Literal(Value::Str(text)) => match ascii_letter_byte(text) {
                            Some(value) => Some(value),
                            None => {
                                self.err(
                                    args[0].span,
                                    "AlphabetRange() start must be exactly one ASCII letter (A-Z or a-z)",
                                );
                                None
                            }
                        },
                        _ => None,
                    };

                    let end = match &args[1].kind {
                        ExprKind::Literal(Value::Str(text)) => match ascii_letter_byte(text) {
                            Some(value) => Some(value),
                            None => {
                                self.err(
                                    args[1].span,
                                    "AlphabetRange() end must be exactly one ASCII letter (A-Z or a-z)",
                                );
                                None
                            }
                        },
                        _ => None,
                    };

                    if let (Some(start), Some(end)) = (start, end) {
                        if start.is_ascii_uppercase() != end.is_ascii_uppercase() {
                            self.err(span, "AlphabetRange() endpoints must use the same case");
                        }
                    }
                }
                Some("str".into())
            }

            "IsAsciiUppercase" | "IsAsciiLowercase" => {
                if self.expect_count_mod(&full_name, args, 1, span) {
                    check_str(self, &args[0]);
                }
                Some("bool".into())
            }

            "Repeat" => {
                if self.expect_count_mod(&full_name, args, 2, span) {
                    check_str(self, &args[0]);

                    if let Some(ty) = self.expr_type(&args[1]) {
                        if ty != "int" {
                            self.err(
                                args[1].span,
                                format!("Argument 2 of {} expects int, got {}", function.text, ty),
                            );
                        } else if let ExprKind::Literal(Value::Int(n)) = &args[1].kind {
                            if *n < 0 {
                                self.err(
                                    args[1].span,
                                    "Repeat() count must be a non-negative integer",
                                );
                            }
                        } else if let ExprKind::Unary(op, inner) = &args[1].kind {
                            if op == "-" && matches!(&inner.kind, ExprKind::Literal(Value::Int(_)))
                            {
                                self.err(
                                    args[1].span,
                                    "Repeat() count must be a non-negative integer",
                                );
                            }
                        }
                    }
                }
                Some("str".into())
            }

            _ => {
                self.err(
                    function.span,
                    format!("Unknown function '{}' in module 'strings'", function.text),
                );
                for arg in args {
                    self.expr_type(arg);
                }
                None
            }
        }
    }

    fn check_math_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let name = function.text.as_str();
        let count = match name {
            "AbsoluteValue" | "SquareRoot" | "Floor" | "Ceil" | "Round" | "Sum" | "Average" => 1,
            "Minimum" | "Maximum" | "RandomInterval" => 2,
            "Clamp" => 3,
            _ => {
                self.err(
                    function.span,
                    format!("Unknown function '{}' in module 'math'", name),
                );
                for arg in args {
                    self.expr_type(arg);
                }
                return None;
            }
        };
        let full_name = format!("{}:{}()", module.text, name);
        if !self.expect_count_mod(&full_name, args, count, span) {
            return None;
        }
        let types: Vec<Option<String>> = args.iter().map(|a| self.expr_type(a)).collect();
        let first = types[0].as_deref();
        let numeric = |t: &str| t == "int" || t == "flt";

        match name {
            "RandomInterval" => {
                for (arg, ty) in args.iter().zip(&types) {
                    if let Some(ty) = ty {
                        if ty != "int" {
                            self.err(
                                arg.span,
                                format!("RandomInterval() expects int, got {}", ty),
                            );
                        }
                    }
                }
                if let (ExprKind::Literal(Value::Int(a)), ExprKind::Literal(Value::Int(b))) =
                    (&args[0].kind, &args[1].kind)
                {
                    if a > b {
                        self.err(span, "RandomInterval() minimum must not exceed maximum");
                    }
                }
                Some("int".into())
            }
            "SquareRoot" | "Floor" | "Ceil" | "Round" => {
                if let Some(ty) = first {
                    if ty != "flt" {
                        self.err(args[0].span, format!("{}() expects flt, got {}", name, ty));
                    }
                }
                Some("flt".into())
            }
            "AbsoluteValue" | "Minimum" | "Maximum" | "Clamp" => {
                if let Some(ty) = first {
                    if !numeric(ty) {
                        self.err(
                            args[0].span,
                            format!("{}() expects int or flt, got {}", name, ty),
                        );
                    }
                }
                for (arg, ty) in args.iter().zip(&types).skip(1) {
                    if let Some(ty) = ty {
                        if let Some(first) = first {
                            if ty != first || !numeric(first) {
                                self.err(
                                    arg.span,
                                    format!(
                                        "{}() expects matching int or flt arguments, got {} and {}",
                                        name, first, ty
                                    ),
                                );
                            }
                        } else if !numeric(ty) {
                            self.err(
                                arg.span,
                                format!("{}() expects int or flt, got {}", name, ty),
                            );
                        }
                    }
                }
                first.filter(|t| numeric(t)).map(str::to_string)
            }
            "Sum" | "Average" => {
                if let Some(ty) = first {
                    if ty != "ls<int>" && ty != "ls<flt>" {
                        self.err(
                            args[0].span,
                            format!("{}() expects ls<int> or ls<flt>, got {}", name, ty),
                        );
                    }
                }
                if name == "Average" {
                    Some("flt".into())
                } else {
                    first.and_then(|t| list_inner(t)).map(str::to_string)
                }
            }
            _ => unreachable!(),
        }
    }

    fn check_types_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> Option<String> {
        let name = function.text.as_str();
        if !matches!(
            name,
            "Convert" | "IsConvertible" | "DeclaredTypeOf" | "CurrentTypeOf"
        ) {
            self.err(
                function.span,
                format!("Unknown function '{}' in module 'types'", name),
            );
            for arg in args {
                self.expr_type(arg);
            }
            return None;
        }
        let arity = if matches!(name, "DeclaredTypeOf" | "CurrentTypeOf") {
            1
        } else {
            2
        };
        if !self.expect_count_mod(&format!("{}:{}()", module.text, name), args, arity, span) {
            return None;
        }

        if name == "DeclaredTypeOf" {
            if !matches!(&args[0].kind, ExprKind::Variable(_)) {
                self.err(args[0].span, "DeclaredTypeOf() requires a variable");
                self.expr_type(&args[0]);
                return None;
            }
            self.expr_type(&args[0])?;
            return Some("type".into());
        }

        if name == "CurrentTypeOf" {
            self.expr_type(&args[0])?;
            return Some("type".into());
        }

        let source = self.expr_type(&args[0]);
        self.expr_type(&args[1]);
        let target = match &args[1].kind {
            ExprKind::Literal(Value::Type(target)) if simple_convert_target(target) => target,
            _ => {
                self.err(
                    args[1].span,
                    "The conversion target must be a literal int, flt, str, bool or ls<str>",
                );
                return None;
            }
        };
        if name == "Convert" {
            if let Some(source) = source {
                if !supported_conversion(&source, target) {
                    self.err(
                        args[0].span,
                        format!("Convert() cannot convert {} to {}", source, target),
                    );
                }
            }
            Some(target.clone())
        } else {
            Some("bool".into())
        }
    }

    fn call_type(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Option<FunctionResult<String>> {
        match name {
            "Output" => {
                if self.expect_count(name, args, 1, span) {
                    self.expr_type(&args[0]);
                }

                return Some(FunctionResult::NoValue);
            }

            "Input" => {
                if self.expect_count(name, args, 1, span) {
                    if let Some(ty) = self.expr_type(&args[0]) {
                        if ty != "str" {
                            self.err(
                                args[0].span,
                                format!("Input() requires a str prompt, got {}", ty),
                            );
                        }
                    }
                }

                Some("str".into())
            }

            "Includes" => {
                if self.expect_count(name, args, 2, span) {
                    let haystack = self.expr_type(&args[0]);
                    let needle = self.expr_type(&args[1]);

                    if let (Some(haystack), Some(needle)) = (haystack, needle) {
                        match list_element_type(&haystack) {
                            Some(_) => {
                                if !list_accepts_element(&haystack, &needle) {
                                    self.err(
                                        args[1].span,
                                        format!(
                                            "Includes() cannot search for {} inside {}",
                                            needle, haystack
                                        ),
                                    );
                                }
                            }

                            None if haystack == "str" => {
                                if !type_accepts("str", &needle) {
                                    self.err(
                                        args[1].span,
                                        format!(
                                            "Includes() requires a str to search inside a str, got {}",
                                            needle
                                        ),
                                    );
                                } else if let ExprKind::Literal(Value::Str(pattern)) = &args[1].kind
                                {
                                    // Only substring search forbids the empty pattern.
                                    // Membership in ls<str> must allow an empty string.
                                    if pattern.is_empty() {
                                        self.err(
                                            args[1].span,
                                            "Includes() cannot search for an empty string",
                                        );
                                    }
                                }
                            }

                            None => self.err(
                                args[0].span,
                                format!("Includes() requires a string or list, got {}", haystack),
                            ),
                        }
                    }
                }

                Some("bool".into())
            }

            "Length" => {
                if self.expect_count(name, args, 1, span) {
                    if let Some(ty) = self.expr_type(&args[0]) {
                        if !type_accepts("str", &ty) && list_element_type(&ty).is_none() {
                            self.err(
                                args[0].span,
                                format!("Length() requires a string or list, got {}", ty),
                            );
                        }
                    }
                }

                Some("int".into())
            }

            "Insert" | "Remove" => {
                let count = if name == "Insert" { 3 } else { 2 };

                if !self.expect_count(name, args, count, span) {
                    return None;
                }

                let list_type = self.expr_type(&args[0]);
                self.check_index_expr(&args[1]);

                let value_type = if name == "Insert" {
                    self.expr_type(&args[2])
                } else {
                    None
                };

                let list_type = list_type?;

                if list_element_type(&list_type).is_none() {
                    self.err(
                        args[0].span,
                        format!(
                            "{}() expects a list as first argument, got {}",
                            name, list_type
                        ),
                    );
                    return None;
                }

                if let Some(ty) = value_type {
                    if !list_accepts_element(&list_type, &ty) {
                        self.err(
                            args[2].span,
                            format!("Cannot insert {} into {}", ty, list_type),
                        );
                    }
                }

                Some(list_type)
            }

            _ => {
                let arg_types: Vec<Option<String>> =
                    args.iter().map(|arg| self.expr_type(arg)).collect();

                let caller = self.current_function.as_ref().map(|(name, _)| name.clone());
                if caller.as_deref() == Some(name) {
                    self.err(span, format!("Function '{}' cannot call itself", name));
                    return None;
                }
                if let Some(caller) = &caller {
                    self.calls
                        .entry(caller.clone())
                        .or_default()
                        .push(name.into());
                }

                let Some(info) = self.functions.get_mut(name) else {
                    self.unresolved_calls.push(UnresolvedCall {
                        caller,
                        callee: name.into(),
                        span,
                    });
                    return None;
                };
                info.used = true;
                let params = info.params.clone();
                let result = info.result.clone();

                if args.len() != params.len() {
                    self.err(
                        span,
                        format!(
                            "Function '{}' expects {} argument(s), got {}",
                            name,
                            params.len(),
                            args.len()
                        ),
                    );
                }

                for (index, ((arg, ty), expected)) in args
                    .iter()
                    .zip(arg_types.iter())
                    .zip(params.iter())
                    .enumerate()
                {
                    if let Some(ty) = ty {
                        if !type_accepts(expected, ty) {
                            self.err(
                                arg.span,
                                format!(
                                    "Argument {} of '{}' expects {}, got {}",
                                    index + 1,
                                    name,
                                    expected,
                                    ty
                                ),
                            );
                        }
                    }
                }

                return Some(match result {
                    FunctionResult::Value(ty) => FunctionResult::Value(ty.name),
                    FunctionResult::NoValue => FunctionResult::NoValue,
                });
            }
        }.map(FunctionResult::Value)
    }
}

// ============================================================
// Interpreter
// ============================================================

struct Variable {
    declared_type: String,
    value: Value,
}

enum ExecFlow {
    Continue,
    Return(Value),
    Break,
    Skip,
}

struct Interpreter {
    scopes: Vec<HashMap<String, Variable>>,
    functions: HashMap<String, FunctionDef>,
    return_types: Vec<FunctionResult<TypeAnn>>,
}

fn resolve_index(n: i64, length: usize, is_insert: bool, span: Span) -> EResult<usize> {
    if n <= 0 {
        return Err(Diagnostic::new(span, "Index must be positive and non-zero"));
    }
    let maximum = if is_insert {
        length
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new(span, "List size out of range"))?
    } else {
        length
    };
    let index = usize::try_from(n).map_err(|_| Diagnostic::new(span, "Index out of bounds"))?;
    if index > maximum {
        return Err(Diagnostic::new(
            span,
            format!("Index {} out of bounds (max {})", n, maximum),
        ));
    }
    Ok(index - 1)
}

// Only exact quotients representable by binary64 are permitted for int / int.
// Cancelling common factors yields a dyadic rational when denominator is a power of 2.
fn exact_integer_quotient(a: i64, b: i64) -> Option<f64> {
    if b == 0 {
        return None;
    }
    if a == 0 {
        return Some(0.0);
    }
    let mut numerator = a.unsigned_abs();
    let mut denominator = b.unsigned_abs();
    let mut x = numerator;
    let mut y = denominator;
    while y != 0 {
        let rem = x % y;
        x = y;
        y = rem;
    }
    numerator /= x;
    denominator /= x;
    if !denominator.is_power_of_two() {
        return None;
    }
    let odd = numerator >> numerator.trailing_zeros();
    if (u64::BITS - odd.leading_zeros()) > f64::MANTISSA_DIGITS {
        return None;
    }
    // Use reduced operands: converting unreduced i64 values first can
    // round them even when their quotient is exactly representable.
    let magnitude = (numerator as f64) / (denominator as f64);
    let result = if (a < 0) != (b < 0) {
        -magnitude
    } else {
        magnitude
    };
    result.is_finite().then_some(result)
}

// Type conversion is explicit; only str -> ls<str> is supported for lists in 0.2.0.
fn simple_convert_target(target: &str) -> bool {
    matches!(target, "int" | "flt" | "str" | "bool" | "ls<str>")
}

fn supported_conversion(source: &str, target: &str) -> bool {
    if !simple_convert_target(target) {
        return false;
    }
    match (source, target) {
        ("int" | "flt" | "str" | "bool", t) if t == source => true,
        ("int", "flt" | "str") => true,
        ("flt", "int" | "str") => true,
        ("str", "int" | "flt" | "bool") => true,
        ("bool", "str") => true,
        ("str", "ls<str>") => true,
        _ => false,
    }
}

fn convert_value(value: &Value, target: &str) -> Option<Value> {
    if !supported_conversion(&value.type_name(), target) {
        return None;
    }
    if value.type_name() == target {
        return Some(value.clone());
    }
    match (value, target) {
        (Value::Int(n), "flt") => {
            let float = *n as f64;
            ((float as i128) == (*n as i128)).then_some(Value::Flt(float))
        }
        (Value::Int(n), "str") => Some(Value::Str(n.to_string())),
        (Value::Flt(n), "int")
            if n.is_finite()
                && n.fract() == 0.0
                && *n >= i64::MIN as f64
                && *n < 9_223_372_036_854_775_808.0 =>
        {
            Some(Value::Int(*n as i64))
        }
        (Value::Flt(n), "str") => Some(Value::Str(n.to_string())),
        (Value::Str(s), "int") => s.parse::<i64>().ok().map(Value::Int),
        (Value::Str(s), "flt") => s
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Value::Flt),
        (Value::Str(s), "bool") => match s.as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        (Value::Bool(n), "str") => Some(Value::Str(n.to_string())),
        (Value::Str(text), "ls<str>") => Some(Value::List(
            "str".into(),
            text.chars()
                .map(|character| Value::Str(character.to_string()))
                .collect(),
        )),
        _ => None,
    }
}

// An integer average follows Stage 1 exact int/int division semantics.
// The reduced fraction must be exactly representable in f64.
fn exact_integer_average(sum: i128, count: usize) -> Option<f64> {
    if count == 0 {
        return None;
    }
    if sum == 0 {
        return Some(0.0);
    }
    let mut numerator = sum.unsigned_abs();
    let mut denominator = count as u128;
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    numerator /= a;
    denominator /= a;
    if !denominator.is_power_of_two() {
        return None;
    }
    let odd = numerator >> numerator.trailing_zeros();
    if u128::BITS - odd.leading_zeros() > f64::MANTISSA_DIGITS {
        return None;
    }
    let result = (numerator as f64) / (denominator as f64);
    if !result.is_finite() {
        return None;
    }
    Some(if sum < 0 { -result } else { result })
}

impl Interpreter {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            return_types: Vec::new(),
        }
    }

    fn lookup(&self, name: &str) -> Option<&Variable> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn target_mut(&mut self, name: &Name) -> EResult<&mut Variable> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(&name.text))
            .ok_or_else(|| {
                Diagnostic::new(name.span, format!("Undefined variable '{}'", name.text))
            })?;

        if index == 0 && !self.return_types.is_empty() {
            return Err(Diagnostic::new(
                name.span,
                format!(
                    "Cannot assign to global variable '{}' inside a function",
                    name.text
                ),
            ));
        }

        self.scopes[index].get_mut(&name.text).ok_or_else(|| {
            Diagnostic::new(name.span, format!("Undefined variable '{}'", name.text))
        })
    }

    fn run(&mut self, program: &[Stmt]) -> EResult<()> {
        self.execute_block(program)?;
        Ok(())
    }

    fn run_scoped(&mut self, body: &[Stmt], scope: HashMap<String, Variable>) -> EResult<ExecFlow> {
        self.scopes.push(scope);

        let result = self.execute_block(body);

        self.scopes.pop();
        result
    }

    fn execute_block(&mut self, statements: &[Stmt]) -> EResult<ExecFlow> {
        for stmt in statements {
            let flow = self.execute_stmt(stmt)?;
            if !matches!(flow, ExecFlow::Continue) {
                return Ok(flow);
            }
        }

        Ok(ExecFlow::Continue)
    }

    fn execute_stmt(&mut self, stmt: &Stmt) -> EResult<ExecFlow> {
        match &stmt.kind {
            Statement::Import { path, alias } => {
                let scope = self.scopes.last_mut().unwrap();
                scope.insert(
                    alias.text.clone(),
                    Variable {
                        declared_type: "module".into(),
                        value: Value::Type(path.clone()),
                    },
                );
            }

            Statement::Function(definition) => {
                if self.scopes.len() != 1 || !self.return_types.is_empty() {
                    return Err(Diagnostic::new(
                        definition.name.span,
                        "Functions can only be declared at the top level",
                    ));
                }

                if self.functions.contains_key(&definition.name.text)
                    || self.lookup(&definition.name.text).is_some()
                {
                    return Err(Diagnostic::new(
                        definition.name.span,
                        format!("'{}' is already declared", definition.name.text),
                    ));
                }

                self.functions
                    .insert(definition.name.text.clone(), definition.clone());
            }

            Statement::Declare {
                name,
                declared_type,
                value,
            } => {
                let evaluated = self.evaluate(value)?;

                if !type_accepts(&declared_type.name, &evaluated.type_name()) {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "Variable '{}' expects {}, got {}",
                            name.text,
                            declared_type.name,
                            evaluated.type_name()
                        ),
                    ));
                }

                let scope = self.scopes.last_mut().unwrap();

                if scope.contains_key(&name.text) {
                    return Err(Diagnostic::new(
                        name.span,
                        format!("'{}' is already declared in this scope", name.text),
                    ));
                }

                scope.insert(
                    name.text.clone(),
                    Variable {
                        declared_type: declared_type.name.clone(),
                        value: evaluated,
                    },
                );
            }

            Statement::Assign { name, value } => {
                let new_value = self.evaluate(value)?;
                let variable = self.target_mut(name)?;

                if !type_accepts(&variable.declared_type, &new_value.type_name()) {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "Cannot assign {} to '{}' of type {}",
                            new_value.type_name(),
                            name.text,
                            variable.declared_type
                        ),
                    ));
                }

                variable.value = new_value;
            }

            Statement::AssignIndex { name, index, value } => {
                let index_value = self.evaluate(index)?;
                let new_value = self.evaluate(value)?;
                let variable = self.target_mut(name)?;

                match &mut variable.value {
                    Value::List(inner, elements) => {
                        if !type_accepts(inner, &new_value.type_name()) {
                            return Err(Diagnostic::new(
                                value.span,
                                format!(
                                    "Cannot assign {} to element of ls<{}>",
                                    new_value.type_name(),
                                    inner
                                ),
                            ));
                        }

                        let n = match index_value {
                            Value::Int(n) => n,

                            other => {
                                return Err(Diagnostic::new(
                                    index.span,
                                    format!("Index must be int, got {}", other.type_name()),
                                ));
                            }
                        };

                        let resolved = resolve_index(n, elements.len(), false, index.span)?;

                        elements[resolved] = new_value;
                    }

                    other => {
                        return Err(Diagnostic::new(
                            name.span,
                            format!("Cannot index into {}", other.type_name()),
                        ));
                    }
                }
            }

            Statement::Expression(expr) => {
                if matches!(self.evaluate_result(expr)?, FunctionResult::Value(_)) {
                    return Err(Diagnostic::new(stmt.span, UNUSED_VALUE));
                }
            }
            Statement::Discard(expr) => {
                if matches!(self.evaluate_result(expr)?, FunctionResult::NoValue) {
                    return Err(Diagnostic::new(expr.span, NOTHING_TO_DISCARD));
                }
            }

            Statement::Return {
                declared_type,
                value,
            } => {
                let expected = self.return_types.last().cloned().ok_or_else(|| {
                    Diagnostic::new(stmt.span, "return is only allowed inside a function")
                })?;

                let FunctionResult::Value(annotation) = expected else {
                    return Err(Diagnostic::new(stmt.span, NO_VALUE_RETURN));
                };
                let expected = annotation.name;
                if !type_accepts(&expected, &declared_type.name) {
                    return Err(Diagnostic::new(
                        declared_type.span,
                        format!(
                            "return declares {}, expected {}",
                            declared_type.name, expected
                        ),
                    ));
                }

                let result = self.evaluate(value)?;

                if !type_accepts(&declared_type.name, &result.type_name()) {
                    return Err(Diagnostic::new(
                        value.span,
                        format!(
                            "return declares {}, but the value is {}",
                            declared_type.name,
                            result.type_name()
                        ),
                    ));
                }

                return Ok(ExecFlow::Return(result));
            }

            Statement::Break => return Ok(ExecFlow::Break),

            Statement::Skip => return Ok(ExecFlow::Skip),

            Statement::If {
                branches,
                else_body,
            } => {
                for (condition, body) in branches {
                    let matched = match self.evaluate(condition)? {
                        Value::Bool(value) => value,

                        other => {
                            return Err(Diagnostic::new(
                                condition.span,
                                format!("if condition must be bool, got {}", other.type_name()),
                            ));
                        }
                    };

                    if matched {
                        return self.run_scoped(body, HashMap::new());
                    }
                }

                if let Some(body) = else_body {
                    return self.run_scoped(body, HashMap::new());
                }
            }

            Statement::While { condition, body } => loop {
                let keep_going = match self.evaluate(condition)? {
                    Value::Bool(value) => value,

                    other => {
                        return Err(Diagnostic::new(
                            condition.span,
                            format!("loop condition must be bool, got {}", other.type_name()),
                        ));
                    }
                };

                if !keep_going {
                    break;
                }

                match self.run_scoped(body, HashMap::new())? {
                    ExecFlow::Continue => {}
                    ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                    ExecFlow::Break => break,
                    ExecFlow::Skip => continue,
                }
            },

            Statement::CycleIn {
                item_name,
                list_expr,
                body,
            } => {
                let (inner, elements) = match self.evaluate(list_expr)? {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            list_expr.span,
                            format!("loop .. in .. requires a list, got {}", other.type_name()),
                        ));
                    }
                };

                // elements is a separate snapshot of the list.
                for element in elements {
                    let mut scope = HashMap::new();

                    scope.insert(
                        item_name.text.clone(),
                        Variable {
                            declared_type: inner.clone(),
                            value: element,
                        },
                    );

                    match self.run_scoped(body, scope)? {
                        ExecFlow::Continue => {}
                        ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                        ExecFlow::Break => break,
                        ExecFlow::Skip => continue,
                    }
                }
            }
        }

        Ok(ExecFlow::Continue)
    }

    fn evaluate(&mut self, expr: &Expr) -> EResult<Value> {
        match self.evaluate_result(expr)? {
            FunctionResult::Value(value) => Ok(value),
            FunctionResult::NoValue => Err(Diagnostic::new(expr.span, VALUE_REQUIRED)),
        }
    }

    fn evaluate_result(&mut self, expr: &Expr) -> EResult<FunctionResult<Value>> {
        match &expr.kind {
            ExprKind::Literal(value) => Ok(value.clone()),

            ExprKind::Interpolate(parts) => {
                let mut result = String::new();

                for part in parts {
                    match part {
                        InterpolatedPart::Text(text) => {
                            result.push_str(text);
                        }

                        InterpolatedPart::Expr(inner) => {
                            result.push_str(&self.evaluate(inner)?.display());
                        }
                    }
                }

                Ok(Value::Str(result))
            }

            ExprKind::ListLiteral(inner, elements) => {
                let mut values = Vec::new();

                for element in elements {
                    let value = self.evaluate(element)?;

                    if !type_accepts(inner, &value.type_name()) {
                        return Err(Diagnostic::new(
                            element.span,
                            format!(
                                "List element type mismatch: expected {}, got {}",
                                inner,
                                value.type_name()
                            ),
                        ));
                    }

                    values.push(value);
                }

                Ok(Value::List(inner.clone(), values))
            }

            ExprKind::Variable(name) => match self.lookup(name) {
                Some(variable) if variable.declared_type == "module" => Err(Diagnostic::new(
                    expr.span,
                    format!("'{}' is a module, not a variable", name),
                )),

                Some(variable) => Ok(variable.value.clone()),

                None => Err(Diagnostic::new(
                    expr.span,
                    format!("Undefined variable '{}'", name),
                )),
            },

            ExprKind::Index(array, index) => {
                let array_value = self.evaluate(array)?;
                let index_value = self.evaluate(index)?;

                let n = match index_value {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            index.span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                match array_value {
                    Value::List(_, elements) => {
                        let index = resolve_index(n, elements.len(), false, index.span)?;
                        Ok(elements[index].clone())
                    }

                    Value::Str(text) => {
                        let length = text.chars().count();
                        let index = resolve_index(n, length, false, index.span)?;
                        let character = text
                            .chars()
                            .nth(index)
                            .expect("Resolved string index must be in bounds");
                        Ok(Value::Str(character.to_string()))
                    }

                    other => Err(Diagnostic::new(
                        array.span,
                        format!("Cannot index into {}", other.type_name()),
                    )),
                }
            }

            ExprKind::Unary(op, operand) => {
                let value = self.evaluate(operand)?;

                match (op.as_str(), value) {
                    ("-", Value::Int(n)) => n.checked_neg().map(Value::Int).ok_or_else(|| {
                        Diagnostic::new(expr.span, "Integer overflow in unary minus")
                    }),
                    ("-", Value::Flt(n)) => Ok(Value::Flt(-n)),
                    ("!", Value::Bool(value)) => Ok(Value::Bool(!value)),
                    ("-", other) => Err(Diagnostic::new(
                        expr.span,
                        format!("Unary '-' requires int or flt, got {}", other.type_name()),
                    )),
                    (_, other) => Err(Diagnostic::new(
                        expr.span,
                        format!("Unary '!' requires bool, got {}", other.type_name()),
                    )),
                }
            }

            ExprKind::Binary(left, op, right) => {
                let a = self.evaluate(left)?;
                if op == "&&" || op == "||" {
                    match &a {
                        Value::Bool(false) if op == "&&" => return Ok(FunctionResult::Value(Value::Bool(false))),
                        Value::Bool(true) if op == "||" => return Ok(FunctionResult::Value(Value::Bool(true))),
                        Value::Bool(_) => {}
                        other => {
                            return Err(Diagnostic::new(
                                expr.span,
                                format!(
                                    "Operator '{}' requires bool, got {}",
                                    op,
                                    other.type_name()
                                ),
                            ));
                        }
                    }
                }
                let b = self.evaluate(right)?;
                let result = match (a, op.as_str(), b) {
                    (Value::Int(a), op @ ("+" | "-" | "*" | "%" | "^"), Value::Int(b)) => {
                        let checked = match op {
                            "+" => a.checked_add(b),
                            "-" => a.checked_sub(b),
                            "*" => a.checked_mul(b),
                            "%" => a.checked_rem(b),
                            "^" => {
                                if b < 0 {
                                    None
                                } else {
                                    u32::try_from(b)
                                        .ok()
                                        .and_then(|exponent| a.checked_pow(exponent))
                                }
                            }
                            _ => unreachable!(),
                        };
                        Value::Int(checked.ok_or_else(|| Diagnostic::new(
                            expr.span, "Integer arithmetic out of range (overflow, zero divisor or exponent)"
                        ))?)
                    }
                    (Value::Int(a), "/", Value::Int(b)) => {
                        if b == 0 {
                            return Err(Diagnostic::new(expr.span, "Division by zero"));
                        }
                        Value::Flt(exact_integer_quotient(a, b).ok_or_else(|| {
                            Diagnostic::new(
                                expr.span,
                                "Integer division would lose floating-point precision",
                            )
                        })?)
                    }
                    (Value::Flt(a), "+", Value::Flt(b)) => Value::Flt(a + b),
                    (Value::Flt(a), "-", Value::Flt(b)) => Value::Flt(a - b),
                    (Value::Flt(a), "*", Value::Flt(b)) => Value::Flt(a * b),
                    (Value::Flt(_), "/", Value::Flt(0.0)) => {
                        return Err(Diagnostic::new(expr.span, "Division by zero"));
                    }
                    (Value::Flt(_), "%", Value::Flt(0.0)) => {
                        return Err(Diagnostic::new(expr.span, "Modulo by zero"));
                    }
                    (Value::Flt(a), "/", Value::Flt(b)) => Value::Flt(a / b),
                    (Value::Flt(a), "%", Value::Flt(b)) => Value::Flt(a % b),
                    (Value::Flt(a), "^", Value::Flt(b)) => Value::Flt(a.powf(b)),
                    (Value::Int(a), "==", Value::Int(b)) => Value::Bool(a == b),
                    (Value::Int(a), "!=", Value::Int(b)) => Value::Bool(a != b),
                    (Value::Int(a), "<", Value::Int(b)) => Value::Bool(a < b),
                    (Value::Int(a), ">", Value::Int(b)) => Value::Bool(a > b),
                    (Value::Flt(a), "==", Value::Flt(b)) => Value::Bool(a == b),
                    (Value::Flt(a), "!=", Value::Flt(b)) => Value::Bool(a != b),
                    (Value::Flt(a), "<", Value::Flt(b)) => Value::Bool(a < b),
                    (Value::Flt(a), ">", Value::Flt(b)) => Value::Bool(a > b),
                    (Value::Str(a), "==", Value::Str(b)) => Value::Bool(a == b),
                    (Value::Str(a), "!=", Value::Str(b)) => Value::Bool(a != b),
                    (Value::Bool(a), "==", Value::Bool(b)) => Value::Bool(a == b),
                    (Value::Bool(a), "!=", Value::Bool(b)) => Value::Bool(a != b),
                    (Value::Bool(a), "&&", Value::Bool(b)) => Value::Bool(a && b),
                    (Value::Bool(a), "||", Value::Bool(b)) => Value::Bool(a || b),
                    (Value::Type(a), "==", Value::Type(b)) => Value::Bool(a == b),
                    (Value::Type(a), "!=", Value::Type(b)) => Value::Bool(a != b),
                    (Value::List(ta, a), "==", Value::List(tb, b)) => {
                        Value::Bool(ta == tb && a == b)
                    }
                    (Value::List(ta, a), "!=", Value::List(tb, b)) => {
                        Value::Bool(ta != tb || a != b)
                    }
                    (a, op, b) => {
                        return Err(Diagnostic::new(
                            expr.span,
                            format!(
                                "Operator '{}' cannot be applied to {} and {}",
                                op,
                                a.type_name(),
                                b.type_name()
                            ),
                        ));
                    }
                };
                if let Value::Flt(n) = &result {
                    if !n.is_finite() {
                        return Err(Diagnostic::new(
                            expr.span,
                            "Floating-point result out of range",
                        ));
                    }
                }
                Ok(result)
            }

            ExprKind::Call(name, args) => return self.call(name, args, expr.span),

            ExprKind::ModuleCall {
                module,
                function,
                args,
            } => {
                let path = match self.lookup(&module.text) {
                    Some(var) if var.declared_type == "module" => {
                        if let Value::Type(p) = &var.value {
                            p.clone()
                        } else {
                            unreachable!()
                        }
                    }
                    Some(_) => {
                        return Err(Diagnostic::new(
                            module.span,
                            format!("'{}' is not a module", module.text),
                        ));
                    }
                    None => {
                        return Err(Diagnostic::new(
                            module.span,
                            format!("Undefined module '{}'", module.text),
                        ));
                    }
                };

                match path.as_str() {
                    "strings" => self.call_strings_module(module, function, args, expr.span),
                    "math" => self.call_math_module(module, function, args, expr.span),
                    "types" => self.call_types_module(module, function, args, expr.span),
                    _ => Err(Diagnostic::new(
                        expr.span,
                        format!("Execution of module '{}' is not supported", path),
                    )),
                }
            }
        }.map(FunctionResult::Value)
    }

    fn check_count(name: &str, actual: usize, expected: usize, span: Span) -> EResult<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(Diagnostic::new(
                span,
                format!(
                    "{}() expects {} argument(s), got {}",
                    name, expected, actual
                ),
            ))
        }
    }

    fn call_strings_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let mut vals = Vec::new();

        for arg in args {
            vals.push((self.evaluate(arg)?, arg.span));
        }

        let expected = match function.text.as_str() {
            "Trim" | "Split" | "StartsWith" | "EndsWith" | "Repeat" | "AlphabetRange" => 2,
            "Replace" => 3,
            "ToLowerCase" | "ToUpperCase" | "Capitalize" | "IsAsciiUppercase"
            | "IsAsciiLowercase" => 1,
            _ => {
                return Err(Diagnostic::new(
                    function.span,
                    format!("Unknown function '{}' in module 'strings'", function.text),
                ));
            }
        };

        if vals.len() != expected {
            return Err(Diagnostic::new(
                span,
                format!(
                    "{}:{}() from module \"strings\" expects {} argument(s), got {}",
                    module.text,
                    function.text,
                    expected,
                    vals.len()
                ),
            ));
        }

        let as_str = |val: &Value, span: Span| -> EResult<String> {
            if let Value::Str(s) = val {
                Ok(s.clone())
            } else {
                Err(Diagnostic::new(
                    span,
                    format!("Expected str, got {}", val.type_name()),
                ))
            }
        };

        let as_int = |val: &Value, span: Span| -> EResult<i64> {
            if let Value::Int(n) = val {
                Ok(*n)
            } else {
                Err(Diagnostic::new(
                    span,
                    format!("Expected int, got {}", val.type_name()),
                ))
            }
        };

        match function.text.as_str() {
            "Trim" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let old = as_str(&vals[1].0, vals[1].1)?;

                if old.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Trim() cannot use an empty string as a pattern",
                    ));
                }

                let mut res = text.as_str();

                loop {
                    let mut changed = false;

                    if let Some(stripped) = res.strip_prefix(old.as_str()) {
                        res = stripped;
                        changed = true;
                    }

                    if let Some(stripped) = res.strip_suffix(old.as_str()) {
                        res = stripped;
                        changed = true;
                    }

                    if !changed {
                        break;
                    }
                }

                Ok(Value::Str(res.to_string()))
            }

            "Replace" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let old = as_str(&vals[1].0, vals[1].1)?;
                let new_val = as_str(&vals[2].0, vals[2].1)?;

                if old.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Replace() cannot use an empty string as a pattern",
                    ));
                }

                Ok(Value::Str(text.replace(old.as_str(), &new_val)))
            }

            "Split" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let sep = as_str(&vals[1].0, vals[1].1)?;

                if sep.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Split() cannot use an empty string as a separator",
                    ));
                }

                let parts = text
                    .split(sep.as_str())
                    .map(|s| Value::Str(s.to_string()))
                    .collect();

                Ok(Value::List("str".to_string(), parts))
            }

            "StartsWith" | "EndsWith" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let part = as_str(&vals[1].0, vals[1].1)?;

                if part.is_empty() {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        format!("{}() cannot search for an empty string", function.text),
                    ));
                }

                if function.text == "StartsWith" {
                    Ok(Value::Bool(text.starts_with(part.as_str())))
                } else {
                    Ok(Value::Bool(text.ends_with(part.as_str())))
                }
            }

            "AlphabetRange" => {
                let from = as_str(&vals[0].0, vals[0].1)?;
                let to = as_str(&vals[1].0, vals[1].1)?;

                match ascii_alphabet_range(&from, &to) {
                    Ok(result) => Ok(Value::Str(result)),
                    Err(message) => Err(Diagnostic::new(span, message)),
                }
            }

            "IsAsciiUppercase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Bool(
                    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_uppercase()),
                ))
            }

            "IsAsciiLowercase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Bool(
                    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_lowercase()),
                ))
            }

            "ToLowerCase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Str(text.to_lowercase()))
            }

            "ToUpperCase" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                Ok(Value::Str(text.to_uppercase()))
            }

            "Capitalize" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let mut result = String::new();
                let mut done = false;

                for ch in text.chars() {
                    if !done && ch.is_alphabetic() {
                        result.extend(ch.to_uppercase());
                        done = true;
                    } else {
                        result.push(ch);
                    }
                }

                Ok(Value::Str(result))
            }

            "Repeat" => {
                let text = as_str(&vals[0].0, vals[0].1)?;
                let count = as_int(&vals[1].0, vals[1].1)?;

                if count < 0 {
                    return Err(Diagnostic::new(
                        vals[1].1,
                        "Repeat() count must be a non-negative integer",
                    ));
                }

                let max_len: usize = 64 * 1024 * 1024;

                if count > max_len as i64 {
                    return Err(Diagnostic::new(
                        span,
                        "Resulting string is too large (exceeds 64 MiB limit)",
                    ));
                }

                let n = count as usize;

                match text.len().checked_mul(n) {
                    Some(total) if total <= max_len => Ok(Value::Str(text.repeat(n))),

                    _ => Err(Diagnostic::new(
                        span,
                        "Resulting string is too large (exceeds 64 MiB limit)",
                    )),
                }
            }

            _ => Err(Diagnostic::new(
                function.span,
                format!("Unknown function '{}' in module 'strings'", function.text),
            )),
        }
    }

    fn call_math_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let name = function.text.as_str();
        let count = match name {
            "AbsoluteValue" | "SquareRoot" | "Floor" | "Ceil" | "Round" | "Sum" | "Average" => 1,
            "Minimum" | "Maximum" | "RandomInterval" => 2,
            "Clamp" => 3,
            _ => {
                return Err(Diagnostic::new(
                    function.span,
                    format!("Unknown function '{}' in module 'math'", name),
                ));
            }
        };
        Self::check_count(
            &format!("{}:{}", module.text, name),
            args.len(),
            count,
            span,
        )?;
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.evaluate(arg)?);
        }
        let mismatch = || {
            Diagnostic::new(
                span,
                format!("{}() arguments have invalid or incompatible types", name),
            )
        };
        let out_of_range =
            || Diagnostic::new(span, format!("{}() result out of range or invalid", name));

        match (name, values.as_slice()) {
            ("RandomInterval", [Value::Int(a), Value::Int(b)]) => {
                if a > b {
                    return Err(Diagnostic::new(
                        span,
                        "RandomInterval() minimum must not exceed maximum",
                    ));
                }
                use rand::Rng;
                Ok(Value::Int(rand::rng().random_range(*a..=*b)))
            }
            ("AbsoluteValue", [Value::Int(n)]) => {
                Ok(Value::Int(n.checked_abs().ok_or_else(out_of_range)?))
            }
            ("AbsoluteValue", [Value::Flt(n)]) => Ok(Value::Flt(n.abs())),
            ("SquareRoot", [Value::Flt(n)]) => {
                if *n < 0.0 {
                    return Err(Diagnostic::new(
                        span,
                        "SquareRoot() requires a non-negative value",
                    ));
                }
                Ok(Value::Flt(n.sqrt()))
            }
            ("Floor", [Value::Flt(n)]) => Ok(Value::Flt(n.floor())),
            ("Ceil", [Value::Flt(n)]) => Ok(Value::Flt(n.ceil())),
            ("Round", [Value::Flt(n)]) => Ok(Value::Flt(n.round())),
            ("Minimum", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).min(*b))),
            ("Maximum", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int((*a).max(*b))),
            ("Minimum", [Value::Flt(a), Value::Flt(b)]) => Ok(Value::Flt(a.min(*b))),
            ("Maximum", [Value::Flt(a), Value::Flt(b)]) => Ok(Value::Flt(a.max(*b))),
            ("Clamp", [Value::Int(n), Value::Int(lo), Value::Int(hi)]) => {
                if lo > hi {
                    return Err(Diagnostic::new(
                        span,
                        "Clamp() minimum must not exceed maximum",
                    ));
                }
                Ok(Value::Int((*n).clamp(*lo, *hi)))
            }
            ("Clamp", [Value::Flt(n), Value::Flt(lo), Value::Flt(hi)]) => {
                if lo > hi {
                    return Err(Diagnostic::new(
                        span,
                        "Clamp() minimum must not exceed maximum",
                    ));
                }
                Ok(Value::Flt((*n).max(*lo).min(*hi)))
            }
            ("Sum", [Value::List(inner, items)]) if inner == "int" => {
                let mut sum = 0_i64;
                for value in items {
                    if let Value::Int(n) = value {
                        sum = sum.checked_add(*n).ok_or_else(out_of_range)?;
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Int(sum))
            }
            ("Sum", [Value::List(inner, items)]) if inner == "flt" => {
                let mut sum = 0.0_f64;
                for value in items {
                    if let Value::Flt(n) = value {
                        sum += *n;
                        if !sum.is_finite() {
                            return Err(out_of_range());
                        }
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Flt(sum))
            }
            ("Average", [Value::List(inner, items)]) if inner == "int" => {
                let mut sum = 0_i128;
                for value in items {
                    if let Value::Int(n) = value {
                        sum = sum.checked_add(*n as i128).ok_or_else(out_of_range)?;
                    } else {
                        return Err(mismatch());
                    }
                }
                if items.is_empty() {
                    return Err(Diagnostic::new(span, "Average() requires a non-empty list"));
                }
                let result = exact_integer_average(sum, items.len()).ok_or_else(|| {
                    Diagnostic::new(
                        span,
                        "Average() of integers would lose floating-point precision",
                    )
                })?;
                Ok(Value::Flt(result))
            }
            ("Average", [Value::List(inner, items)]) if inner == "flt" => {
                if items.is_empty() {
                    return Err(Diagnostic::new(span, "Average() requires a non-empty list"));
                }
                // Scale each term before adding to avoid intermediate sum overflow.
                let divisor = items.len() as f64;
                let mut average = 0.0_f64;
                for value in items {
                    if let Value::Flt(n) = value {
                        average += *n / divisor;
                        if !average.is_finite() {
                            return Err(out_of_range());
                        }
                    } else {
                        return Err(mismatch());
                    }
                }
                Ok(Value::Flt(average))
            }
            _ => Err(mismatch()),
        }
    }

    fn call_types_module(
        &mut self,
        module: &Name,
        function: &Name,
        args: &[Expr],
        span: Span,
    ) -> EResult<Value> {
        let name = function.text.as_str();
        if !matches!(
            name,
            "Convert" | "IsConvertible" | "DeclaredTypeOf" | "CurrentTypeOf"
        ) {
            return Err(Diagnostic::new(
                function.span,
                format!("Unknown function '{}' in module 'types'", name),
            ));
        }

        let arity = if matches!(name, "DeclaredTypeOf" | "CurrentTypeOf") {
            1
        } else {
            2
        };
        Self::check_count(
            &format!("{}:{}", module.text, name),
            args.len(),
            arity,
            span,
        )?;

        if name == "DeclaredTypeOf" {
            let variable_name = match &args[0].kind {
                ExprKind::Variable(variable_name) => variable_name,
                _ => {
                    return Err(Diagnostic::new(
                        args[0].span,
                        "DeclaredTypeOf() requires a variable",
                    ));
                }
            };
            let variable = self.lookup(variable_name).ok_or_else(|| {
                Diagnostic::new(
                    args[0].span,
                    format!("Undefined variable '{}'", variable_name),
                )
            })?;
            return Ok(Value::Type(variable.declared_type.clone()));
        }

        if name == "CurrentTypeOf" {
            let value = self.evaluate(&args[0])?;
            return Ok(Value::Type(value.type_name()));
        }

        let source = self.evaluate(&args[0])?;
        let target = match &args[1].kind {
            ExprKind::Literal(Value::Type(target)) if simple_convert_target(target) => target,
            _ => {
                return Err(Diagnostic::new(
                    args[1].span,
                    "The conversion target must be a literal int, flt, str, bool or ls<str>",
                ));
            }
        };
        let converted = convert_value(&source, target);
        if name == "IsConvertible" {
            Ok(Value::Bool(converted.is_some()))
        } else {
            converted.ok_or_else(|| {
                Diagnostic::new(
                    span,
                    format!(
                        "Convert() cannot convert {} value to {}",
                        source.type_name(),
                        target
                    ),
                )
            })
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], span: Span) -> EResult<FunctionResult<Value>> {
        match name {
            "Output" => {
                Self::check_count(name, args.len(), 1, span)?;

                let value = self.evaluate(&args[0])?;
                let text = value.display();

                let mut stdout = io::stdout().lock();

                write!(stdout, "{}", text)
                    .map_err(|e| Diagnostic::new(span, format!("Failed to write output: {}", e)))?;

                stdout
                    .flush()
                    .map_err(|e| Diagnostic::new(span, format!("Failed to flush output: {}", e)))?;

                if !text.is_empty() {
                    OUTPUT_LINE_OPEN.store(!text.ends_with('\n'), Ordering::Relaxed);
                }

                return Ok(FunctionResult::NoValue);
            }

            "Input" => {
                Self::check_count(name, args.len(), 1, span)?;

                let prompt = match self.evaluate(&args[0])? {
                    Value::Str(text) => text,

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Input() requires a str prompt, got {}", other.type_name()),
                        ));
                    }
                };

                // Print the prompt exactly like Output() does,
                // so it is visible before the program starts waiting.
                {
                    let mut stdout = io::stdout().lock();

                    write!(stdout, "{}", prompt).map_err(|e| {
                        Diagnostic::new(span, format!("Failed to write output: {}", e))
                    })?;

                    stdout.flush().map_err(|e| {
                        Diagnostic::new(span, format!("Failed to flush output: {}", e))
                    })?;
                }

                if !prompt.is_empty() {
                    OUTPUT_LINE_OPEN.store(!prompt.ends_with('\n'), Ordering::Relaxed);
                }

                let mut line = String::new();

                let bytes_read = io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(|e| Diagnostic::new(span, format!("Failed to read input: {}", e)))?;

                if bytes_read == 0 {
                    return Err(Diagnostic::new(
                        span,
                        "Input() reached the end of input: nothing left to read",
                    ));
                }

                // Remove only the Enter itself (\n or \r\n).
                // Everything the user typed stays untouched.
                if line.ends_with('\n') {
                    line.pop();

                    if line.ends_with('\r') {
                        line.pop();
                    }
                }

                // In a terminal the user's Enter already moved
                // the cursor to a new line. With piped input it did not.
                if io::stdin().is_terminal() {
                    OUTPUT_LINE_OPEN.store(false, Ordering::Relaxed);
                }

                return Ok(FunctionResult::Value(Value::Str(line)));
            }

            "Includes" => {
                Self::check_count(name, args.len(), 2, span)?;

                let haystack = self.evaluate(&args[0])?;
                let needle = self.evaluate(&args[1])?;

                return match haystack {
                    Value::Str(text) => {
                        let pattern = match needle {
                            Value::Str(pattern) => pattern,

                            other => {
                                return Err(Diagnostic::new(
                                    args[1].span,
                                    format!(
                                        "Includes() requires a str to search inside a str, got {}",
                                        other.type_name()
                                    ),
                                ));
                            }
                        };

                        if pattern.is_empty() {
                            return Err(Diagnostic::new(
                                args[1].span,
                                "Includes() cannot search for an empty string",
                            ));
                        }

                        Ok(Value::Bool(text.contains(&pattern)))
                    }

                    Value::List(inner, elements) => {
                        if !type_accepts(&inner, &needle.type_name()) {
                            return Err(Diagnostic::new(
                                args[1].span,
                                format!(
                                    "Includes() cannot search for {} inside ls<{}>",
                                    needle.type_name(),
                                    inner
                                ),
                            ));
                        }

                        Ok(Value::Bool(elements.contains(&needle)))
                    }

                    other => Err(Diagnostic::new(
                        args[0].span,
                        format!(
                            "Includes() requires a string or list, got {}",
                            other.type_name()
                        ),
                    )),
                }
                .map(FunctionResult::Value);
            }

            "Length" => {
                Self::check_count(name, args.len(), 1, span)?;

                return match self.evaluate(&args[0])? {
                    Value::Str(text) => Ok(Value::Int(
                        i64::try_from(text.chars().count())
                            .map_err(|_| Diagnostic::new(span, "String too long"))?,
                    )),

                    Value::List(_, elements) => Ok(Value::Int(
                        i64::try_from(elements.len())
                            .map_err(|_| Diagnostic::new(span, "List too long"))?,
                    )),

                    other => Err(Diagnostic::new(
                        args[0].span,
                        format!(
                            "Length() requires a string or list, got {}",
                            other.type_name()
                        ),
                    )),
                }
                .map(FunctionResult::Value);
            }

            "Insert" => {
                Self::check_count(name, args.len(), 3, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;
                let element = self.evaluate(&args[2])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Insert() expects a list, got {}", other.type_name()),
                        ));
                    }
                };

                let n = match index {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                if !type_accepts(&inner, &element.type_name()) {
                    return Err(Diagnostic::new(
                        args[2].span,
                        format!("Cannot insert {} into ls<{}>", element.type_name(), inner),
                    ));
                }

                let index = resolve_index(n, elements.len(), true, args[1].span)?;

                elements.insert(index, element);

                return Ok(FunctionResult::Value(Value::List(inner, elements)));
            }

            "Remove" => {
                Self::check_count(name, args.len(), 2, span)?;

                let list = self.evaluate(&args[0])?;
                let index = self.evaluate(&args[1])?;

                let (inner, mut elements) = match list {
                    Value::List(inner, elements) => (inner, elements),

                    other => {
                        return Err(Diagnostic::new(
                            args[0].span,
                            format!("Remove() expects a list, got {}", other.type_name()),
                        ));
                    }
                };

                let n = match index {
                    Value::Int(n) => n,

                    other => {
                        return Err(Diagnostic::new(
                            args[1].span,
                            format!("Index must be int, got {}", other.type_name()),
                        ));
                    }
                };

                let index = resolve_index(n, elements.len(), false, args[1].span)?;

                elements.remove(index);

                return Ok(FunctionResult::Value(Value::List(inner, elements)));
            }

            _ => {}
        }

        let definition = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::new(span, format!("Unknown function '{}'", name)))?;

        if args.len() != definition.parameters.len() {
            return Err(Diagnostic::new(
                span,
                format!(
                    "Function '{}' expects {} argument(s), got {}",
                    name,
                    definition.parameters.len(),
                    args.len()
                ),
            ));
        }

        // Remember each argument's position along with its value.
        let mut values = Vec::new();

        for arg in args {
            values.push((self.evaluate(arg)?, arg.span));
        }

        // A function cannot see the caller's local variables.
        let saved_scopes = self.scopes.split_off(1);

        self.return_types.push(definition.result.clone());

        let result = self.call_inner(&definition, values);

        self.return_types.pop();
        self.scopes.truncate(1);
        self.scopes.extend(saved_scopes);

        result
    }

    fn call_inner(
        &mut self,
        definition: &FunctionDef,
        values: Vec<(Value, Span)>,
    ) -> EResult<FunctionResult<Value>> {
        let mut frame = HashMap::new();
        let mut values = values.into_iter();

        for parameter in &definition.parameters {
            let (value, value_span) = values.next().ok_or_else(|| {
                Diagnostic::new(
                    parameter.name.span,
                    format!(
                        "Missing argument '{}' for '{}'",
                        parameter.name.text, definition.name.text
                    ),
                )
            })?;

            if !type_accepts(&parameter.declared_type.name, &value.type_name()) {
                return Err(Diagnostic::new(
                    value_span,
                    format!(
                        "Argument '{}' of '{}' expects {}, got {}",
                        parameter.name.text,
                        definition.name.text,
                        parameter.declared_type.name,
                        value.type_name()
                    ),
                ));
            }

            frame.insert(
                parameter.name.text.clone(),
                Variable {
                    declared_type: parameter.declared_type.name.clone(),
                    value,
                },
            );
        }

        self.scopes.push(frame);

        let (annotation, value) = match self.execute_block(&definition.body)? {
            ExecFlow::Return(value) => match &definition.result {
                FunctionResult::Value(ty) => (ty, value),
                FunctionResult::NoValue => {
                    return Err(Diagnostic::new(definition.name.span, NO_VALUE_RETURN));
                }
            },
            ExecFlow::Continue if matches!(definition.result, FunctionResult::NoValue) => {
                return Ok(FunctionResult::NoValue);
            }
            ExecFlow::Continue => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    format!(
                        "Function '{}' finished without return",
                        definition.name.text
                    ),
                ));
            }
            ExecFlow::Break => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    "'break' cannot escape a function",
                ));
            }
            ExecFlow::Skip => {
                return Err(Diagnostic::new(
                    definition.name.span,
                    "'skip' cannot escape a function",
                ));
            }
        };

        if !type_accepts(&annotation.name, &value.type_name()) {
            return Err(Diagnostic::new(
                annotation.span,
                format!(
                    "Function '{}' declares return type {}, got {}",
                    definition.name.text,
                    annotation.name,
                    value.type_name()
                ),
            ));
        }

        Ok(FunctionResult::Value(value))
    }
}

// ============================================================
// Entry point and commands
// ============================================================

enum Failure {
    Syntax(Diagnostic),
    Static(Vec<Diagnostic>),
    Runtime(Diagnostic),
}

fn parse_source(source: &str) -> Result<Vec<Stmt>, Failure> {
    let tokens = lex(source).map_err(Failure::Syntax)?;

    Parser::new(tokens).parse_program().map_err(Failure::Syntax)
}

fn check_source(source: &str) -> Result<Vec<Stmt>, Failure> {
    let program = parse_source(source)?;
    let errors = Checker::new().check_program(&program);

    if errors.is_empty() {
        Ok(program)
    } else {
        Err(Failure::Static(errors))
    }
}

fn execute(source: &str) -> Result<(), Failure> {
    let program = check_source(source)?;

    Interpreter::new().run(&program).map_err(Failure::Runtime)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Formatting needs valid syntax and also works on statically invalid code.
    let format_check = args.len() == 4 && args[1] == "format" && args[2] == "--check";
    let format_write = args.len() == 3 && args[1] == "format";

    if format_check || format_write {
        let path = if format_check { &args[3] } else { &args[2] };
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("Cannot read '{}': {}", path, error);
                std::process::exit(1);
            }
        };

        let formatted = match formatter::format(&source) {
            Ok(formatted) => formatted,
            Err(error) => {
                print_diagnostic("EphoFMT error", path, &source, &error);
                std::process::exit(1);
            }
        };

        if format_check {
            if formatted == source {
                println!("OK: '{}' is formatted", path);
            } else {
                eprintln!("Not formatted: '{}'", path);
                std::process::exit(1);
            }
        } else {
            if formatted != source {
                if let Err(error) = fs::write(path, formatted) {
                    eprintln!("Cannot write '{}': {}", path, error);
                    std::process::exit(1);
                }
            }
            println!("Formatted '{}'", path);
        }
        return;
    }

    if args.len() != 3 {
        eprintln!("Usage:");
        eprintln!("  epholang tokens <file.epho>");
        eprintln!("  epholang parse <file.epho>");
        eprintln!("  epholang check <file.epho>");
        eprintln!("  epholang run <file.epho>");
        eprintln!("  epholang format <file.epho>");
        eprintln!("  epholang format --check <file.epho>");
        std::process::exit(1);
    }

    let command = &args[1];
    let path = &args[2];

    let source = match fs::read_to_string(path) {
        Ok(source) => source,

        Err(e) => {
            eprintln!("Cannot read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let result = match command.as_str() {
        "tokens" => lex(&source).map_err(Failure::Syntax).map(|tokens| {
            for token in tokens {
                println!("{:?}", token);
            }
        }),

        "parse" => parse_source(&source).map(|program| {
            for stmt in program {
                println!("{:#?}", stmt);
            }
        }),

        "check" => check_source(&source).map(|_| println!("OK: no static errors")),

        "run" => execute(&source),

        _ => {
            eprintln!("Unknown command '{}'", command);
            std::process::exit(1);
        }
    };

    if let Err(failure) = result {
        if OUTPUT_LINE_OPEN.load(Ordering::Relaxed) {
            println!();
        }

        let _ = io::stdout().flush();

        match failure {
            Failure::Syntax(diagnostic) => {
                print_diagnostic("Syntax error", path, &source, &diagnostic);
            }

            Failure::Static(diagnostics) => {
                for diagnostic in diagnostics {
                    print_diagnostic("Static error", path, &source, &diagnostic);
                }
            }

            Failure::Runtime(diagnostic) => {
                print_diagnostic("Runtime error", path, &source, &diagnostic);
            }
        }

        std::process::exit(1);
    }
}

#[cfg(test)]
mod stage1_tests {
    use super::*;

    #[test]
    fn return_markers_and_discard_tokens_are_unambiguous() {
        let tokens = lex("-| -> - | || -/* gap */| _ _item item_").unwrap();
        for (token, symbol) in tokens.iter().zip(["-|", "->", "-", "|", "||", "-", "|"]) {
            assert!(token.symbol(symbol));
        }
        assert_eq!(tokens[7].kind, TokenKind::Discard);
        assert_eq!(tokens[8].kind, TokenKind::Identifier("_item".into()));
        assert_eq!(tokens[9].kind, TokenKind::Identifier("item_".into()));
    }

    #[test]
    fn numeric_literals_have_distinct_types() {
        let tokens = lex("5 5.0").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Integer(5)));
        assert!(matches!(tokens[1].kind, TokenKind::Float(v) if v == 5.0));
    }

    #[test]
    fn function_syntax_parses_and_checks() {
        let program = "function Square(int x) -> int:\n\treturn int x * x\nOutput(Square(5))";
        assert!(check_source(program).is_ok());
    }

    #[test]
    fn default_parameters_are_rejected() {
        let source = "function Bad(int x = 1) -> int:\n\treturn int x";
        assert!(parse_source(source).is_err());
    }

    #[test]
    fn mixed_arithmetic_is_rejected() {
        assert!(check_source("Output(1 + 2.0)").is_err());
    }

    #[test]
    fn integer_quotient_exactness() {
        assert_eq!(exact_integer_quotient(5, 2), Some(2.5));
        assert_eq!(exact_integer_quotient(1, 4), Some(0.25));
        assert_eq!(exact_integer_quotient(1, 3), None);
        assert_eq!(exact_integer_quotient(1, 10), None);
        assert_eq!(exact_integer_quotient(0, 5), Some(0.0));
        assert_eq!(exact_integer_quotient(5, 0), None);
        assert_eq!(
            exact_integer_quotient(9_007_199_254_740_993, 3),
            Some(3_002_399_751_580_331.0)
        );
    }

    #[test]
    fn random_interval_inclusive_equal_bounds() {
        let source =
            "import \"math\" as math\nint value := math:RandomInterval(7, 7)\nOutput(value)";
        assert!(check_source(source).is_ok());
        assert!(execute(source).is_ok());
    }

    #[test]
    fn random_interval_rejects_inverted_bounds() {
        let source = "import \"math\" as math\nOutput(math:RandomInterval(8, 4))";
        assert!(check_source(source).is_err());
    }

    #[test]
    fn integer_index_and_length() {
        let source =
            "ls<int> values := list<int>{1, 2, 3}\nOutput(values[1])\nOutput(Length(values))";
        assert!(check_source(source).is_ok());
    }
}

#[cfg(test)]
fn evaluate_checked(source: &str) -> EResult<Value> {
    let program = match check_source(source) {
        Ok(program) => program,
        Err(Failure::Static(errors)) => panic!("Static errors: {:?}", errors),
        Err(Failure::Syntax(err)) => panic!("Syntax error: {:?}", err),
        Err(Failure::Runtime(_)) => unreachable!(),
    };
    let mut vm = Interpreter::new();
    let (last, previous) = program.split_last().expect("Non-empty program");
    vm.execute_block(previous)?;
    if let Statement::Discard(expr) = &last.kind {
        vm.evaluate(expr)
    } else {
        panic!("Expected a final '_ = expression' in the test program");
    }
}

#[cfg(test)]
mod stage2_tests {
    use super::*;

    fn math(expr: &str) -> EResult<Value> {
        evaluate_checked(&format!("import \"math\" as math\n_ = {expr}"))
    }

    fn types(expr: &str) -> EResult<Value> {
        evaluate_checked(&format!("import \"types\" as types\n_ = {expr}"))
    }

    #[test]
    fn break_and_skip_control_while_loops() {
        let source = r#"
int i := 0
int total := 0
loop i < 10:
	i = i + 1
	if i == 2:
		skip
	if i == 5:
		break
	total = total + i
_ = total
"#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(8));
    }

    #[test]
    fn break_and_skip_control_for_in_loops() {
        let source = r#"
ls<int> values := list<int>{1, 2, 3, 4}
int total := 0
loop item in values:
	if item == 2:
		skip
	total = total + item
	if item == 3:
		break
_ = total
"#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(4));
    }

    #[test]
    fn loop_control_only_affects_the_nearest_nested_loop() {
        let source = r#"
int outer := 0
int total := 0
loop outer < 3:
	outer = outer + 1
	int inner := 0
	loop inner < 4:
		inner = inner + 1
		if inner == 2:
			break
		total = total + 1
_ = total
"#;

        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(3));
    }

    #[test]
    fn loop_control_is_rejected_outside_loops() {
        assert!(matches!(check_source("break"), Err(Failure::Static(_))));
        assert!(matches!(check_source("skip"), Err(Failure::Static(_))));
    }

    #[test]
    fn function_cannot_control_a_callers_loop() {
        let source = r#"
function Invalid() -> int:
	break
	return int 1
loop true:
	Output(Invalid())
	break
"#;

        assert!(matches!(check_source(source), Err(Failure::Static(_))));
    }

    #[test]
    fn loop_control_is_allowed_inside_a_functions_own_loop() {
        let source = r#"
function First() -> int:
	loop true:
		break
	return int 1
Output(First())
"#;

        assert!(check_source(source).is_ok());
        assert!(execute(source).is_ok());
    }

    #[test]
    fn absolute_value_and_square_root() {
        assert_eq!(math("math:AbsoluteValue((-17))").unwrap(), Value::Int(17));
        assert_eq!(math("math:AbsoluteValue((-2.5))").unwrap(), Value::Flt(2.5));
        assert_eq!(math("math:SquareRoot(81.0)").unwrap(), Value::Flt(9.0));
        assert!(math("math:SquareRoot((-1.0))").is_err());
        assert!(check_source("import \"math\" as math\n_ = math:SquareRoot(9)").is_err());
    }

    #[test]
    fn absolute_value_integer_minimum_rejected() {
        let source = "import \"math\" as math\nimport \"types\" as types\n_ = math:AbsoluteValue(types:Convert(\"-9223372036854775808\", int))";
        assert!(evaluate_checked(source).is_err());
    }

    #[test]
    fn rounding_away_from_zero() {
        assert_eq!(math("math:Round(2.5)").unwrap(), Value::Flt(3.0));
        assert_eq!(math("math:Round((-2.5))").unwrap(), Value::Flt(-3.0));
        assert_eq!(math("math:Floor((-2.1))").unwrap(), Value::Flt(-3.0));
        assert_eq!(math("math:Ceil((-2.1))").unwrap(), Value::Flt(-2.0));
    }

    #[test]
    fn min_max_clamp_typed_and_bounds_checked() {
        assert_eq!(math("math:Minimum(3, 8)").unwrap(), Value::Int(3));
        assert_eq!(math("math:Maximum(3.0, 8.0)").unwrap(), Value::Flt(8.0));
        assert_eq!(math("math:Clamp(15, 0, 10)").unwrap(), Value::Int(10));
        assert_eq!(
            math("math:Clamp((-2.0), 0.0, 10.0)").unwrap(),
            Value::Flt(0.0)
        );
        assert!(math("math:Clamp(1, 3, 2)").is_err());
        assert!(check_source("import \"math\" as math\n_ = math:Minimum(2, 3.0)").is_err());
    }

    #[test]
    fn sum_empty_and_overflow() {
        assert_eq!(math("math:Sum(list<int>{})").unwrap(), Value::Int(0));
        assert_eq!(math("math:Sum(list<int>{1, 2, 3})").unwrap(), Value::Int(6));
        assert_eq!(
            math("math:Sum(list<flt>{1.5, 2.5})").unwrap(),
            Value::Flt(4.0)
        );
        assert!(math("math:Sum(list<int>{9223372036854775807, 1})").is_err());
        assert!(check_source("import \"math\" as math\n_ = math:Sum(list<str>{\"x\"})").is_err());
    }

    #[test]
    fn average_exactness_int_and_floats() {
        assert_eq!(
            math("math:Average(list<int>{10, 20, 30})").unwrap(),
            Value::Flt(20.0)
        );
        assert_eq!(
            math("math:Average(list<int>{1, 2})").unwrap(),
            Value::Flt(1.5)
        );
        assert_eq!(
            math("math:Average(list<flt>{2.0, 4.0})").unwrap(),
            Value::Flt(3.0)
        );
        assert!(math("math:Average(list<int>{1, 2, 2})").is_err());
        assert!(math("math:Average(list<int>{})").is_err());
        assert!(math("math:Average(list<flt>{})").is_err());
    }

    #[test]
    fn random_interval_keeps_inclusive_endpoints() {
        // Equal bounds prove that both ends of an inclusive range are accepted.
        assert_eq!(math("math:RandomInterval(5, 5)").unwrap(), Value::Int(5));

        // Invalid literal bounds are rejected during static checking, not at runtime.
        let literal_bounds = "import \"math\" as math\n_ = math:RandomInterval(10, 1)";
        assert!(matches!(
            check_source(literal_bounds),
            Err(Failure::Static(_))
        ));

        // Non-literal bounds pass static checking but fail at runtime when inverted.
        let dynamic_bounds = "import \"math\" as math\nint minimum := 10\nint maximum := 1\n_ = math:RandomInterval(minimum, maximum)";
        assert!(check_source(dynamic_bounds).is_ok());
        assert!(evaluate_checked(dynamic_bounds).is_err());

        // The numeric types of both boundaries are checked statically.
        assert!(check_source("import \"math\" as math\n_ = math:RandomInterval(1, 2.0)").is_err());
    }

    #[test]
    fn conversions_validate_values() {
        assert_eq!(
            types("types:Convert(\"123\", int)").unwrap(),
            Value::Int(123)
        );
        assert_eq!(
            types("types:Convert(\"3.25\", flt)").unwrap(),
            Value::Flt(3.25)
        );
        assert_eq!(types("types:Convert(3.0, int)").unwrap(), Value::Int(3));
        assert_eq!(
            types("types:Convert(42, str)").unwrap(),
            Value::Str("42".into())
        );
        assert_eq!(
            types("types:Convert(\"true\", bool)").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            types("types:Convert(false, str)").unwrap(),
            Value::Str("false".into())
        );
        assert!(types("types:Convert(3.9, int)").is_err());
        assert!(types("types:Convert(9223372036854775807, flt)").is_err());
        assert!(types("types:Convert(9223372036854775808.0, int)").is_err());
        assert!(types("types:Convert(\"bad\", int)").is_err());
    }

    #[test]
    fn convert_string_to_list_of_strings() {
        assert_eq!(
            types(r#"types:Convert("hello", ls<str>)"#).unwrap(),
            Value::List(
                "str".into(),
                ["h", "e", "l", "l", "o"]
                    .into_iter()
                    .map(|character| Value::Str(character.into()))
                    .collect()
            )
        );
    }

    #[test]
    fn convert_string_to_list_of_strings_supports_unicode_scalar_values() {
        assert_eq!(
            types(r#"types:Convert("Привет", ls<str>)"#).unwrap(),
            Value::List(
                "str".into(),
                ["П", "р", "и", "в", "е", "т"]
                    .into_iter()
                    .map(|character| Value::Str(character.into()))
                    .collect()
            )
        );
        assert_eq!(
            types(r#"types:Convert("A🙂Б", ls<str>)"#).unwrap(),
            Value::List(
                "str".into(),
                ["A", "🙂", "Б"]
                    .into_iter()
                    .map(|character| Value::Str(character.into()))
                    .collect()
            )
        );
    }

    #[test]
    fn convert_empty_string_to_empty_string_list() {
        assert_eq!(
            types(r#"types:Convert("", ls<str>)"#).unwrap(),
            Value::List("str".into(), Vec::new())
        );
    }

    #[test]
    fn is_convertible_supports_string_to_string_list() {
        assert_eq!(
            types(r#"types:IsConvertible("hello", ls<str>)"#).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            types(r#"types:IsConvertible(123, ls<str>)"#).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn string_to_list_conversion_rejects_other_list_targets() {
        assert!(
            check_source(
                r#"import "types" as types
_ = types:Convert("hello", ls<int>)"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "types" as types
_ = types:Convert(123, ls<str>)"#
            )
            .is_err()
        );
    }

    #[test]
    fn is_convertible_returns_false_without_error() {
        assert_eq!(
            types("types:IsConvertible(\"123\", int)").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            types("types:IsConvertible(\"bad\", int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(3.9, int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(true, int)").unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            types("types:IsConvertible(list<int>{1}, str)").unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn convert_rejects_unsupported_pair_statically() {
        assert!(check_source("import \"types\" as types\n_ = types:Convert(true, int)").is_err());
        assert!(
            check_source("import \"types\" as types\n_ = types:Convert(list<int>{1}, str)")
                .is_err()
        );
    }

    #[test]
    fn type_target_must_be_literal() {
        assert!(
            check_source("import \"types\" as types\n_ = types:Convert(1, types:CurrentTypeOf(1))")
                .is_err()
        );
        assert!(
            check_source("import \"types\" as types\n_ = types:IsConvertible(1, ls<int>)").is_err()
        );
    }

    fn strings(expr: &str) -> EResult<Value> {
        evaluate_checked(&format!("import \"strings\" as strings\n_ = {expr}"))
    }

    #[test]
    fn alphabet_range_supports_inclusive_uppercase_lowercase_and_reverse_ranges() {
        assert_eq!(
            strings(r#"strings:AlphabetRange("A", "F")"#).unwrap(),
            Value::Str("ABCDEF".into())
        );
        assert_eq!(
            strings(r#"strings:AlphabetRange("A", "S")"#).unwrap(),
            Value::Str("ABCDEFGHIJKLMNOPQRS".into())
        );
        assert_eq!(
            strings(r#"strings:AlphabetRange("a", "f")"#).unwrap(),
            Value::Str("abcdef".into())
        );
        assert_eq!(
            strings(r#"strings:AlphabetRange("F", "A")"#).unwrap(),
            Value::Str("FEDCBA".into())
        );
        assert_eq!(
            strings(r#"strings:AlphabetRange("z", "x")"#).unwrap(),
            Value::Str("zyx".into())
        );
        assert_eq!(
            strings(r#"strings:AlphabetRange("M", "M")"#).unwrap(),
            Value::Str("M".into())
        );
    }

    #[test]
    fn alphabet_range_rejects_invalid_literal_endpoints_statically() {
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:AlphabetRange("AA", "Z")"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:AlphabetRange("A", "z")"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:AlphabetRange("1", "9")"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:AlphabetRange("Я", "Я")"#
            )
            .is_err()
        );
    }

    #[test]
    fn alphabet_range_rejects_invalid_dynamic_endpoints_at_runtime() {
        let source = r#"import "strings" as strings
str start := "A"
str end := "z"
_ = strings:AlphabetRange(start, end)"#;
        assert!(check_source(source).is_ok());
        assert!(evaluate_checked(source).is_err());
    }

    #[test]
    fn ascii_case_predicates_check_every_character() {
        assert_eq!(
            strings(r#"strings:IsAsciiUppercase("ABC")"#).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiUppercase("A1")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiUppercase("AbC")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiUppercase("Я")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiUppercase("")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiLowercase("abc")"#).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiLowercase("a1")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiLowercase("Abc")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiLowercase("я")"#).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            strings(r#"strings:IsAsciiLowercase("")"#).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn ascii_string_functions_validate_argument_types_and_counts() {
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:AlphabetRange(1, "Z")"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:IsAsciiUppercase(1)"#
            )
            .is_err()
        );
        assert!(
            check_source(
                r#"import "strings" as strings
_ = strings:IsAsciiLowercase("a", "b")"#
            )
            .is_err()
        );
    }

    #[test]
    fn string_indexing_is_one_based_and_returns_str() {
        assert_eq!(
            evaluate_checked(r#"_ = "hello"[1]"#).unwrap(),
            Value::Str("h".into())
        );
        assert_eq!(
            evaluate_checked(r#"_ = "hello"[5]"#).unwrap(),
            Value::Str("o".into())
        );
        assert_eq!(
            evaluate_checked(
                r#"str text := "hello"
_ = text[2]"#
            )
            .unwrap(),
            Value::Str("e".into())
        );
    }

    #[test]
    fn string_indexing_uses_unicode_scalar_values() {
        assert_eq!(
            evaluate_checked(r#"_ = "Привет"[1]"#).unwrap(),
            Value::Str("П".into())
        );
        assert_eq!(
            evaluate_checked(r#"_ = "A🙂Б"[2]"#).unwrap(),
            Value::Str("🙂".into())
        );
        assert_eq!(
            evaluate_checked(r#"_ = "A🙂Б"[3]"#).unwrap(),
            Value::Str("Б".into())
        );
    }

    #[test]
    fn string_indexing_rejects_zero_and_out_of_bounds() {
        assert!(matches!(
            check_source(r#"_ = "hello"[0]"#),
            Err(Failure::Static(_))
        ));
        assert!(evaluate_checked(r#"_ = "hello"[6]"#).is_err());
    }

    #[test]
    fn string_indexing_requires_an_integer_index() {
        assert!(matches!(
            check_source(r#"_ = "hello"["1"]"#),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn strings_are_not_mutable_through_index_assignment() {
        assert!(matches!(
            check_source(
                r#"str text := "hello"
text[1] = "H""#
            ),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn list_indexing_still_works_after_string_indexing() {
        assert_eq!(
            evaluate_checked("ls<int> values := list<int>{10, 20, 30}\n_ = values[2]").unwrap(),
            Value::Int(20)
        );
    }

    #[test]
    fn module_imports_and_invalid_arity() {
        assert!(check_source("import \"math\" as math\nimport \"types\" as types\nOutput(math:AbsoluteValue(types:Convert(\"-4\", int)))").is_ok());
        assert!(check_source("import \"math\" as math\n_ = math:AbsoluteValue(1, 2)").is_err());
        assert!(check_source("import \"types\" as types\n_ = types:Convert(1)").is_err());
        assert!(check_source("import \"types\" as types\n_ = types:Unknown(1)").is_err());
    }
}

#[cfg(test)]
mod stage3_tests {
    use super::*;

    #[test]
    fn includes_allows_empty_string_as_list_element() {
        let source = r#"ls<str> items := list<str>{"", "value"}
Output(Includes(items, ""))"#;
        assert!(
            check_source(source).is_ok(),
            "Empty strings are valid list elements"
        );
        let program = check_source(source).ok().unwrap();
        // The list membership operation itself must evaluate to true.
        let mut interpreter = Interpreter::new();
        let (last, preceding) = program.split_last().unwrap();
        interpreter.execute_block(preceding).unwrap();
        let Statement::Expression(Expr {
            kind: ExprKind::Call(_, args),
            ..
        }) = &last.kind
        else {
            panic!("Expected Output(...) expression");
        };
        assert_eq!(interpreter.evaluate(&args[0]).unwrap(), Value::Bool(true));
    }

    #[test]
    fn includes_rejects_empty_substring_statically() {
        assert!(matches!(
            check_source("Output(Includes(\"abc\", \"\"))"),
            Err(Failure::Static(_))
        ));
    }

    #[test]
    fn fractional_output_preserves_tiny_nonzero_values() {
        assert_eq!(
            Value::Flt(0.00000000000000001).display(),
            "0.00000000000000001"
        );
        assert_eq!(
            Value::Flt(1.2345678901234567).display(),
            "1.2345678901234567"
        );
    }

    #[test]
    fn integer_conversion_boundaries_are_checked() {
        assert_eq!(
            convert_value(&Value::Str("-9223372036854775808".into()), "int"),
            Some(Value::Int(i64::MIN))
        );
        assert_eq!(
            convert_value(&Value::Str("9223372036854775807".into()), "int"),
            Some(Value::Int(i64::MAX))
        );
        assert!(convert_value(&Value::Flt(9_223_372_036_854_775_808.0), "int").is_none());
        assert!(convert_value(&Value::Int(i64::MAX), "flt").is_none());
    }

    #[test]
    fn exact_division_rejects_non_dyadic_quotients() {
        assert_eq!(exact_integer_quotient(5, 2), Some(2.5));
        assert_eq!(exact_integer_quotient(1, 3), None);
        assert_eq!(exact_integer_quotient(0, 3), Some(0.0));
        assert_eq!(exact_integer_quotient(1, 0), None);
    }
}

#[cfg(test)]
mod union_type_tests {
    use super::*;

    #[test]
    fn declared_and_current_type_of_are_available_in_types_module() {
        let declared = r#"import "types" as types
int | str value := 10
_ = types:DeclaredTypeOf(value)"#;
        let initial = r#"import "types" as types
int | str value := 10
_ = types:CurrentTypeOf(value)"#;
        let updated = r#"import "types" as types
int | str value := 10
value = "Hi!"
_ = types:CurrentTypeOf(value)"#;

        assert_eq!(
            evaluate_checked(declared).unwrap(),
            Value::Type("int | str".into())
        );
        assert_eq!(
            evaluate_checked(initial).unwrap(),
            Value::Type("int".into())
        );
        assert_eq!(
            evaluate_checked(updated).unwrap(),
            Value::Type("str".into())
        );
    }

    #[test]
    fn declared_type_of_requires_a_variable() {
        let source = r#"import "types" as types
_ = types:DeclaredTypeOf(10)"#;
        assert!(check_source(source).is_err());
    }

    #[test]
    fn union_types_parse_in_declarations_parameters_and_returns() {
        let source = r#"
function Echo(int | str value) -> int | str:
	return int | str value
int | str item := 42
item = "answer"
_ = Echo(item)
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(
            evaluate_checked(source).unwrap(),
            Value::Str("answer".into())
        );
    }

    #[test]
    fn union_lists_accept_each_declared_member() {
        let source = r#"
ls<int | str> values := list<int | str>{1, "two", 3}
_ = Includes(values, "two")
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Bool(true));
    }

    #[test]
    fn union_lists_return_union_element_types() {
        let source = r#"
ls<int | str> values := list<int | str>{1, "two"}
_ = values[2]
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Str("two".into()));
    }

    #[test]
    fn union_of_list_types_can_be_indexed_and_measured() {
        let source = r#"
ls<int> | ls<str> values := list<int>{1, 2}
_ = Length(values)
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Int(2));
    }

    #[test]
    fn union_types_reject_values_outside_the_union() {
        let source = "int | str value := true";
        assert!(matches!(check_source(source), Err(Failure::Static(_))));
    }

    #[test]
    fn union_list_elements_reject_values_outside_the_union() {
        let source = "ls<int | str> values := list<int | str>{1, true}";
        assert!(matches!(check_source(source), Err(Failure::Static(_))));
    }

    #[test]
    fn union_of_list_types_rejects_unsafe_mutation() {
        let source = r#"
ls<int> | ls<str> values := list<int>{1, 2}
values[1] = "text"
"#;

        assert!(matches!(check_source(source), Err(Failure::Static(_))));
    }

    #[test]
    fn function_arguments_accept_union_members_but_not_unrelated_types() {
        let accepted = r#"
function Echo(int | str value) -> int | str:
	return int | str value
_ = Echo("text")
"#;
        let rejected = r#"
function Number(int value) -> int:
	return int value
int | str item := "text"
_ = Number(item)
"#;

        assert!(check_source(accepted).is_ok());
        assert!(matches!(check_source(rejected), Err(Failure::Static(_))));
    }

    #[test]
    fn union_types_are_flattened_and_deduplicated() {
        let ty = Type::from_name("int | str | int").unwrap();
        assert_eq!(ty.name(), "int | str");
        assert!(type_accepts("int | str", "int"));
        assert!(!type_accepts("int", "int | str"));
    }

    #[test]
    fn if_condition_supports_logical_or_at_line_start() {
        let source = r#"
function Check() -> bool:
	if false
		|| false
		|| true:
		return bool true
	else:
		return bool false
_ = Check()
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Bool(true));
    }

    #[test]
    fn if_condition_supports_logical_and_at_line_start() {
        let source = r#"
function Check() -> bool:
	if true
		&& true
		&& false:
		return bool true
	else:
		return bool false
_ = Check()
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Bool(false));
    }

    #[test]
    fn multiline_logical_operators_preserve_precedence() {
        let source = r#"
function Check() -> bool:
	if false
		|| true
		&& false:
		return bool true
	else:
		return bool false
_ = Check()
"#;

        assert!(check_source(source).is_ok());
        assert_eq!(evaluate_checked(source).unwrap(), Value::Bool(false));
    }

    #[test]
    fn ordinary_newline_still_ends_expression() {
        let source = r#"
bool value := true
false
"#;

        let program = match parse_source(source) {
            Ok(program) => program,
            Err(_) => panic!("Expected two separate statements"),
        };
        assert_eq!(program.len(), 2);
        assert!(matches!(
            program[1].kind,
            Statement::Expression(Expr {
                kind: ExprKind::Literal(Value::Bool(false)),
                ..
            })
        ));
    }
}

#[cfg(test)]
mod indentation_syntax_tests {
    use super::*;

    fn syntax_error(source: &str) -> Diagnostic {
        match parse_source(source) {
            Err(Failure::Syntax(error)) => error,
            _ => panic!("Expected a syntax error for {source:?}"),
        }
    }

    fn static_errors(source: &str) -> Vec<Diagnostic> {
        match check_source(source) {
            Err(Failure::Static(errors)) => errors,
            Err(Failure::Syntax(error)) => panic!("Unexpected syntax error: {error:?}"),
            _ => panic!("Expected static errors for {source:?}"),
        }
    }

    #[test]
    fn emits_layout_tokens_only_for_block_depth_changes() {
        let source = "if true:\r\n\t/* outer\r\n /* inner */\r\n*/\r\n\tif true:\r\n\t\tOutput(\r\n42\r\n)\r\n\t \r\nOutput(0)";
        let tokens = lex(source).unwrap();
        let markers: Vec<_> = tokens
            .iter()
            .filter(|t| matches!(t.kind, TokenKind::Indent | TokenKind::Dedent))
            .map(|t| (&t.kind, t.span.line))
            .collect();
        assert_eq!(
            markers,
            [
                (&TokenKind::Indent, 5),
                (&TokenKind::Indent, 6),
                (&TokenKind::Dedent, 10),
                (&TokenKind::Dedent, 10)
            ]
        );
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn arbitrary_tab_width_is_irrelevant_to_nesting() {
        let mut source = String::new();
        for depth in 0..40 {
            source.push_str(&"\t".repeat(depth));
            source.push_str("if true:\n");
        }
        source.push_str(&"\t".repeat(40));
        source.push_str("Output(1)");
        assert!(check_source(&source).is_ok());
        let tokens = lex(&source).unwrap();
        assert_eq!(
            tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Dedent)
                .count(),
            40
        );
    }

    #[test]
    fn indentation_and_block_diagnostics_point_to_the_affected_line() {
        let cases = [
            ("\tOutput(1)", 1, "Unexpected indentation"),
            ("if true:\n    Output(1)", 2, "must use tabs"),
            ("if true:\n \tOutput(1)", 2, "cannot mix tabs and spaces"),
            ("if true:\n\t\tOutput(1)", 2, "increase by one tab"),
            (
                "if true:\n\tOutput(1)\n\t\tOutput(2)",
                3,
                "Unexpected indentation",
            ),
            (
                "if true:\n/* just a comment */\nOutput(1)",
                3,
                "Expected an indented block",
            ),
            ("if true: Output(1)", 1, "Expected a newline"),
            ("Output(\n 1\n)", 2, "must use tabs"),
            (
                "if true:\n\tOutput(1)\n\telse:\n\t\tOutput(2)",
                3,
                "without a preceding 'if'",
            ),
        ];
        for (source, line, message) in cases {
            let error = syntax_error(source);
            assert_eq!(error.span.line, line, "{source:?}");
            assert!(error.message.contains(message), "{source:?}: {error:?}");
        }
    }

    #[test]
    fn comments_preserve_logical_statement_separators() {
        let source = "Output(1) /* a\n  /* nested */\n*/ Output(2)";
        let ast = match parse_source(source) {
            Ok(ast) => ast,
            Err(_) => panic!("Expected two statements"),
        };
        assert_eq!(ast.len(), 2);

        let source = "if true:\n\t/* opener\n   /* nested */\n   */ Output(1)\nOutput(2)\n";
        assert!(check_source(source).is_ok());
        assert_eq!(formatter::format(source).unwrap(), source);
    }

    #[test]
    fn allows_else_after_trivia_and_closes_nested_blocks_at_the_right_level() {
        let source = concat!(
            "bool ready := true\n",
            "function Choose(int value) -> str:\n",
            "\tif value == 0:\n",
            "\t\treturn str \"zero\"\n",
            "\n\t/* branches */\n",
            "\telse if value == 1:\n",
            "\t\tif ready:\n",
            "\t\t\treturn str \"ready\"\n",
            "\t\telse:\n",
            "\t\t\treturn str \"waiting\"\n",
            "\n\telse:\n",
            "\t\treturn str \"other\"\n",
            "Output(Choose(1))",
        );
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn multiline_expressions_and_lists_use_expression_delimiters() {
        let source = concat!(
            "function Add(\n\tint first,\n\tint second,\n) -> int:\n",
            "\treturn int (first +\n\t\tsecond)\n",
            "ls<\n\tint\n> items := list<int>{\n\tAdd(1,\n\t\t2),\n\t5,\n}\n",
            "loop item in list<int>{\n\titems[\n\t\t1\n\t],\n}:\n",
            "\tOutput(item)\n",
        );
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn rejects_brace_blocks_and_previous_declaration_syntax() {
        for (source, expected) in [
            ("function Test() int:\n\treturn int 1", "Expected '->'"),
            (
                "function Test(value int) -> int:\n\treturn int value",
                "reserved keyword",
            ),
            (
                "function Test() -> int {\n\treturn int 1\n}",
                "Expected ':'",
            ),
            ("if true {\n\tOutput(1)\n}", "Expected ':'"),
            ("loop true {\n\tbreak\n}", "Expected ':'"),
            (
                "if true:\n\tOutput(1)\notherwise:\n\tOutput(2)\n",
                "Expected a newline",
            ),
            ("value int := 1\nOutput(value)", "reserved keyword"),
        ] {
            let error = syntax_error(source);
            assert!(error.message.contains(expected), "{source:?}: {error:?}");
        }
    }

    #[test]
    fn checks_nested_interpolation_and_quoted_text_on_original_spans() {
        let source = concat!(
            "import \"strings\" as strings\nstr name := \"Hexer\"\n",
            r##"Output("Result: #{strings:Repeat("Hello #{name}", 2)} #{list<str>{"}" }[1]}")"##,
        );
        assert!(check_source(source).is_ok());

        let errors = static_errors(r##"Output("x #{Length("y #{missing}")}")"##);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, Span::new(1, 25, "missing".len()));

        for (source, expected) in [
            (
                "Output(\"#{(1 +\n\t2)}\")",
                "Interpolation must be on one line",
            ),
            (
                "Output(\"#{1 /*\n nested */}\")",
                "Interpolation must be on one line",
            ),
            (r##"Output("#{/* comment */}")"##, "Empty interpolation"),
            (r##"Output("#{1"##, "Unclosed interpolation"),
            (r##"Output("#{Length("text"##, "Unclosed string"),
            (
                r##"Output("#{Length("text"}")"##,
                "Expected ')' inside interpolation",
            ),
            ("Output(\"one\r\n two\")", "Newline inside string"),
        ] {
            let error = syntax_error(source);
            assert!(error.message.contains(expected), "{source:?}: {error:?}");
        }
    }

    #[test]
    fn counts_syntactic_references_and_reports_every_unused_kind() {
        let source = concat!(
            "import \"math\" as _math\n",
            "int _variable := 1\n",
            "function _B() -> int:\n\treturn int 2\n",
            "function _A(int _parameter) -> int:\n",
            "\tif false:\n\t\tOutput(_B())\n",
            "\tloop _item in list<int>{1}:\n\t\tbreak\n",
            "\treturn int 1\n",
        );
        let errors = static_errors(source);
        let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "Unused module '_math'",
                "Unused variable '_variable'",
                "Unused function '_A'",
                "Unused parameter '_parameter' in function '_A'",
                "Unused loop variable '_item'",
            ]
        );
        assert!(check_source("int x := 1\nx = 2\nls<int> xs := list<int>{1}\nxs[1] = 3").is_ok());
    }

    #[test]
    fn resolves_names_in_declaration_order_and_through_erroneous_expressions() {
        let source = concat!(
            "Output(F())\n",
            "Output(mod:Sum(list<int>{1}))\n",
            "Output(later)\n",
            "import \"math\" as mod\n",
            "function F() -> int:\n\treturn int 1\n",
            "int later := 2\n",
        );
        let errors = static_errors(source);
        for message in [
            "Function 'F' must be declared before it is called",
            "Undefined module 'mod'",
            "Undefined variable 'later'",
            "import statements must appear at the beginning of the file",
        ] {
            assert!(errors.iter().any(|e| e.message == message), "{errors:?}");
        }

        let source = concat!(
            "import \"strings\" as strings\n",
            "int value := 1\n",
            "Output(unknown + value)\n",
            "missing:Call(value)\n",
            "strings:Unknown(value)\n",
        );
        let errors = static_errors(source);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors.iter().all(|e| !e.message.contains("Unused")));

        let errors = static_errors("import \"strings\" as strings\nOutput(strings)");
        assert!(
            errors
                .iter()
                .any(|e| e.message == "Unused module 'strings'")
        );
    }

    #[test]
    fn diagnoses_direct_and_indirect_recursion_from_nested_expressions() {
        let direct = concat!(
            "function Direct() -> str:\n",
            "\tif false:\n\t\tOutput(missing + Direct())\n",
            "\treturn str \"#{Direct()}\"\n",
        );
        let errors = static_errors(direct);
        assert_eq!(
            errors
                .iter()
                .filter(|e| e.message.contains("cannot call itself"))
                .count(),
            2
        );

        let indirect = concat!(
            "function A() -> str:\n",
            "\treturn str \"#{B()}\"\n",
            "function B() -> str:\n",
            "\treturn str list<str>{C()}[1]\n",
            "function C() -> str:\n",
            "\tloop false:\n",
            "\t\tmissing:Call(A())\n",
            "\treturn str \"done\"\n",
        );
        let errors = static_errors(indirect);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("A -> B -> C -> A")),
            "{errors:?}"
        );
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;

    fn errors(source: &str) -> Vec<Diagnostic> {
        match check_source(source) {
            Err(Failure::Static(errors)) => errors,
            _ => panic!("Expected static errors in {source}"),
        }
    }

    #[test]
    fn typed_functions_preserve_their_returned_values() {
        for (ty, literal, expected) in [
            ("int", "42", Value::Int(42)),
            ("flt", "2.5", Value::Flt(2.5)),
            ("bool", "false", Value::Bool(false)),
            ("str", "\"hi\"", Value::Str("hi".into())),
            ("none", "null", Value::Null),
        ] {
            let source = format!("function F() -> {ty}:\n\treturn {ty} {literal}\n_ = F()");
            assert_eq!(evaluate_checked(&source).unwrap(), expected);
        }
    }

    #[test]
    fn standalone_values_require_explicit_discard() {
        for (setup, expression) in [
            ("", "42"),
            ("", "null"),
            ("int x := 1\n", "x"),
            ("function F() -> int:\n\treturn int 1\n", "F() + 10"),
            ("function F() -> none:\n\treturn none null\n", "F()"),
            ("", "Length(\"hi\")"),
            ("", "Input(\"\")"),
            ("import \"math\" as m\n", "m:Average(list<int>{1})"),
            ("import \"strings\" as s\n", "s:ToUpperCase(\"hi\")"),
            ("import \"types\" as t\n", "t:CurrentTypeOf(null)"),
        ] {
            let diagnostics = errors(&format!("{setup}{expression}"));
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            assert_eq!(diagnostics[0].message, UNUSED_VALUE);
            assert_eq!(diagnostics[0].span.length, expression.len());
            assert!(check_source(&format!("{setup}_ = {expression}")).is_ok());
        }
    }

    #[test]
    fn no_value_is_rejected_in_every_value_context() {
        let def = "function Display() -|:\n\tOutput(1)\n";
        for usage in [
            "int x := Display()\nOutput(x)",
            "int x := 1\nx = Display()",
            "Output(Display())",
            "if Display():\n\tOutput(1)",
            "loop Display():\n\tbreak",
            "_ = Display() + 1",
            "_ = !Display()",
            "_ = list<int>{Display()}",
            "_ = list<int>{1}[Display()]",
            "_ = Display()[1]",
            "loop x in Display():\n\tOutput(x)",
            "Output(\"#{Display()}\")",
            "Output(\"#{\"#{Display()}\"}\")",
            "_ = false && Display()",
            "function F() -> none:\n\treturn none Display()\n_ = F()",
        ] {
            let diagnostics = errors(&format!("{def}{usage}"));
            assert!(
                diagnostics.iter().any(|d| d.message == VALUE_REQUIRED),
                "{usage}: {diagnostics:?}"
            );
            assert!(!diagnostics.iter().any(|d| d.message == UNUSED_VALUE));
        }
        for source in ["_ = Output(1)".to_string(), format!("{def}_ = Display()")] {
            assert_eq!(errors(&source)[0].message, NOTHING_TO_DISCARD);
        }
        let diagnostics = errors("Output(Length(Output(0)))");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.column, 15);
    }

    #[test]
    fn underscore_is_exclusive_to_discard_statements() {
        for source in [
            "_ := 1",
            "int _ := 1",
            "Output(_)",
            "int x := _",
            "example = _",
            "_ = 1 + _",
            "_()",
            "(_)=1",
            "_[1] = 3",
            "import \"math\" as _",
            "function _() -|:\n\tOutput(1)",
            "function F(int _) -|:\n\tOutput(1)",
            "loop _ in list<int>{1}:\n\tbreak",
            "import \"math\" as m\n_ = m:_()",
            "Output(\"#{_}\")",
        ] {
            assert!(
                matches!(parse_source(source), Err(Failure::Syntax(ref d)) if d.message == DISCARD_POSITION),
                "{source}"
            );
        }
        assert!(matches!(
            parse_source("function = _"),
            Err(Failure::Syntax(_))
        ));
        assert!(check_source("int _item := 4\n_ = _item\n_ = _item").is_ok());
    }

    #[test]
    fn markers_and_returns_follow_the_function_contract() {
        for marker in ["- |", "-/* gap */|", "-| int", "-> -|"] {
            assert!(parse_source(&format!("function F() {marker}:\n\tOutput(1)")).is_err());
        }
        for source in [
            "-| x := 1",
            "function F(-| x) -|:\n\tOutput(x)",
            "_ = ls<-|>",
        ] {
            assert!(parse_source(source).is_err(), "{source}");
        }
        assert!(matches!(
            parse_source("function F() -|:\n\treturn"),
            Err(Failure::Syntax(_))
        ));
        let nested =
            "function F() -|:\n\tloop true:\n\t\tif false:\n\t\t\treturn none null\n\t\tbreak\nF()";
        assert!(errors(nested).iter().any(|d| d.message == NO_VALUE_RETURN));
        assert!(
            errors("function F() -> int:\n\tif true:\n\t\treturn int 1\n_ = F()")
                .iter()
                .any(|d| d.message.contains("every path"))
        );
    }

    #[test]
    fn unchecked_interpreter_distinguishes_no_value_and_null() {
        let source = "function F() -|:\n\t_ = 1\nfunction N() -> none:\n\treturn none null";
        let program = parse_source(source).ok().unwrap();
        let mut vm = Interpreter::new();
        vm.run(&program).unwrap();
        let span = Span::new(1, 1, 1);
        assert!(matches!(
            vm.call("F", &[], span).unwrap(),
            FunctionResult::NoValue
        ));
        assert!(matches!(
            vm.call("N", &[], span).unwrap(),
            FunctionResult::Value(Value::Null)
        ));
        for (usage, message) in [
            ("_ = F()", NOTHING_TO_DISCARD),
            ("Output(F())", VALUE_REQUIRED),
            ("N()", UNUSED_VALUE),
        ] {
            let program = parse_source(usage).ok().unwrap();
            assert_eq!(vm.run(&program).unwrap_err().message, message);
        }
    }
}
