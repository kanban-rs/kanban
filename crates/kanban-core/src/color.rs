use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid colour '{0}': expected #RRGGBB")]
pub struct ParseHexColorError(pub String);

impl FromStr for HexColor {
    type Err = ParseHexColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseHexColorError(s.to_string());
        let digits = s.strip_prefix('#').ok_or_else(err)?;
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| err());
        Ok(Self {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
        })
    }
}

impl fmt::Display for HexColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_full_form_returns_channels() {
        let color: HexColor = "#2E3440".parse().unwrap();
        assert_eq!(
            color,
            HexColor {
                r: 46,
                g: 52,
                b: 64
            }
        );
    }

    #[test]
    fn test_parse_hex_color_lowercase_digits_are_accepted() {
        let lower: HexColor = "#2e3440".parse().unwrap();
        let upper: HexColor = "#2E3440".parse().unwrap();
        assert_eq!(lower, upper);
    }

    #[test]
    fn test_parse_hex_color_short_form_is_rejected() {
        assert!("#2E3".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_without_hash_is_rejected() {
        assert!("2E3440".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_non_hex_digits_are_rejected() {
        assert!("#GGGGGG".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_named_colour_is_rejected() {
        assert!("blue".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_sign_character_is_rejected() {
        assert!("#+F0000".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_surrounding_whitespace_is_rejected() {
        assert!(" #2E3440".parse::<HexColor>().is_err());
        assert!("#2E3440 ".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_multibyte_input_is_rejected_without_panic() {
        assert!("#2é340".parse::<HexColor>().is_err());
    }

    #[test]
    fn test_parse_hex_color_error_names_the_input_and_expected_form() {
        let err = "#2E3".parse::<HexColor>().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("#2E3"));
        assert!(message.contains("#RRGGBB"));
    }

    #[test]
    fn test_display_hex_color_is_uppercase_canonical_form() -> Result<(), Box<dyn std::error::Error>>
    {
        let color: HexColor = "#2e3440".parse()?;
        assert_eq!(color.to_string(), "#2E3440");
        Ok(())
    }
}
