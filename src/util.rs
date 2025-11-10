use rust_decimal::{Decimal, prelude::FromPrimitive};
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

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let d = Decimal::from_f64(v)
                .ok_or_else(|| serde::de::Error::custom("failed to parse decimal"))?
                .round_dp(4);

            Ok(d)
        }
    }

    d.deserialize_any(DecimalVisitor)
}

pub fn serialize_decimal<S>(value: &Decimal, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&value.round_dp(4).to_string())
}
