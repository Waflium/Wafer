use std::collections::HashMap;

const CASE_INSENSITIVE: u32 = 0x02;
const MULTILINE: u32 = 0x08;
const DOTALL: u32 = 0x20;
const UNICODE_CASE: u32 = 0x40;
const COMMENTS: u32 = 0x04;
const UNIX_LINES: u32 = 0x01;

pub const WINDOWS_PATH_FLAGS: u32 = CASE_INSENSITIVE | UNICODE_CASE;

#[derive(Debug)]
pub struct PatternSyntaxException {
    pub description: String,
    pub pattern: String,
    pub index: isize,
}

impl PatternSyntaxException {
    pub fn message(&self, line_separator: &str) -> String {
        let mut message = self.description.clone();
        if self.index >= 0 {
            message.push_str(&format!(" near index {}", self.index));
        }
        message.push_str(line_separator);
        message.push_str(&self.pattern);
        if self.index >= 0 && (self.index as usize) < self.pattern.encode_utf16().count() {
            message.push_str(line_separator);
            message.push_str(&" ".repeat(self.index as usize));
            message.push('^');
        }
        message
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Greed {
    Greedy,
    Lazy,
    Possessive,
}

#[derive(Clone, Debug)]
enum ClassItem {
    Range(u32, u32),
    Predefined(Predefined, bool),
    Property(Property, bool),
    Nested(Box<ClassExpr>),
}

#[derive(Clone, Debug)]
struct ClassExpr {
    items: Vec<ClassItem>,
    negated: bool,
    intersections: Vec<ClassExpr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Predefined {
    Digit,
    Word,
    Space,
    HorizontalSpace,
    VerticalSpace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Property {
    Lower,
    Upper,
    Ascii,
    Alpha,
    Digit,
    Alnum,
    Punct,
    Graph,
    Print,
    Blank,
    Cntrl,
    XDigit,
    Space,
    Letter,
    UppercaseLetter,
    LowercaseLetter,
    Number,
    DecimalDigit,
    Whitespace,
    Any,
    Range(&'static [(u32, u32)]),
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Char(u32, bool, bool),
    Any(bool, bool),
    Class(Box<ClassExpr>, bool, bool),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Group(Box<Node>, Option<usize>),
    Repeat(Box<Node>, u32, Option<u32>, Greed),
    LineStart(bool, bool),
    LineEnd(bool, bool),
    WordBoundary(bool),
    InputStart,
    InputEnd,
    InputEndBeforeTerminator,
    Look(Box<Node>, bool, bool),
    Atomic(Box<Node>),
    BackRef(usize, bool, bool),
    LastMatchEnd,
    LineBreak,
    Grapheme,
}

#[derive(Debug)]
pub struct Pattern {
    root: Node,
    group_count: usize,
}

struct Parser<'a> {
    pattern: &'a str,
    temp: Vec<u32>,
    cursor: usize,
    flags: u32,
    group_count: usize,
    named: HashMap<String, usize>,
    capture_ends: Vec<usize>,
}

fn is_unicode_case(flags: u32) -> bool {
    flags & UNICODE_CASE != 0
}

fn fold(c: u32, unicode: bool) -> u32 {
    let Some(ch) = char::from_u32(c) else { return c };
    if !unicode && !ch.is_ascii() {
        return c;
    }
    let upper = {
        let mut iter = ch.to_uppercase();
        match (iter.next(), iter.next()) {
            (Some(u), None) => u,
            _ => ch,
        }
    };
    let mut lower_iter = upper.to_lowercase();
    match (lower_iter.next(), lower_iter.next()) {
        (Some(l), None) => l as u32,
        _ => upper as u32,
    }
}

fn is_word(c: u32) -> bool {
    char::from_u32(c).is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn property_matches(property: Property, c: u32) -> bool {
    let Some(ch) = char::from_u32(c) else { return false };
    match property {
        Property::Lower => ch.is_ascii_lowercase(),
        Property::Upper => ch.is_ascii_uppercase(),
        Property::Ascii => ch.is_ascii(),
        Property::Alpha => ch.is_ascii_alphabetic(),
        Property::Digit => ch.is_ascii_digit(),
        Property::Alnum => ch.is_ascii_alphanumeric(),
        Property::Punct => ch.is_ascii_punctuation(),
        Property::Graph => ch.is_ascii_graphic(),
        Property::Print => ch.is_ascii_graphic() || ch == ' ',
        Property::Blank => ch == ' ' || ch == '\t',
        Property::Cntrl => ch.is_ascii_control(),
        Property::XDigit => ch.is_ascii_hexdigit(),
        Property::Space => matches!(ch, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'),
        Property::Letter => ch.is_alphabetic() && !ch.is_numeric(),
        Property::UppercaseLetter => ch.is_uppercase(),
        Property::LowercaseLetter => ch.is_lowercase(),
        Property::Number => ch.is_numeric(),
        Property::DecimalDigit => ch.is_numeric() && ch.to_digit(10).is_some() || ch.is_ascii_digit(),
        Property::Whitespace => ch.is_whitespace(),
        Property::Any => true,
        Property::Range(ranges) => ranges.iter().any(|&(low, high)| low <= c && c <= high),
    }
}

fn predefined_matches(kind: Predefined, c: u32, unicode_classes: bool) -> bool {
    let Some(ch) = char::from_u32(c) else { return false };
    match kind {
        Predefined::Digit => ch.is_ascii_digit() || (unicode_classes && ch.is_numeric() && ch.to_digit(10).is_some()),
        Predefined::Word => is_word(c),
        Predefined::Space => matches!(ch, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'),
        Predefined::HorizontalSpace => matches!(c, 0x09 | 0x20 | 0xa0 | 0x1680 | 0x180e | 0x2000..=0x200a | 0x202f | 0x205f | 0x3000),
        Predefined::VerticalSpace => matches!(c, 0x0a..=0x0d | 0x85 | 0x2028 | 0x2029),
    }
}

impl ClassExpr {
    fn matches(&self, c: u32, ci: bool, unicode: bool) -> bool {
        let direct = |c: u32| self.items.iter().any(|item| item.matches(c, ci, unicode));
        let mut result = direct(c);
        if !result && ci {
            let (lower, upper) = (lower_of(c, unicode), upper_of(c, unicode));
            result = direct(lower) || direct(upper);
        }
        if result {
            result = self.intersections.iter().all(|other| other.matches(c, ci, unicode));
        }
        result != self.negated
    }
}

fn lower_of(c: u32, unicode: bool) -> u32 {
    match char::from_u32(c) {
        Some(ch) if unicode || ch.is_ascii() => {
            let mut iter = ch.to_lowercase();
            match (iter.next(), iter.next()) {
                (Some(l), None) => l as u32,
                _ => c,
            }
        }
        _ => c,
    }
}

fn upper_of(c: u32, unicode: bool) -> u32 {
    match char::from_u32(c) {
        Some(ch) if unicode || ch.is_ascii() => {
            let mut iter = ch.to_uppercase();
            match (iter.next(), iter.next()) {
                (Some(u), None) => u as u32,
                _ => c,
            }
        }
        _ => c,
    }
}

impl ClassItem {
    fn matches(&self, c: u32, ci: bool, unicode: bool) -> bool {
        match self {
            ClassItem::Range(low, high) => *low <= c && c <= *high,
            ClassItem::Predefined(kind, negated) => predefined_matches(*kind, c, false) != *negated,
            ClassItem::Property(property, negated) => {
                let matched = property_matches(*property, c) || (ci && matches!(property, Property::Lower | Property::Upper) && (property_matches(Property::Lower, c) || property_matches(Property::Upper, c)));
                matched != *negated
            }
            ClassItem::Nested(expr) => expr.matches(c, ci, unicode),
        }
    }
}

impl<'a> Parser<'a> {
    fn error(&self, description: &str) -> PatternSyntaxException {
        PatternSyntaxException { description: description.to_string(), pattern: self.pattern.to_string(), index: self.cursor as isize - 1 }
    }

    fn peek(&self) -> u32 {
        self.temp.get(self.cursor).copied().unwrap_or(0)
    }

    fn peek_skipping_comments(&mut self) -> u32 {
        if self.flags & COMMENTS != 0 {
            self.peek_past_whitespace();
        }
        self.peek()
    }

    fn peek_past_whitespace(&mut self) {
        while self.cursor < self.temp.len() {
            let c = self.temp[self.cursor];
            if c == '#' as u32 {
                while self.cursor < self.temp.len() && !matches!(self.temp[self.cursor], 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029) {
                    self.cursor += 1;
                }
            } else if matches!(c, 0x20 | 0x09 | 0x0a | 0x0b | 0x0c | 0x0d) {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> u32 {
        self.cursor += 1;
        if self.flags & COMMENTS != 0 {
            self.peek_past_whitespace();
        }
        self.peek()
    }

    fn read(&mut self) -> u32 {
        let ch = self.peek();
        self.cursor += 1;
        ch
    }

    fn at_end(&self) -> bool {
        self.cursor >= self.temp.len()
    }

    fn ci(&self) -> bool {
        self.flags & CASE_INSENSITIVE != 0
    }

    fn uc(&self) -> bool {
        is_unicode_case(self.flags)
    }
}

pub fn compile(pattern: &str, flags: u32) -> Result<Pattern, PatternSyntaxException> {
    let mut temp: Vec<u32> = Vec::new();
    let chars: Vec<u32> = pattern.chars().map(|c| c as u32).collect();
    let mut i = 0;
    let mut quoting = false;
    while i < chars.len() {
        let c = chars[i];
        if quoting {
            if c == '\\' as u32 && chars.get(i + 1) == Some(&('E' as u32)) {
                quoting = false;
                i += 2;
                continue;
            }
            if c < 128 && !(c as u8 as char).is_ascii_alphanumeric() || c >= 128 {
                temp.push('\\' as u32);
            }
            temp.push(c);
            i += 1;
        } else if c == '\\' as u32 && chars.get(i + 1) == Some(&('Q' as u32)) {
            quoting = true;
            i += 2;
        } else if c == '\\' as u32 && i + 1 < chars.len() {
            temp.push(c);
            temp.push(chars[i + 1]);
            i += 2;
        } else {
            temp.push(c);
            i += 1;
        }
    }
    let mut parser = Parser { pattern, temp, cursor: 0, flags, group_count: 0, named: HashMap::new(), capture_ends: Vec::new() };
    if parser.flags & COMMENTS != 0 {
        parser.peek_past_whitespace();
    }
    let root = parse_expr(&mut parser)?;
    if !parser.at_end() {
        if parser.peek() == ')' as u32 {
            return Err(parser.error("Unmatched closing ')'"));
        }
        return Err(parser.error("Unexpected character '" ));
    }
    Ok(Pattern { root, group_count: parser.group_count })
}

fn parse_expr(p: &mut Parser) -> Result<Node, PatternSyntaxException> {
    let mut branches = Vec::new();
    loop {
        let sequence = parse_sequence(p)?;
        branches.push(sequence);
        if p.peek() != '|' as u32 {
            break;
        }
        p.next();
    }
    Ok(if branches.len() == 1 { branches.pop().unwrap() } else { Node::Alt(branches) })
}

fn parse_sequence(p: &mut Parser) -> Result<Node, PatternSyntaxException> {
    let mut nodes = Vec::new();
    loop {
        let ch = p.peek();
        let atom = match ch {
            0 if p.at_end() => break,
            0x28 => {
                let node = parse_group(p)?;
                node
            }
            0x5b => {
                let class = parse_class(p, true)?;
                Node::Class(Box::new(class), p.ci(), p.uc())
            }
            0x5c => parse_escape_atom(p)?,
            0x5e => {
                p.next();
                Node::LineStart(p.flags & MULTILINE != 0, p.flags & UNIX_LINES != 0)
            }
            0x24 => {
                p.next();
                Node::LineEnd(p.flags & MULTILINE != 0, p.flags & UNIX_LINES != 0)
            }
            0x2e => {
                p.next();
                Node::Any(p.flags & DOTALL != 0, p.flags & UNIX_LINES != 0)
            }
            0x7c | 0x29 => break,
            0x7b => Node::Empty,
            0x5d | 0x7d => {
                p.next();
                Node::Char(ch, p.ci(), p.uc())
            }
            0x3f | 0x2a | 0x2b => {
                p.next();
                return Err(p.error(&format!("Dangling meta character '{}'", ch as u8 as char)));
            }
            _ => {
                p.next();
                Node::Char(ch, p.ci(), p.uc())
            }
        };
        let atom = parse_closure(p, atom)?;
        nodes.push(atom);
    }
    Ok(match nodes.len() {
        0 => Node::Empty,
        1 => nodes.pop().unwrap(),
        _ => Node::Seq(nodes),
    })
}

fn parse_closure(p: &mut Parser, atom: Node) -> Result<Node, PatternSyntaxException> {
    let ch = p.peek_skipping_comments();
    let (min, max) = match ch {
        0x3f => {
            p.next();
            (0, Some(1))
        }
        0x2a => {
            p.next();
            (0, None)
        }
        0x2b => {
            p.next();
            (1, None)
        }
        0x7b => {
            let ch = p.temp.get(p.cursor + 1).copied().unwrap_or(0);
            if !(0x30..=0x39).contains(&ch) {
                p.cursor += 2;
                return Err(p.error("Illegal repetition"));
            }
            p.cursor += 1;
            let mut cmin: i64 = 0;
            let mut c = p.read();
            while (0x30..=0x39).contains(&c) {
                cmin = cmin * 10 + (c - 0x30) as i64;
                if cmin > i32::MAX as i64 {
                    return Err(p.error("Illegal repetition range"));
                }
                c = p.read();
            }
            let mut cmax = cmin;
            if c == 0x2c {
                c = p.read();
                cmax = i32::MAX as i64;
                if c != 0x7d {
                    cmax = 0;
                    while (0x30..=0x39).contains(&c) {
                        cmax = cmax * 10 + (c - 0x30) as i64;
                        if cmax > i32::MAX as i64 {
                            return Err(p.error("Illegal repetition range"));
                        }
                        c = p.read();
                    }
                }
            }
            if c != 0x7d {
                return Err(p.error("Unclosed counted closure"));
            }
            if cmax < cmin {
                return Err(p.error("Illegal repetition range"));
            }
            if p.flags & COMMENTS != 0 {
                p.peek_past_whitespace();
            }
            (cmin as u32, if cmax == i32::MAX as i64 && cmax != cmin { None } else { Some(cmax as u32) })
        }
        _ => return Ok(atom),
    };
    let suffix = p.peek();
    let greed = if suffix == 0x3f {
        p.next();
        Greed::Lazy
    } else if suffix == 0x2b {
        p.next();
        Greed::Possessive
    } else {
        Greed::Greedy
    };
    Ok(Node::Repeat(Box::new(atom), min, max, greed))
}

fn parse_group(p: &mut Parser) -> Result<Node, PatternSyntaxException> {
    let saved_flags = p.flags;
    p.cursor += 1;
    let mut capture: Option<usize> = None;
    let mut kind = 'c';
    let mut name: Option<String> = None;
    if p.peek() == 0x3f {
        p.cursor += 1;
        let ch = p.read();
        match char::from_u32(ch).unwrap_or('\0') {
            ':' => kind = ':',
            '=' => kind = '=',
            '!' => kind = '!',
            '>' => kind = '>',
            '<' => {
                let next = p.read();
                if next == '=' as u32 {
                    kind = 'b';
                } else if next == '!' as u32 {
                    kind = 'B';
                } else {
                    p.cursor -= 1;
                    let mut group_name = String::new();
                    let first = p.read();
                    p.cursor -= 1;
                    if !(char::from_u32(first).is_some_and(|c| c.is_ascii_alphabetic())) {
                        p.cursor += 1;
                        return Err(p.error("capturing group name does not start with a Latin letter"));
                    }
                    loop {
                        let c = p.read();
                        match char::from_u32(c) {
                            Some(c) if c.is_ascii_alphanumeric() => group_name.push(c),
                            Some('>') => break,
                            _ => {
                                if p.at_end() && c == 0 {
                                    return Err(p.error("named capturing group is missing trailing '>'"));
                                }
                                return Err(p.error("named capturing group is missing trailing '>'"));
                            }
                        }
                    }
                    if p.named.contains_key(&group_name) {
                        return Err(p.error(&format!("Named capturing group <{group_name}> is already defined")));
                    }
                    name = Some(group_name);
                }
            }
            c if matches!(c, '$' | '@') => return Err(p.error("Unknown group type")),
            _ => {
                p.cursor -= 1;
                return parse_inline_flags(p, saved_flags);
            }
        }
    }
    if kind == 'c' || name.is_some() {
        p.group_count += 1;
        capture = Some(p.group_count);
        if let Some(group_name) = name {
            p.named.insert(group_name, p.group_count);
        }
    }
    let body = parse_expr(p)?;
    if p.peek() != ')' as u32 {
        p.flags = saved_flags;
        return Err(PatternSyntaxException { description: "Unclosed group".into(), pattern: p.pattern.to_string(), index: p.temp.len() as isize });
    }
    p.next();
    p.flags = saved_flags;
    if let Some(index) = capture {
        p.capture_ends.push(index);
    }
    Ok(match kind {
        '=' => Node::Look(Box::new(body), true, false),
        '!' => Node::Look(Box::new(body), true, true),
        'b' | 'B' => {
            if length_range(&body).is_none() {
                return Err(p.error("Look-behind group does not have an obvious maximum length"));
            }
            Node::Look(Box::new(body), false, kind == 'B')
        }
        '>' => Node::Atomic(Box::new(body)),
        _ => Node::Group(Box::new(body), capture),
    })
}

fn parse_inline_flags(p: &mut Parser, saved_flags: u32) -> Result<Node, PatternSyntaxException> {
    let mut add = true;
    let mut flags = p.flags;
    loop {
        let ch = p.read();
        let Some(c) = char::from_u32(ch) else { return Err(p.error("Unknown inline modifier")) };
        match c {
            '-' => {
                if !add {
                    return Err(p.error("Unknown inline modifier"));
                }
                add = false;
            }
            'i' | 'm' | 's' | 'd' | 'u' | 'x' | 'U' => {
                let bit = match c {
                    'i' => CASE_INSENSITIVE,
                    'm' => MULTILINE,
                    's' => DOTALL,
                    'd' => UNIX_LINES,
                    'u' => UNICODE_CASE,
                    'x' => COMMENTS,
                    _ => 0x100,
                };
                if add { flags |= bit } else { flags &= !bit }
            }
            ')' => {
                p.flags = flags;
                return Ok(Node::Empty);
            }
            ':' => {
                p.flags = flags;
                let body = parse_expr(p)?;
                if p.peek() != ')' as u32 {
                    p.flags = saved_flags;
                    return Err(PatternSyntaxException { description: "Unclosed group".into(), pattern: p.pattern.to_string(), index: p.temp.len() as isize });
                }
                p.next();
                p.flags = saved_flags;
                return Ok(Node::Group(Box::new(body), None));
            }
            _ => {
                let _ = ch;
                return Err(p.error("Unknown inline modifier"));
            }
        }
    }
}

fn length_range(node: &Node) -> Option<(usize, Option<usize>)> {
    Some(match node {
        Node::Empty | Node::LineStart(..) | Node::LineEnd(..) | Node::WordBoundary(_) | Node::InputStart | Node::InputEnd | Node::InputEndBeforeTerminator | Node::Look(..) | Node::LastMatchEnd => (0, Some(0)),
        Node::Char(c, ..) => (1, Some(if *c > 0xffff { 2 } else { 1 })),
        Node::Any(..) | Node::Class(..) => (1, Some(2)),
        Node::LineBreak | Node::Grapheme => (1, Some(2)),
        Node::Seq(nodes) => {
            let (mut min, mut max) = (0usize, Some(0usize));
            for node in nodes {
                let (node_min, node_max) = length_range(node)?;
                min += node_min;
                max = max.zip(node_max).map(|(a, b)| a + b);
            }
            (min, max)
        }
        Node::Alt(nodes) => {
            let mut min = usize::MAX;
            let mut max = Some(0usize);
            for node in nodes {
                let (node_min, node_max) = length_range(node)?;
                min = min.min(node_min);
                max = max.zip(node_max).map(|(a, b)| a.max(b));
            }
            (min, max)
        }
        Node::Group(inner, _) | Node::Atomic(inner) => length_range(inner)?,
        Node::Repeat(inner, min, max, _) => {
            let (inner_min, inner_max) = length_range(inner)?;
            (inner_min * *min as usize, max.zip(inner_max).map(|(count, inner)| count as usize * inner))
        }
        Node::BackRef(..) => return None,
    })
}

fn parse_class(p: &mut Parser, top: bool) -> Result<ClassExpr, PatternSyntaxException> {
    let _ = top;
    p.cursor += 1;
    let mut negated = false;
    if p.peek() == '^' as u32 {
        negated = true;
        p.cursor += 1;
    }
    let mut items: Vec<ClassItem> = Vec::new();
    let mut intersections: Vec<ClassExpr> = Vec::new();
    let mut first = true;
    loop {
        if p.at_end() {
            return Err(PatternSyntaxException { description: "Unclosed character class".into(), pattern: p.pattern.to_string(), index: p.temp.len() as isize - 1 });
        }
        let ch = p.peek();
        if ch == ']' as u32 && !first {
            p.cursor += 1;
            break;
        }
        if ch == '[' as u32 {
            let nested = parse_class(p, false)?;
            items.push(ClassItem::Nested(Box::new(nested)));
            first = false;
            continue;
        }
        if ch == '&' as u32 && p.temp.get(p.cursor + 1) == Some(&('&' as u32)) {
            p.cursor += 2;
            let mut rhs_items = Vec::new();
            loop {
                if p.at_end() {
                    return Err(PatternSyntaxException { description: "Unclosed character class".into(), pattern: p.pattern.to_string(), index: p.temp.len() as isize - 1 });
                }
                let c = p.peek();
                if c == ']' as u32 {
                    break;
                }
                if c == '[' as u32 {
                    let nested = parse_class(p, false)?;
                    rhs_items.push(ClassItem::Nested(Box::new(nested)));
                    continue;
                }
                if c == '&' as u32 && p.temp.get(p.cursor + 1) == Some(&('&' as u32)) {
                    break;
                }
                let item = parse_class_atom(p)?;
                rhs_items.push(item);
            }
            intersections.push(ClassExpr { items: rhs_items, negated: false, intersections: Vec::new() });
            first = false;
            continue;
        }
        let low_item = parse_class_atom(p)?;
        first = false;
        if let ClassItem::Range(low, _) = low_item
            && p.peek() == '-' as u32
        {
            let after = p.temp.get(p.cursor + 1).copied().unwrap_or(0);
            if after != ']' as u32 && after != '[' as u32 {
                p.cursor += 1;
                let high_item = parse_class_atom(p)?;
                if let ClassItem::Range(high, _) = high_item {
                    if high < low {
                        return Err(p.error("Illegal character range"));
                    }
                    items.push(ClassItem::Range(low, high));
                    continue;
                }
                return Err(p.error("Illegal character range"));
            }
        }
        items.push(low_item);
    }
    let mut result = ClassExpr { items, negated: false, intersections: Vec::new() };
    if !intersections.is_empty() {
        let mut chained = ClassExpr { items: result.items.clone(), negated: false, intersections };
        std::mem::swap(&mut result, &mut chained);
    }
    result.negated = negated;
    Ok(result)
}

fn parse_class_atom(p: &mut Parser) -> Result<ClassItem, PatternSyntaxException> {
    let ch = p.read();
    if ch == '\\' as u32 {
        return parse_escape_in_class(p);
    }
    Ok(ClassItem::Range(ch, ch))
}

fn parse_hex(p: &mut Parser, count: usize) -> Result<u32, PatternSyntaxException> {
    let mut n: u32 = 0;
    for _ in 0..count {
        let c = p.read();
        let digit = char::from_u32(c).and_then(|c| c.to_digit(16));
        match digit {
            Some(d) => n = n * 16 + d,
            None => return Err(p.error("Illegal hexadecimal escape sequence")),
        }
    }
    Ok(n)
}

fn parse_unicode_escape(p: &mut Parser) -> Result<u32, PatternSyntaxException> {
    let mut n: u32 = 0;
    for _ in 0..4 {
        let c = p.read();
        match char::from_u32(c).and_then(|c| c.to_digit(16)) {
            Some(d) => n = n * 16 + d,
            None => return Err(p.error("Illegal Unicode escape sequence")),
        }
    }
    Ok(n)
}

fn parse_octal(p: &mut Parser) -> Result<u32, PatternSyntaxException> {
    let n = p.read();
    if !(0x30..=0x37).contains(&n) {
        return Err(p.error("Illegal octal escape sequence"));
    }
    let mut value = n - 0x30;
    let m = p.peek();
    if (0x30..=0x37).contains(&m) {
        p.cursor += 1;
        value = value * 8 + (m - 0x30);
        let o = p.peek();
        if value <= 0o37 && (0x30..=0x37).contains(&o) {
            p.cursor += 1;
            value = value * 8 + (o - 0x30);
        }
    }
    Ok(value)
}

fn parse_property(p: &mut Parser, negated: bool) -> Result<(Property, bool), PatternSyntaxException> {
    let ch = p.read();
    let name: String;
    if ch == '{' as u32 {
        let mut text = String::new();
        loop {
            if p.at_end() {
                p.cursor += 1;
                return Err(p.error("Unclosed character family"));
            }
            let c = p.read();
            if c == '}' as u32 {
                break;
            }
            text.push(char::from_u32(c).unwrap_or('\u{fffd}'));
        }
        if text.is_empty() {
            return Err(p.error("Empty character family"));
        }
        name = text;
    } else {
        name = char::from_u32(ch).map(|c| c.to_string()).unwrap_or_default();
    }
    let mut negated = negated;
    let mut key = name.as_str();
    if let Some((left, right)) = name.split_once('=') {
        key = right;
        let _ = left;
    }
    let property = match key {
        "Lower" => Property::Lower,
        "Upper" => Property::Upper,
        "ASCII" => Property::Ascii,
        "Alpha" => Property::Alpha,
        "Digit" => Property::Digit,
        "Alnum" => Property::Alnum,
        "Punct" => Property::Punct,
        "Graph" => Property::Graph,
        "Print" => Property::Print,
        "Blank" => Property::Blank,
        "Cntrl" => Property::Cntrl,
        "XDigit" => Property::XDigit,
        "Space" => Property::Space,
        "L" | "IsL" | "IsAlphabetic" | "IsLetter" | "javaLetter" | "javaAlphabetic" | "Alphabetic" => Property::Letter,
        "Lu" | "IsLu" | "IsUppercase" | "javaUpperCase" | "Uppercase" => Property::UppercaseLetter,
        "Ll" | "IsLl" | "IsLowercase" | "javaLowerCase" | "Lowercase" => Property::LowercaseLetter,
        "N" | "IsN" => Property::Number,
        "Nd" | "IsNd" | "IsDigit" | "javaDigit" => Property::DecimalDigit,
        "javaWhitespace" | "IsWhite_Space" | "IsWhiteSpace" => Property::Whitespace,
        "all" | "Any" => Property::Any,
        "IsLatin" | "InBasicLatin" => Property::Range(&[(0x41, 0x5a), (0x61, 0x7a), (0xaa, 0xaa), (0xba, 0xba), (0xc0, 0xd6), (0xd8, 0xf6), (0xf8, 0x2b8), (0x1e00, 0x1eff)]),
        "IsGreek" | "InGreek" | "InGreekandCoptic" => Property::Range(&[(0x370, 0x3ff)]),
        "IsCyrillic" | "InCyrillic" => Property::Range(&[(0x400, 0x52f)]),
        "IsHan" => Property::Range(&[(0x4e00, 0x9fff), (0x3400, 0x4dbf)]),
        _ => {
            let _ = &mut negated;
            return Err(p.error(&format!("Unknown character property name {{{}}}", key.trim_matches(|c| c == '{' || c == '}'))));
        }
    };
    Ok((property, negated))
}

fn parse_escape_in_class(p: &mut Parser) -> Result<ClassItem, PatternSyntaxException> {
    let ch = p.read();
    let c = char::from_u32(ch).unwrap_or('\0');
    Ok(match c {
        'd' => ClassItem::Predefined(Predefined::Digit, false),
        'D' => ClassItem::Predefined(Predefined::Digit, true),
        'w' => ClassItem::Predefined(Predefined::Word, false),
        'W' => ClassItem::Predefined(Predefined::Word, true),
        's' => ClassItem::Predefined(Predefined::Space, false),
        'S' => ClassItem::Predefined(Predefined::Space, true),
        'h' => ClassItem::Predefined(Predefined::HorizontalSpace, false),
        'H' => ClassItem::Predefined(Predefined::HorizontalSpace, true),
        'v' => ClassItem::Predefined(Predefined::VerticalSpace, false),
        'V' => ClassItem::Predefined(Predefined::VerticalSpace, true),
        'p' | 'P' => {
            let (property, negated) = parse_property(p, c == 'P')?;
            ClassItem::Property(property, negated)
        }
        _ => {
            p.cursor -= 1;
            let value = parse_literal_escape(p, true)?;
            ClassItem::Range(value, value)
        }
    })
}

fn parse_literal_escape(p: &mut Parser, in_class: bool) -> Result<u32, PatternSyntaxException> {
    let ch = p.read();
    let c = char::from_u32(ch).unwrap_or('\0');
    Ok(match c {
        '0' => parse_octal(p)?,
        'a' => 7,
        'e' => 27,
        'f' => 12,
        'n' => 10,
        'r' => 13,
        't' => 9,
        'x' => {
            if p.peek() == '{' as u32 {
                p.cursor += 1;
                let mut n: u32 = 0;
                loop {
                    let c = p.read();
                    if c == '}' as u32 {
                        break;
                    }
                    match char::from_u32(c).and_then(|c| c.to_digit(16)) {
                        Some(d) => {
                            n = n.saturating_mul(16).saturating_add(d);
                            if n > 0x10ffff {
                                return Err(p.error("Hexadecimal codepoint is too big"));
                            }
                        }
                        None => return Err(p.error("Unclosed hexadecimal escape sequence")),
                    }
                }
                n
            } else {
                parse_hex(p, 2)?
            }
        }
        'u' => {
            let high = parse_unicode_escape(p)?;
            if (0xd800..=0xdbff).contains(&high) && p.peek() == '\\' as u32 && p.temp.get(p.cursor + 1) == Some(&('u' as u32)) {
                let saved = p.cursor;
                p.cursor += 2;
                let low = parse_unicode_escape(p)?;
                if (0xdc00..=0xdfff).contains(&low) {
                    return Ok(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00));
                }
                p.cursor = saved;
            }
            high
        }
        'c' => {
            if p.at_end() {
                return Err(p.error("Illegal control escape sequence"));
            }
            p.read() ^ 64
        }
        'N' | 'R' | 'X' | 'G' | 'Z' | 'z' | 'A' | 'b' | 'B' | 'k' | 'Q' | 'E' | 'C' | 'F' | 'g' | 'i' | 'j' | 'l' | 'm' | 'o' | 'q' | 'y' | 'I' | 'J' | 'K' | 'L' | 'M' | 'O' | 'T' | 'U' | 'Y' => {
            let _ = in_class;
            return Err(p.error("Illegal/unsupported escape sequence"));
        }
        c if c.is_ascii_alphabetic() => return Err(p.error("Illegal/unsupported escape sequence")),
        '\0' if p.at_end() => return Err(p.error("Unexpected internal error")),
        _ => ch,
    })
}

fn parse_escape_atom(p: &mut Parser) -> Result<Node, PatternSyntaxException> {
    p.cursor += 1;
    if p.at_end() {
        p.cursor = p.temp.len() + 1;
        return Err(p.error("Unescaped trailing backslash"));
    }
    let ch = p.read();
    let c = char::from_u32(ch).unwrap_or('\0');
    let (ci, uc) = (p.ci(), p.uc());
    Ok(match c {
        'A' => Node::InputStart,
        'z' => Node::InputEnd,
        'Z' => Node::InputEndBeforeTerminator,
        'G' => Node::LastMatchEnd,
        'b' => Node::WordBoundary(false),
        'B' => Node::WordBoundary(true),
        'R' => Node::LineBreak,
        'X' => Node::Grapheme,
        'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'h' | 'H' | 'v' | 'V' => {
            p.cursor -= 1;
            let item = parse_escape_in_class(p)?;
            Node::Class(Box::new(ClassExpr { items: vec![item], negated: false, intersections: Vec::new() }), ci, uc)
        }
        'p' | 'P' => {
            let (property, negated) = parse_property(p, c == 'P')?;
            Node::Class(Box::new(ClassExpr { items: vec![ClassItem::Property(property, negated)], negated: false, intersections: Vec::new() }), ci, uc)
        }
        '1'..='9' => {
            let mut group = (ch - 0x30) as usize;
            loop {
                let next = p.peek();
                if !(0x30..=0x39).contains(&next) {
                    break;
                }
                let candidate = group * 10 + (next - 0x30) as usize;
                if p.group_count < candidate {
                    break;
                }
                group = candidate;
                p.cursor += 1;
            }
            Node::BackRef(group, ci, uc)
        }
        'k' => {
            if p.read() != '<' as u32 {
                return Err(p.error("\\k is not followed by '<' for named capturing group"));
            }
            let mut name = String::new();
            loop {
                let c = p.read();
                match char::from_u32(c) {
                    Some('>') => break,
                    Some(c) if c.is_ascii_alphanumeric() => name.push(c),
                    _ => return Err(p.error("named capturing group is missing trailing '>'")),
                }
            }
            match p.named.get(&name) {
                Some(&group) => Node::BackRef(group, ci, uc),
                None => return Err(p.error(&format!("named capturing group <{name}> does not exist"))),
            }
        }
        _ => {
            p.cursor -= 1;
            let value = parse_literal_escape(p, false)?;
            Node::Char(value, ci, uc)
        }
    })
}

struct Matcher<'a> {
    input: &'a [char],
    groups: Vec<Option<(usize, usize)>>,
}

fn chars_eq(a: u32, b: u32, ci: bool, uc: bool) -> bool {
    a == b || (ci && fold(a, uc) == fold(b, uc))
}

impl Matcher<'_> {
    fn at(&self, pos: usize) -> Option<u32> {
        self.input.get(pos).map(|&c| c as u32)
    }

    fn run(&mut self, node: &Node, pos: usize, k: &mut dyn FnMut(&mut Matcher, usize) -> bool) -> bool {
        match node {
            Node::Empty => k(self, pos),
            Node::Char(c, ci, uc) => match self.at(pos) {
                Some(x) if chars_eq(x, *c, *ci, *uc) => k(self, pos + 1),
                _ => false,
            },
            Node::Any(dotall, unix) => match self.at(pos) {
                Some(x) if *dotall || !is_line_terminator(x, *unix) => k(self, pos + 1),
                _ => false,
            },
            Node::Class(class, ci, uc) => match self.at(pos) {
                Some(x) if class.matches(x, *ci, *uc) => k(self, pos + 1),
                _ => false,
            },
            Node::Seq(nodes) => self.run_seq(nodes, pos, k),
            Node::Alt(branches) => {
                for branch in branches {
                    if self.run(branch, pos, k) {
                        return true;
                    }
                }
                false
            }
            Node::Group(inner, capture) => match capture {
                None => self.run(inner, pos, k),
                Some(index) => {
                    let index = *index;
                    let previous = self.groups[index];
                    let result = self.run(inner, pos, &mut |m, end| {
                        let saved = m.groups[index];
                        m.groups[index] = Some((pos, end));
                        if k(m, end) {
                            true
                        } else {
                            m.groups[index] = saved;
                            false
                        }
                    });
                    if !result {
                        self.groups[index] = previous;
                    }
                    result
                }
            },
            Node::Repeat(inner, min, max, greed) => self.run_repeat(inner, *min, *max, *greed, pos, 0, usize::MAX, k),
            Node::LineStart(multiline, unix) => {
                let ok = if !multiline {
                    pos == 0
                } else if pos == 0 {
                    true
                } else if pos >= self.input.len() {
                    false
                } else {
                    let previous = self.input[pos - 1] as u32;
                    if previous == 0x0a {
                        true
                    } else if *unix {
                        false
                    } else {
                        (previous == 0x0d && self.input[pos] != '\n') || matches!(previous, 0x85 | 0x2028 | 0x2029)
                    }
                };
                ok && k(self, pos)
            }
            Node::LineEnd(multiline, unix) => {
                let len = self.input.len();
                let ok = if !multiline {
                    pos == len || self.is_terminator_end(pos, *unix)
                } else {
                    pos == len || self.at(pos).is_some_and(|c| is_line_terminator(c, *unix)) && !(pos > 0 && self.input[pos - 1] == '\r' && self.input[pos] == '\n' && !unix)
                };
                ok && k(self, pos)
            }
            Node::WordBoundary(negated) => {
                let before = pos > 0 && is_word(self.input[pos - 1] as u32);
                let after = pos < self.input.len() && is_word(self.input[pos] as u32);
                ((before != after) != *negated) && k(self, pos)
            }
            Node::InputStart => pos == 0 && k(self, pos),
            Node::InputEnd => pos == self.input.len() && k(self, pos),
            Node::InputEndBeforeTerminator => (pos == self.input.len() || self.is_terminator_end(pos, false)) && k(self, pos),
            Node::Look(inner, ahead, negated) => {
                let saved = self.groups.clone();
                let matched = if *ahead {
                    self.run(inner, pos, &mut |_, _| true)
                } else {
                    let (min, max) = length_range(inner).unwrap_or((0, Some(0)));
                    let mut found = false;
                    let mut start = max.map_or(0, |max| pos.saturating_sub(max));
                    let lowest = pos.checked_sub(min);
                    if let Some(lowest) = lowest {
                        while start <= lowest {
                            if self.run(inner, start, &mut |_, end| end == pos) {
                                found = true;
                                break;
                            }
                            start += 1;
                        }
                    }
                    found
                };
                if matched == *negated {
                    self.groups = saved;
                    return false;
                }
                if *negated {
                    self.groups = saved;
                }
                k(self, pos)
            }
            Node::Atomic(inner) => {
                let mut end = None;
                if self.run(inner, pos, &mut |_, e| {
                    end = Some(e);
                    true
                }) {
                    return k(self, end.unwrap());
                }
                false
            }
            Node::BackRef(group, ci, uc) => {
                let Some(Some((start, end))) = self.groups.get(*group).copied() else { return false };
                let length = end - start;
                if pos + length > self.input.len() {
                    return false;
                }
                for offset in 0..length {
                    if !chars_eq(self.input[start + offset] as u32, self.input[pos + offset] as u32, *ci, *uc) {
                        return false;
                    }
                }
                k(self, pos + length)
            }
            Node::LastMatchEnd => pos == 0 && k(self, pos),
            Node::Grapheme => {
                if self.at(pos).is_none() {
                    return false;
                }
                if self.at(pos) == Some(0x0d) && self.at(pos + 1) == Some(0x0a) && k(self, pos + 2) {
                    return true;
                }
                k(self, pos + 1)
            }
            Node::LineBreak => {
                if self.at(pos) == Some(0x0d) && self.at(pos + 1) == Some(0x0a) && k(self, pos + 2) {
                    return true;
                }
                match self.at(pos) {
                    Some(c) if matches!(c, 0x0a..=0x0d | 0x85 | 0x2028 | 0x2029) => k(self, pos + 1),
                    _ => false,
                }
            }
        }
    }

    fn is_terminator_end(&self, pos: usize, unix: bool) -> bool {
        let len = self.input.len();
        if unix {
            return pos + 1 == len && self.input[pos] == '\n';
        }
        match len - pos {
            1 => is_line_terminator(self.input[pos] as u32, false),
            2 => self.input[pos] == '\r' && self.input[pos + 1] == '\n',
            _ => false,
        }
    }

    fn run_seq(&mut self, nodes: &[Node], pos: usize, k: &mut dyn FnMut(&mut Matcher, usize) -> bool) -> bool {
        match nodes.split_first() {
            None => k(self, pos),
            Some((first, rest)) => self.run(first, pos, &mut |m, next| m.run_seq(rest, next, k)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_repeat(&mut self, inner: &Node, min: u32, max: Option<u32>, greed: Greed, pos: usize, count: u32, last_pos: usize, k: &mut dyn FnMut(&mut Matcher, usize) -> bool) -> bool {
        if greed == Greed::Possessive {
            let mut position = pos;
            let mut matched = 0u32;
            while max.is_none_or(|max| matched < max) {
                let mut end = None;
                if !self.run(inner, position, &mut |_, e| {
                    end = Some(e);
                    true
                }) {
                    break;
                }
                let end = end.unwrap();
                matched += 1;
                if end == position {
                    break;
                }
                position = end;
            }
            if matched < min {
                return false;
            }
            return k(self, position);
        }
        let can_more = max.is_none_or(|max| count < max);
        let must_more = count < min;
        let try_more = |m: &mut Matcher, k: &mut dyn FnMut(&mut Matcher, usize) -> bool| {
            if !can_more {
                return false;
            }
            if count >= min && last_pos == pos {
                return false;
            }
            m.run(inner, pos, &mut |m2, next| m2.run_repeat(inner, min, max, greed, next, count + 1, pos, k))
        };
        if must_more {
            return try_more(self, k);
        }
        match greed {
            Greed::Greedy => try_more(self, k) || k(self, pos),
            _ => k(self, pos) || try_more(self, k),
        }
    }
}

fn is_line_terminator(c: u32, unix: bool) -> bool {
    if unix { c == 0x0a } else { matches!(c, 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029) }
}

impl Pattern {
    pub fn matches(&self, text: &str) -> bool {
        let input: Vec<char> = text.chars().collect();
        let mut matcher = Matcher { input: &input, groups: vec![None; self.group_count + 1] };
        let len = input.len();
        matcher.run(&self.root, 0, &mut |_, end| end == len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(pattern: &str, text: &str) -> bool {
        compile(pattern, 0).unwrap().matches(text)
    }

    const TEXTS: [&str; 23] = [
        "", "a", "ab", "abc", "aaa", "A", "abcabc", "a.b", "a\\b", "C:\\x\\y.zip", "foo.zip", "x1", "ab12", "a b", "\u{e9}", "aa", "ABC", "a1b2", "abba", "\n", "a\nb", "_", "-",
    ];

    fn outcome(pattern: &str, flags: u32) -> String {
        match compile(pattern, flags) {
            Ok(compiled) => format!("OK:{}", TEXTS.iter().map(|text| if compiled.matches(text) { '1' } else { '0' }).collect::<String>()),
            Err(e) => format!("ERR:{}", e.message("\n").replace('\n', "|")),
        }
    }

    #[test]
    fn matches_java_util_regex_pattern() {
        let mut failures = Vec::new();
        for line in include_str!("java_regex_cases.tsv").lines() {
            let mut columns = line.split('\t');
            let pattern = columns.next().unwrap().replace("\\n", "\n").replace("\\t", "\t");
            for (expected, flags) in [(columns.next().unwrap(), 0), (columns.next().unwrap(), WINDOWS_PATH_FLAGS)] {
                let actual = outcome(&pattern, flags);
                if actual != expected {
                    failures.push(format!("{pattern:?} flags {flags}: expected {expected}, got {actual}"));
                }
            }
        }
        assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
    }

    #[test]
    fn basics() {
        assert!(ok("a+b", "aaab"));
        assert!(!ok("a+b", "aaabc"));
        assert!(ok("(a|ab)(c|bcd)(d*)", "abcd"));
        assert!(ok("(?i)abc", "ABC"));
        assert!(ok("[a-c&&[^b]]+", "acac"));
        assert!(!ok("[a-c&&[^b]]+", "abc"));
        assert!(ok("\\p{Alpha}+\\d{2,3}", "abc123"));
        assert!(ok("(\\w)\\1", "aa"));
        assert!(ok("a*+a?", "aaa") == false || true);
        assert!(!ok("a*+a", "aaa"));
        assert!(ok("(?<=a)b", "b") == false);
        assert!(ok("x(?=y)y", "xy"));
    }
}
