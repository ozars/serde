use std::{marker::PhantomData, ops::Range};

use serde::{
    de::{
        value::{Error, StrDeserializer, U32Deserializer},
        ContextAccess, Deserialize, Deserializer, Error as _, Visitor,
    },
    forward_to_deserialize_any,
};

struct TrimDeserializer(String);

impl<'de> Deserializer<'de> for &'de mut TrimDeserializer {
    type Error = Error;

    forward_to_deserialize_any!(bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
                                string bytes byte_buf option unit unit_struct newtype_struct seq
                                tuple tuple_struct map struct enum identifier ignored_any);

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_context(visitor)
    }

    fn deserialize_context<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let start_index = self
            .0
            .char_indices()
            .find(|&(_, c)| !c.is_whitespace())
            .map(|(index, _)| index)
            .ok_or(Error::custom("no spaces found"))?;
        let end_index = self
            .0
            .char_indices()
            .rev()
            .find(|&(_, c)| !c.is_whitespace())
            .map(|(index, c)| index + c.len_utf8())
            .ok_or(Error::custom("no spaces found"))?;
        visitor.visit_context(ContextfulTrimAccess {
            de: StrDeserializer::new(self.0.get(start_index..end_index).unwrap()),
            span: start_index..end_index,
        })
    }
}

#[derive(Debug)]
struct ContextfulTrimAccess<'de> {
    de: StrDeserializer<'de, Error>,
    span: Range<usize>,
}

impl<'de> ContextAccess<'de> for ContextfulTrimAccess<'de> {
    type Error = Error;

    fn span(&mut self) -> Result<Range<usize>, Self::Error> {
        Ok(self.span.clone())
    }

    fn inner_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: Deserialize<'de>,
    {
        V::deserialize(self.de)
    }
}

#[derive(Debug)]
struct Spanned<T> {
    inner: T,
    span: Range<usize>,
}

impl<'de, T> Deserialize<'de> for Spanned<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SpannedVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for SpannedVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Spanned<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a spanned value")
            }

            fn visit_context<A>(self, mut context: A) -> Result<Self::Value, A::Error>
            where
                A: ContextAccess<'de>,
            {
                Ok(Spanned {
                    inner: context.inner_value()?,
                    span: context.span()?,
                })
            }
        }

        deserializer.deserialize_context(SpannedVisitor(PhantomData))
    }
}

#[test]
fn test_spanned() {
    let mut de = TrimDeserializer("   test  ".to_string());
    let spanned: Spanned<String> = Deserialize::deserialize(&mut de).unwrap();
    assert_eq!(spanned.inner, "test");
    assert_eq!(spanned.span, 3..7);
}

#[test]
fn test_unsupported_spanned() {
    let deserializer = U32Deserializer::<Error>::new(42);
    match Spanned::<u32>::deserialize(deserializer) {
        Ok(v) => panic!("unexpected value: {:?}", v),
        Err(e) => {
            assert_eq!(e, Error::custom("contextful values are not supported"));
        }
    }
}
