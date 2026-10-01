//! Just enough KDL for zellij's keybinds: nodes, args, props, children,
//! comments (//, /* */, /-).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Node { pub name: String, pub args: Vec<String>, pub props: Vec<(String, String)>, pub children: Vec<Node> }

#[derive(Debug, PartialEq)]
enum Tok { Word(String), Str(String), Open, Close, End, Slashdash }

fn lex(text: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' | ';' => out.push(Tok::End),
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            c if c.is_whitespace() => {}
            '/' if it.peek() == Some(&'/') => { while it.peek().is_some_and(|&d| d != '\n') { it.next(); } }
            '/' if it.peek() == Some(&'*') => {
                it.next();
                let mut prev = ' ';
                loop {
                    match it.next() { None => return Err("unterminated comment".into()), Some('/') if prev == '*' => break, Some(d) => prev = d }
                }
            }
            '/' if it.peek() == Some(&'-') => { it.next(); out.push(Tok::Slashdash); }
            '"' => {
                let mut s = String::new();
                loop {
                    match it.next() {
                        None => return Err("unterminated string".into()),
                        Some('"') => break,
                        Some('\\') => match it.next() { Some('n') => s.push('\n'), Some('t') => s.push('\t'), Some(e) => s.push(e), None => {} },
                        Some(d) => s.push(d),
                    }
                }
                out.push(Tok::Str(s));
            }
            _ => {
                let mut w = c.to_string();
                while let Some(&d) = it.peek() {
                    if d.is_whitespace() || "{};\"".contains(d) { break; }
                    w.push(d);
                    it.next();
                }
                out.push(Tok::Word(w));
            }
        }
    }
    Ok(out)
}

fn nodes(toks: &[Tok], pos: &mut usize, depth: usize) -> Result<Vec<Node>, String> {
    let mut out = Vec::new();
    let mut drop_next = false;
    while *pos < toks.len() {
        match &toks[*pos] {
            Tok::End => { *pos += 1; }
            Tok::Close => {
                if depth == 0 { return Err("unbalanced brace".into()); }
                *pos += 1;
                return Ok(out);
            }
            Tok::Slashdash => { drop_next = true; *pos += 1; }
            Tok::Open => return Err("unbalanced brace".into()),
            Tok::Word(name) | Tok::Str(name) => {
                let mut node = Node { name: name.clone(), ..Default::default() };
                *pos += 1;
                while *pos < toks.len() {
                    match &toks[*pos] {
                        Tok::Word(w) if w.contains('=') => {
                            let (k, v) = w.split_once('=').unwrap();
                            node.props.push((k.into(), v.trim_matches('"').into()));
                            *pos += 1;
                        }
                        Tok::Word(w) | Tok::Str(w) => { node.args.push(w.clone()); *pos += 1; }
                        Tok::Slashdash => { *pos += 2; } // drop the next arg
                        Tok::Open => { *pos += 1; node.children = nodes(toks, pos, depth + 1)?; break; }
                        Tok::End | Tok::Close => break,
                    }
                }
                if !std::mem::take(&mut drop_next) { out.push(node); }
            }
        }
    }
    if depth > 0 { return Err("unbalanced brace".into()); }
    Ok(out)
}

pub fn parse(text: &str) -> Result<Vec<Node>, String> {
    let toks = lex(text)?;
    nodes(&toks, &mut 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_args_props_children_comments() {
        let n = parse("a 1 \"two\" k=v { b; c \"x\\\"y\" }\n// c\n/- d { e }\nf /* in */ g\n").unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].name, "a");
        assert_eq!(n[0].args, vec!["1", "two"]);
        assert_eq!(n[0].props, vec![("k".to_string(), "v".to_string())]);
        assert_eq!(n[0].children.len(), 2);
        assert_eq!(n[0].children[1].args, vec!["x\"y"]);
        assert_eq!((n[1].name.as_str(), n[1].args.clone()), ("f", vec!["g".to_string()]));
    }

    #[test]
    fn errors() {
        assert!(parse("a \"open").unwrap_err().contains("unterminated"));
        assert!(parse("a { b").unwrap_err().contains("brace"));
        assert!(parse("a }").unwrap_err().contains("brace"));
    }
}
