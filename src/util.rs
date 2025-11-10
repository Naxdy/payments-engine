use rust_decimal::Decimal;
use serde::{Deserializer, Serializer, de::Visitor};

pub fn parse_decimal<'de, D>(d: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    struct DecimalVisitor;

    impl Visitor<'_> for DecimalVisitor {
        type Value = Decimal;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a decimal number")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let mut d = Decimal::from_str_exact(v)
                .map_err(|e| serde::de::Error::custom(format!("failed to parse decimal: {e}")))?;

            d.set_scale(4).map_err(|e| {
                serde::de::Error::custom(format!("failed to set decimal scale: {e}"))
            })?;

            Ok(d)
        }
    }

    d.deserialize_str(DecimalVisitor)
}

pub fn serialize_decimal<S>(value: &Decimal, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&value.to_string())
}
