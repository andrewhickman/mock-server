use std::collections::HashSet;
use std::fmt;
use std::iter::{once, FromIterator};
use std::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, Error, SeqAccess};

pub fn any() -> Box<dyn MethodFilter> {
    Box::new(|_: &http::Method| true)
}

#[derive(Debug)]
pub struct MethodSet {
    set: HashSet<http::Method>,
}

pub trait MethodFilter: Send + Sync {
    fn is_match(&self, method: &http::Method) -> bool;
}

impl<F> MethodFilter for F
where
    F: Fn(&http::Method) -> bool + Send + Sync,
{
    fn is_match(&self, method: &http::Method) -> bool {
        self(method)
    }
}

impl MethodFilter for MethodSet {
    fn is_match(&self, method: &http::Method) -> bool {
        self.set.contains(&method)
    }
}

impl<'de> Deserialize<'de> for MethodSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MethodSetVisitor;

        impl<'de> de::Visitor<'de> for MethodSetVisitor {
            type Value = MethodSet;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a set of HTTP methods")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_string(v.to_owned())
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let method = parse_http_method(v).map_err(E::custom)?;
                Ok(MethodSet {
                    set: HashSet::from_iter(once(method)),
                })
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut set = HashSet::with_capacity(seq.size_hint().unwrap_or(4));
                while let Some(v) = seq.next_element::<String>()? {
                    set.insert(parse_http_method(v).map_err(|err| A::Error::custom(err))?);
                }
                Ok(MethodSet { set })
            }
        }

        deserializer.deserialize_any(MethodSetVisitor)
    }
}

fn parse_http_method(mut string: String) -> Result<http::Method, http::method::InvalidMethod> {
    string.make_ascii_uppercase();
    http::Method::from_str(&string)
}
