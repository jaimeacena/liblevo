//! Fórmulas sencillas al mostrar y exportar.
//!
//! El modelo visual escribe las fórmulas en LaTeX (`$U_{2}O_{3}$`). Al convertir el Markdown en
//! HTML o XHTML, las sencillas (letras y cifras con subíndices, superíndices, `\mathrm` y unos
//! pocos símbolos) se escriben con `<sub>` y `<sup>`. Cualquier otra cosa se deja tal cual, con sus
//! `$`: nunca se inventa una fórmula. El Markdown guardado no cambia.

/// HTML de una fórmula sencilla, o `None` si tiene algo que no se sabe escribir.
pub fn simple_math_html(tex: &str) -> Option<String> {
    let chars: Vec<char> = tex.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    write_group(&chars, &mut i, &mut out, false)?;
    (i == chars.len() && !out.trim().is_empty()).then_some(out)
}

fn escape(c: char, out: &mut String) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        other => out.push(other),
    }
}

/// Escribe hasta el final o hasta la llave que cierra el grupo (si `inside`).
fn write_group(chars: &[char], i: &mut usize, out: &mut String, inside: bool) -> Option<()> {
    while *i < chars.len() {
        let c = chars[*i];
        match c {
            '}' if inside => return Some(()),
            '}' => return None,
            '{' => {
                *i += 1;
                write_group(chars, i, out, true)?;
                if chars.get(*i) != Some(&'}') {
                    return None;
                }
                *i += 1;
            }
            '_' | '^' => {
                *i += 1;
                let tag = if c == '_' { "sub" } else { "sup" };
                out.push_str(&format!("<{tag}>"));
                write_argument(chars, i, out)?;
                out.push_str(&format!("</{tag}>"));
            }
            '\\' => {
                *i += 1;
                let start = *i;
                while *i < chars.len() && chars[*i].is_ascii_alphabetic() {
                    *i += 1;
                }
                let name: String = chars[start..*i].iter().collect();
                match name.as_str() {
                    "mathrm" | "text" | "mathit" | "textrm" | "rm" | "mathbf" => {
                        // El estilo no se representa: solo su contenido.
                        if chars.get(*i) == Some(&'{') {
                            write_argument(chars, i, out)?;
                        }
                    }
                    "cdot" => out.push('·'),
                    "times" => out.push('×'),
                    "pm" => out.push('±'),
                    "circ" => out.push('°'),
                    "prime" => out.push('′'),
                    "ldots" | "dots" => out.push('…'),
                    "," | "" if chars.get(*i).is_some_and(|c| *c == ',' || *c == ' ') => {
                        *i += 1;
                        out.push(' ');
                    }
                    _ => return None,
                }
            }
            c if c.is_alphanumeric() || " +-=()[].,;:/'·×−–".contains(c) => {
                escape(c, out);
                *i += 1;
            }
            _ => return None,
        }
    }
    (!inside).then_some(())
}

/// Argumento de `_`, `^` o de un comando: un grupo entre llaves o un solo carácter.
fn write_argument(chars: &[char], i: &mut usize, out: &mut String) -> Option<()> {
    match chars.get(*i) {
        Some('{') => {
            *i += 1;
            write_group(chars, i, out, true)?;
            if chars.get(*i) != Some(&'}') {
                return None;
            }
            *i += 1;
            Some(())
        }
        Some(c) if c.is_alphanumeric() || "+-".contains(*c) => {
            escape(*c, out);
            *i += 1;
            Some(())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_formulas_get_real_subscripts() {
        assert_eq!(
            simple_math_html("U_{2}O_{3}").as_deref(),
            Some("U<sub>2</sub>O<sub>3</sub>")
        );
        assert_eq!(
            simple_math_html(r"\mathrm{{Na}}\mathrm{{Al}}_{2}").as_deref(),
            Some("NaAl<sub>2</sub>")
        );
        assert_eq!(
            simple_math_html("Pb_{3}P_{2}^{2}O_{8}").as_deref(),
            Some("Pb<sub>3</sub>P<sub>2</sub><sup>2</sup>O<sub>8</sub>")
        );
        assert_eq!(
            simple_math_html(r"3 \cdot H_2O").as_deref(),
            Some("3 · H<sub>2</sub>O")
        );
    }

    #[test]
    fn anything_else_is_left_alone() {
        assert_eq!(simple_math_html(r"\frac{a}{b}"), None);
        assert_eq!(simple_math_html("a_{2"), None);
        assert_eq!(simple_math_html("a}"), None);
        assert_eq!(simple_math_html(""), None);
        assert_eq!(simple_math_html("x < y"), None);
    }
}
