//! Number formatting shared by every report: percentages, thousands
//! separators, and the "general" format (5, 4.5, 1.23e+06) the tables use.

/// A probability as a percent with `places` decimals: 0.1234 -> "12.34%".
pub fn pct(x: f64, places: usize) -> String {
    format!("{:.places$}%", 100.0 * x)
}

/// A value that already is a percent (an amplification): 34 -> "34%" when it
/// is whole and no decimals are asked for, otherwise "31.15%".
pub fn amp_pct(value: f64, places: usize) -> String {
    if places == 0 && value == value.trunc() {
        format!("{}%", general(value))
    } else {
        format!("{value:.places$}%")
    }
}

/// A probability with three digits showing: 100%, 98.3%, 9.36%, 0.07%.
/// The decimals shrink as the number grows so every cell reads at the same
/// width; anything too small for two decimals is a plain 0%.
pub fn pct3(x: f64) -> String {
    let v = 100.0 * x;
    if v >= 100.0 {
        format!("{v:.0}%")
    } else if v >= 10.0 {
        format!("{v:.1}%")
    } else if v >= 0.005 {
        format!("{v:.2}%")
    } else {
        "0%".to_string()
    }
}

/// A rate modifier as a signed whole percent: 0.02 -> "+2%".
pub fn signed_pct(x: f64) -> String {
    let v = 100.0 * x;
    let body = format!("{:.0}", v.abs());
    let sign = if v.is_sign_negative() && body != "0" { '-' } else { '+' };
    format!("{sign}{body}%")
}

/// Thousands separators with `places` decimals: 1234567.8 -> "1,234,568".
pub fn commas(x: f64, places: usize) -> String {
    let text = format!("{:.places$}", x.abs());
    let (whole, frac) = match text.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (text.as_str(), None),
    };
    let mut out = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(frac) = frac {
        out.push('.');
        out.push_str(frac);
    }
    if x < 0.0 && out.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '-');
    }
    out
}

/// The shortest sensible form of a number: 5, 4.5, 0.0001, 1.23457e+06.
pub fn general(x: f64) -> String {
    general_digits(x, 6)
}

/// `general` with `digits` significant digits: 54.5, 1.23, 1e+03.
pub fn general_digits(x: f64, digits: usize) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    if !x.is_finite() {
        return x.to_string();
    }
    let digits = digits.max(1);
    let sci = format!("{:.*e}", digits - 1, x);
    let (mantissa, exp) = sci.split_once('e').expect("scientific format has an exponent");
    let exp: i32 = exp.parse().expect("exponent is an integer");
    if exp < -4 || exp >= digits as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim_zeros(mantissa), exp.abs())
    } else {
        let places = (digits as i32 - 1 - exp) as usize;
        trim_zeros(&format!("{x:.places$}")).to_string()
    }
}

fn trim_zeros(text: &str) -> &str {
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { text }
}

/// Diamonds, abbreviated: 54.5K, 1.23M, 81.6M.
pub fn human(value: f64) -> String {
    for (cut, suffix) in [(1e9, "B"), (1e6, "M"), (1e3, "K")] {
        if value >= cut {
            return format!("{}{suffix}", general_digits(value / cut, 3));
        }
    }
    format!("{value:.0}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages() {
        assert_eq!(pct(0.1234, 2), "12.34%");
        assert_eq!(amp_pct(34.0, 0), "34%");
        assert_eq!(amp_pct(31.154, 2), "31.15%");
        assert_eq!(signed_pct(0.02), "+2%");
        assert_eq!(signed_pct(-0.04), "-4%");
        assert_eq!(signed_pct(0.0), "+0%");
    }

    #[test]
    fn three_digits_showing() {
        assert_eq!(pct3(1.0), "100%");
        assert_eq!(pct3(0.9834), "98.3%");
        assert_eq!(pct3(0.09357), "9.36%");
        assert_eq!(pct3(0.0007), "0.07%");
        assert_eq!(pct3(0.0000004), "0%"); // rounds down rather than to 0.00%
    }

    #[test]
    fn thousands() {
        assert_eq!(commas(1_234_567.8, 0), "1,234,568");
        assert_eq!(commas(999.0, 0), "999");
        assert_eq!(commas(54_545.454_5, 1), "54,545.5");
        assert_eq!(commas(-1_000.0, 0), "-1,000");
    }

    #[test]
    fn general_format_matches_pythons() {
        assert_eq!(general(5.0), "5");
        assert_eq!(general(4.5), "4.5");
        assert_eq!(general(0.0001), "0.0001");
        assert_eq!(general(0.00001), "1e-05");
        assert_eq!(general(1_234_567.0), "1.23457e+06");
        assert_eq!(general_digits(54.54, 3), "54.5");
        assert_eq!(general_digits(120.4, 3), "120");
        assert_eq!(general_digits(999.96, 3), "1e+03");
    }

    #[test]
    fn diamonds() {
        assert_eq!(human(54_545.0), "54.5K");
        assert_eq!(human(1_234_567.0), "1.23M");
        assert_eq!(human(302.0), "302");
    }
}
