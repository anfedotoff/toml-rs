use std::error;
use std::fmt;

use Value;
use self::DecodeErrorKind::*;

#[cfg(feature = "rustc-serialize")] mod rustc_serialize;
#[cfg(feature = "serde")] mod serde;

/// A structure to transform TOML values into Rust values.
///
/// This decoder implements the serialization `Decoder` interface, allowing
/// `Decodable` types to be generated by this decoder. The input is any
/// arbitrary TOML value.
pub struct Decoder {
    /// The TOML value left over after decoding. This can be used to inspect
    /// whether fields were decoded or not.
    pub toml: Option<Value>,
    cur_field: Option<String>,
}

/// Description for errors which can occur while decoding a type.
#[derive(PartialEq, Debug)]
pub struct DecodeError {
    /// Field that this error applies to.
    pub field: Option<String>,
    /// The type of error which occurred while decoding,
    pub kind: DecodeErrorKind,
}

/// Enumeration of possible errors which can occur while decoding a structure.
#[derive(PartialEq, Debug)]
pub enum DecodeErrorKind {
    /// An error flagged by the application, e.g. value out of range
    ApplicationError(String),
    /// A field was expected, but none was found.
    ExpectedField(/* type */ Option<&'static str>),
    /// A field was found, but it had the wrong type.
    ExpectedType(/* expected */ &'static str, /* found */ &'static str),
    /// The nth map key was expected, but none was found.
    ExpectedMapKey(usize),
    /// The nth map element was expected, but none was found.
    ExpectedMapElement(usize),
    /// An enum decoding was requested, but no variants were supplied
    NoEnumVariants,
    /// The unit type was being decoded, but a non-zero length string was found
    NilTooLong,
    /// There was an error with the syntactical structure of the TOML.
    SyntaxError,
    /// The end of the TOML input was reached too soon
    EndOfStream,
}

/// Decodes a TOML value into a decodable type.
///
/// This function will consume the given TOML value and attempt to decode it
/// into the type specified. If decoding fails, `None` will be returned. If a
/// finer-grained error is desired, then it is recommended to use `Decodable`
/// directly.
#[cfg(feature = "rustc-serialize")]
pub fn decode<T: ::rustc_serialize::Decodable>(toml: Value) -> Option<T> {
    ::rustc_serialize::Decodable::decode(&mut Decoder::new(toml)).ok()
}

/// Decodes a TOML value into a decodable type.
///
/// This function will consume the given TOML value and attempt to decode it
/// into the type specified. If decoding fails, `None` will be returned. If a
/// finer-grained error is desired, then it is recommended to use `Decodable`
/// directly.
#[cfg(all(not(feature = "rustc-serialize"), feature = "serde"))]
pub fn decode<T: ::serde::Deserialize>(toml: Value) -> Option<T> {
    ::serde::Deserialize::deserialize(&mut Decoder::new(toml)).ok()
}

/// Decodes a string into a toml-encoded value.
///
/// This function will parse the given string into a TOML value, and then parse
/// the TOML value into the desired type. If any error occurs `None` is return.
///
/// If more fine-grained errors are desired, these steps should be driven
/// manually.
#[cfg(feature = "rustc-serialize")]
pub fn decode_str<T: ::rustc_serialize::Decodable>(s: &str) -> Option<T> {
    ::Parser::new(s).parse().and_then(|t| decode(Value::Table(t)))
}

/// Decodes a string into a toml-encoded value.
///
/// This function will parse the given string into a TOML value, and then parse
/// the TOML value into the desired type. If any error occurs `None` is return.
///
/// If more fine-grained errors are desired, these steps should be driven
/// manually.
#[cfg(all(not(feature = "rustc-serialize"), feature = "serde"))]
pub fn decode_str<T: ::serde::Deserialize>(s: &str) -> Option<T> {
    ::Parser::new(s).parse().and_then(|t| decode(Value::Table(t)))
}

impl Decoder {
    /// Creates a new decoder, consuming the TOML value to decode.
    ///
    /// This decoder can be passed to the `Decodable` methods or driven
    /// manually.
    pub fn new(toml: Value) -> Decoder {
        Decoder { toml: Some(toml), cur_field: None }
    }

    fn sub_decoder(&self, toml: Option<Value>, field: &str) -> Decoder {
        Decoder {
            toml: toml,
            cur_field: if field.len() == 0 {
                self.cur_field.clone()
            } else {
                match self.cur_field {
                    None => Some(format!("{}", field)),
                    Some(ref s) => Some(format!("{}.{}", s, field))
                }
            }
        }
    }

    fn err(&self, kind: DecodeErrorKind) -> DecodeError {
        DecodeError {
            field: self.cur_field.clone(),
            kind: kind,
        }
    }

    fn mismatch(&self, expected: &'static str,
                found: &Option<Value>) -> DecodeError{
        match *found {
            Some(ref val) => self.err(ExpectedType(expected, val.type_str())),
            None => self.err(ExpectedField(Some(expected))),
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(match self.kind {
            ApplicationError(ref err) => {
                write!(f, "{}", err)
            }
            ExpectedField(expected_type) => {
                match expected_type {
                    Some("table") => write!(f, "expected a section"),
                    Some(e) => write!(f, "expected a value of type `{}`", e),
                    None => write!(f, "expected a value"),
                }
            }
            ExpectedType(expected, found) => {
                fn humanize(s: &str) -> String {
                    if s == "section" {
                        format!("a section")
                    } else {
                        format!("a value of type `{}`", s)
                    }
                }
                write!(f, "expected {}, but found {}",
                       humanize(expected),
                       humanize(found))
            }
            ExpectedMapKey(idx) => {
                write!(f, "expected at least {} keys", idx + 1)
            }
            ExpectedMapElement(idx) => {
                write!(f, "expected at least {} elements", idx + 1)
            }
            NoEnumVariants => {
                write!(f, "expected an enum variant to decode to")
            }
            NilTooLong => {
                write!(f, "expected 0-length string")
            }
            SyntaxError => {
                write!(f, "syntax error")
            }
            EndOfStream => {
                write!(f, "end of stream")
            }
        });
        match self.field {
            Some(ref s) => {
                write!(f, " for the key `{}`", s)
            }
            None => Ok(())
        }
    }
}

impl error::Error for DecodeError {
    fn description(&self) -> &str {
        match self.kind {
            ApplicationError(ref s) => &**s,
            ExpectedField(..) => "expected a field",
            ExpectedType(..) => "expected a type",
            ExpectedMapKey(..) => "expected a map key",
            ExpectedMapElement(..) => "expected a map element",
            NoEnumVariants => "no enum variants to decode to",
            NilTooLong => "nonzero length string representing nil",
            SyntaxError => "syntax error",
            EndOfStream => "end of stream",
        }
    }
}
