//! Exact decimal arithmetic for money, quantities and rates (no floats).

use std::fmt;

/// Number of decimal places kept internally.
const SCALE: i128 = 10_000;

/// A decimal number with four fractional digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Dec(i128);

impl Dec {
    pub const ZERO: Dec = Dec(0);

    /// Parses `"1400"`, `"1400.5"`, `"1.400,50"`, `"1400,50"` or `"-3"`.
    pub fn parse(text: &str) -> Option<Dec> {
        let text = text.trim().replace(' ', "");
        if text.is_empty() {
            return None;
        }
        // German notation: dots group thousands, a comma marks the decimals.
        let normalized = if text.contains(',') {
            text.replace('.', "").replace(',', ".")
        } else {
            text
        };
        let (negative, digits) = match normalized.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, normalized.as_str()),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if whole.is_empty() && fraction.is_empty()
            || !whole.chars().all(|c| c.is_ascii_digit())
            || !fraction.chars().all(|c| c.is_ascii_digit())
            || fraction.len() > 4
        {
            return None;
        }
        let whole: i128 = if whole.is_empty() {
            0
        } else {
            whole.parse().ok()?
        };
        let fraction: i128 = format!("{fraction:0<4}").parse().ok()?;
        let value = whole * SCALE + fraction;
        Some(Dec(if negative { -value } else { value }))
    }

    /// Reads a JSON string or number.
    pub fn from_json(value: &serde_json::Value) -> Option<Dec> {
        match value {
            serde_json::Value::String(text) => Dec::parse(text),
            serde_json::Value::Number(number) => Dec::parse(&number.to_string()),
            _ => None,
        }
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Product, rounded half away from zero to four digits.
    pub fn mul(self, other: Dec) -> Dec {
        Dec(div_round(self.0 * other.0, SCALE))
    }

    /// Percentage of `self`: `self * rate / 100`.
    pub fn percent(self, rate: Dec) -> Dec {
        Dec(div_round(self.0 * rate.0, SCALE * 100))
    }

    /// Rounded half away from zero to two decimal places (cents).
    pub fn round2(self) -> Dec {
        Dec(div_round(self.0, 100) * 100)
    }

    /// Two decimals, as EN 16931 expects for amounts.
    pub fn amount(self) -> String {
        let cents = self.round2().0 / 100;
        let sign = if cents < 0 { "-" } else { "" };
        format!("{sign}{}.{:02}", cents.abs() / 100, cents.abs() % 100)
    }
}

impl std::ops::Add for Dec {
    type Output = Dec;
    fn add(self, other: Dec) -> Dec {
        Dec(self.0 + other.0)
    }
}

impl std::ops::Sub for Dec {
    type Output = Dec;
    fn sub(self, other: Dec) -> Dec {
        Dec(self.0 - other.0)
    }
}

impl std::iter::Sum for Dec {
    fn sum<I: Iterator<Item = Dec>>(iter: I) -> Dec {
        iter.fold(Dec::ZERO, |sum, value| sum + value)
    }
}

/// Without trailing zeros: `5`, `0.5`, `19`, `7.75`.
impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let whole = self.0.abs() / SCALE;
        let fraction = self.0.abs() % SCALE;
        if fraction == 0 {
            write!(f, "{sign}{whole}")
        } else {
            let fraction = format!("{fraction:04}");
            write!(f, "{sign}{whole}.{}", fraction.trim_end_matches('0'))
        }
    }
}

fn div_round(value: i128, divisor: i128) -> i128 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder.abs() * 2 >= divisor {
        quotient + value.signum()
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::Dec;

    #[test]
    fn parses_and_formats() {
        assert_eq!(Dec::parse("1400").unwrap().amount(), "1400.00");
        assert_eq!(Dec::parse("1.400,50").unwrap().amount(), "1400.50");
        assert_eq!(Dec::parse("0.5").unwrap().to_string(), "0.5");
        assert_eq!(Dec::parse("19").unwrap().to_string(), "19");
        assert!(Dec::parse("abc").is_none());
        assert!(Dec::parse("1.23456").is_none());
    }

    #[test]
    fn rounds_half_away_from_zero() {
        let basis = Dec::parse("10.05").unwrap();
        // 10.05 * 19 % = 1.9095 -> 1.91
        assert_eq!(basis.percent(Dec::parse("19").unwrap()).amount(), "1.91");
        assert_eq!(Dec::parse("0.005").unwrap().amount(), "0.01");
        assert_eq!(Dec::parse("-0.005").unwrap().amount(), "-0.01");
    }
}
