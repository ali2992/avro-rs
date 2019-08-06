//! Logic handling the intermediate representation of Avro values.
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::u8;

use failure::{Error, Fail};
use serde_json::Value as JsonValue;
use crate::schema::{RecordField, Schema, UnionSchema};
use crate::schema_resolution;

/// Represents any valid Avro value
/// More information about Avro values can be found in the
/// [Avro Specification](https://avro.apache.org/docs/current/spec.html#schemas)
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A `null` Avro value.
    Null,
    /// A `boolean` Avro value.
    Boolean(bool),
    /// A `int` Avro value.
    Int(i32),
    /// A `long` Avro value.
    Long(i64),
    /// A `float` Avro value.
    Float(f32),
    /// A `double` Avro value.
    Double(f64),
    /// A `bytes` Avro value.
    Bytes(Vec<u8>),
    /// A `string` Avro value.
    String(String),
    /// A `fixed` Avro value.
    /// The size of the fixed value is represented as a `usize`.
    Fixed(usize, Vec<u8>),
    /// An `enum` Avro value.
    ///
    /// An Enum is represented by a symbol and its position in the symbols list
    /// of its corresponding schema.
    /// This allows schema-less encoding, as well as schema resolution while
    /// reading values.
    Enum(i32 /* XXX: should it not be an `usize` */, String),
    /// An `union` Avro value.
    Union(usize, Box<Value>),
    /// An `array` Avro value.
    Array(Vec<Value>),
    /// A `map` Avro value.
    Map(HashMap<String, Value>),
    /// A `record` Avro value.
    ///
    /// A Record is represented by a vector of (`<record name>`, `value`).
    /// This allows schema-less encoding.
    ///
    /// See [Record](types.Record) for a more user-friendly support.
    Record(Vec<(String, Value)>),
}

/// Any structure implementing the [ToAvro](trait.ToAvro.html) trait will be usable
/// from a [Writer](../writer/struct.Writer.html).
pub trait ToAvro {
    /// Transforms this value into an Avro-compatible [Value](enum.Value.html).
    fn avro(self) -> Value;
}

macro_rules! to_avro(
    ($t:ty, $v:expr) => (
        impl ToAvro for $t {
            fn avro(self) -> Value {
                $v(self)
            }
        }
    );
);

to_avro!(bool, Value::Boolean);
to_avro!(i32, Value::Int);
to_avro!(i64, Value::Long);
to_avro!(f32, Value::Float);
to_avro!(f64, Value::Double);
to_avro!(String, Value::String);

impl ToAvro for () {
    fn avro(self) -> Value {
        Value::Null
    }
}

impl ToAvro for usize {
    fn avro(self) -> Value {
        (self as i64).avro()
    }
}

impl<'a> ToAvro for &'a str {
    fn avro(self) -> Value {
        Value::String(self.to_owned())
    }
}

impl<'a> ToAvro for &'a [u8] {
    fn avro(self) -> Value {
        Value::Bytes(self.to_owned())
    }
}

impl<T> ToAvro for Option<T>
where
    T: ToAvro,
{
    fn avro(self) -> Value {
        let (i, v) = match self {
            Some(v) => (1, T::avro(v)),
            None => (0, Value::Null),
        };
        /*
         * https://avro.apache.org/docs/current/spec.html#Unions
         * | Thus, for unions containing "null", the "null" is usually listed first, since the default value of such unions is typically null
         *
         * Although that is not a guarantee (that the schema will be defined as ["null", {"type": ...}]: null-first),
         *  this can be used as a guideline to choose variant-index=0 for the None value.
         */
        Value::Union(i, Box::new(v))
    }
}

impl<T, S: BuildHasher> ToAvro for HashMap<String, T, S>
where
    T: ToAvro,
{
    fn avro(self) -> Value {
        Value::Map(
            self.into_iter()
                .map(|(key, value)| (key, value.avro()))
                .collect::<_>(),
        )
    }
}

impl<'a, T, S: BuildHasher> ToAvro for HashMap<&'a str, T, S>
where
    T: ToAvro,
{
    fn avro(self) -> Value {
        Value::Map(
            self.into_iter()
                .map(|(key, value)| (key.to_owned(), value.avro()))
                .collect::<_>(),
        )
    }
}

impl ToAvro for Value {
    fn avro(self) -> Value {
        self
    }
}

/*
impl<S: Serialize> ToAvro for S {
    fn avro(self) -> Value {
        use ser::Serializer;

        self.serialize(&mut Serializer::new()).unwrap()
    }
}
*/

/// Utility interface to build `Value::Record` objects.
#[derive(Debug, Clone)]
pub struct Record<'a> {
    /// List of fields contained in the record.
    /// Ordered according to the fields in the schema given to create this
    /// `Record` object. Any unset field defaults to `Value::Null`.
    pub fields: Vec<(String, Value)>,
    schema_lookup: &'a HashMap<String, usize>,
}

impl<'a> Record<'a> {
    /// Create a `Record` given a `Schema`.
    ///
    /// If the `Schema` is not a `Schema::Record` variant, `None` will be returned.
    pub fn new(schema: &Schema) -> Option<Record> {
        match *schema {
            Schema::Record {
                fields: ref schema_fields,
                lookup: ref schema_lookup,
                ..
            } => {
                let mut fields = Vec::with_capacity(schema_fields.len());
                for schema_field in schema_fields.iter() {
                    fields.push((schema_field.name.clone(), Value::Null));
                }

                Some(Record {
                    fields,
                    schema_lookup,
                })
            }
            _ => None,
        }
    }

    /// Put a compatible value (implementing the `ToAvro` trait) in the
    /// `Record` for a given `field` name.
    ///
    /// **NOTE** Only ensure that the field name is present in the `Schema` given when creating
    /// this `Record`. Does not perform any schema validation.
    pub fn put<V>(&mut self, field: &str, value: V)
    where
        V: ToAvro,
    {
        if let Some(&position) = self.schema_lookup.get(field) {
            self.fields[position].1 = value.avro()
        }
    }
}

impl<'a> ToAvro for Record<'a> {
    fn avro(self) -> Value {
        Value::Record(self.fields)
    }
}

impl ToAvro for JsonValue {
    fn avro(self) -> Value {
        match self {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(b) => Value::Boolean(b),
            JsonValue::Number(ref n) if n.is_i64() => Value::Long(n.as_i64().unwrap()),
            JsonValue::Number(ref n) if n.is_f64() => Value::Double(n.as_f64().unwrap()),
            JsonValue::Number(n) => Value::Long(n.as_u64().unwrap() as i64), // TODO: Not so great
            JsonValue::String(s) => Value::String(s),
            JsonValue::Array(items) => {
                Value::Array(items.into_iter().map(|item| item.avro()).collect::<_>())
            }
            JsonValue::Object(items) => Value::Map(
                items
                    .into_iter()
                    .map(|(key, value)| (key, value.avro()))
                    .collect::<_>(),
            ),
        }
    }
}

impl Value {
    /// Validate the value against the given [Schema](../schema/enum.Schema.html).
    ///
    /// See the [Avro specification](https://avro.apache.org/docs/current/spec.html)
    /// for the full set of rules of schema validation.
    pub fn validate(&self, schema: &Schema) -> bool {
        match (self, schema) {
            (&Value::Null, &Schema::Null) => true,
            (&Value::Boolean(_), &Schema::Boolean) => true,
            (&Value::Int(_), &Schema::Int) => true,
            (&Value::Long(_), &Schema::Long) => true,
            (&Value::Float(_), &Schema::Float) => true,
            (&Value::Double(_), &Schema::Double) => true,
            (&Value::Bytes(_), &Schema::Bytes) => true,
            (&Value::String(_), &Schema::String) => true,
            (&Value::Fixed(n, _), &Schema::Fixed { size, .. }) => n == size,
            (&Value::String(ref s), &Schema::Enum { ref symbols, .. }) => symbols.contains(s),
            (&Value::Enum(i, ref s), &Schema::Enum { ref symbols, .. }) => symbols
                .get(i as usize)
                .map(|ref symbol| symbol == &s)
                .unwrap_or(false),
            // (&Value::Union(None), &Schema::Union(_)) => true,
            (&Value::Union(variant_idx, ref variant_value), &Schema::Union(ref inner)) => {
                match inner.variant_schema(variant_idx) {
                    None =>
                    // Is an invalid variant-index a schema validation error or a failure worth panicking?
                    {
                        false
                    }
                    Some(variant_schema) => variant_value.validate(variant_schema),
                }
            }
            (&Value::Array(ref items), &Schema::Array(ref inner)) => {
                items.iter().all(|item| item.validate(inner))
            }
            (&Value::Map(ref items), &Schema::Map(ref inner)) => {
                items.iter().all(|(_, value)| value.validate(inner))
            }
            (&Value::Record(ref record_fields), &Schema::Record { ref fields, .. }) => {
                fields.len() == record_fields.len()
                    && fields.iter().zip(record_fields.iter()).all(
                        |(field, &(ref name, ref value))| {
                            field.name == *name && value.validate(&field.schema)
                        },
                    )
            }
            _ => false,
        }
    }

    /// Attempt to perform schema resolution on the value, with the given
    /// [Schema](../schema/enum.Schema.html).
    ///
    /// See [Schema Resolution](https://avro.apache.org/docs/current/spec.html#Schema+Resolution)
    /// in the Avro specification for the full set of rules of schema
    /// resolution.
    pub(crate) fn resolve(
        self,
        writer_schema: &Schema,
        reader_schema: &Schema,
    ) -> Result<Self, Error> {
        let resolution = schema_resolution::Resolution::new(writer_schema, reader_schema)?;
        let promoted_value = resolution.promote_value(self)?;

        Ok(promoted_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Name, RecordField, RecordFieldOrder, UnionSchema};

    #[test]
    fn validate() {
        let value_schema_valid = vec![
            (Value::Int(42), Schema::Int, true),
            (Value::Int(42), Schema::Boolean, false),
            (
                Value::Union(0, Box::new(Value::Null)),
                Schema::Union(UnionSchema::new(vec![Schema::Null, Schema::Int]).unwrap()),
                true,
            ),
            (
                Value::Union(1, Box::new(Value::Int(42))),
                Schema::Union(UnionSchema::new(vec![Schema::Null, Schema::Int]).unwrap()),
                true,
            ),
            (
                Value::Union(0, Box::new(Value::Null)),
                Schema::Union(UnionSchema::new(vec![Schema::Double, Schema::Int]).unwrap()),
                false,
            ),
            (
                Value::Union(3, Box::new(Value::Int(42))),
                Schema::Union(
                    UnionSchema::new(vec![
                        Schema::Null,
                        Schema::Double,
                        Schema::String,
                        Schema::Int,
                    ])
                    .unwrap(),
                ),
                true,
            ),
            (
                Value::Array(vec![Value::Long(42i64)]),
                Schema::Array(Box::new(Schema::Long)),
                true,
            ),
            (
                Value::Array(vec![Value::Boolean(true)]),
                Schema::Array(Box::new(Schema::Long)),
                false,
            ),
            (Value::Record(vec![]), Schema::Null, false),
        ];

        for (value, schema, valid) in value_schema_valid.into_iter() {
            assert_eq!(valid, value.validate(&schema));
        }
    }

    #[test]
    fn validate_fixed() {
        let schema = Schema::Fixed {
            size: 4,
            name: Name::new("some_fixed"),
        };

        assert!(Value::Fixed(4, vec![0, 0, 0, 0]).validate(&schema));
        assert!(!Value::Fixed(5, vec![0, 0, 0, 0, 0]).validate(&schema));
    }

    #[test]
    fn validate_enum() {
        let schema = Schema::Enum {
            name: Name::new("some_enum"),
            doc: None,
            symbols: vec![
                "spades".to_string(),
                "hearts".to_string(),
                "diamonds".to_string(),
                "clubs".to_string(),
            ],
        };

        assert!(Value::Enum(0, "spades".to_string()).validate(&schema));
        assert!(Value::String("spades".to_string()).validate(&schema));

        assert!(!Value::Enum(1, "spades".to_string()).validate(&schema));
        assert!(!Value::String("lorem".to_string()).validate(&schema));

        let other_schema = Schema::Enum {
            name: Name::new("some_other_enum"),
            doc: None,
            symbols: vec![
                "hearts".to_string(),
                "diamonds".to_string(),
                "clubs".to_string(),
                "spades".to_string(),
            ],
        };

        assert!(!Value::Enum(0, "spades".to_string()).validate(&other_schema));
    }

    #[test]
    fn validate_record() {
        // {
        //    "type": "record",
        //    "fields": [
        //      {"type": "long", "name": "a"},
        //      {"type": "string", "name": "b"}
        //    ]
        // }
        let schema = Schema::Record {
            name: Name::new("some_record"),
            doc: None,
            fields: vec![
                RecordField {
                    name: "a".to_string(),
                    doc: None,
                    default: None,
                    schema: Schema::Long,
                    order: RecordFieldOrder::Ascending,
                    position: 0,
                },
                RecordField {
                    name: "b".to_string(),
                    doc: None,
                    default: None,
                    schema: Schema::String,
                    order: RecordFieldOrder::Ascending,
                    position: 1,
                },
            ],
            lookup: HashMap::new(),
        };

        assert!(Value::Record(vec![
            ("a".to_string(), Value::Long(42i64)),
            ("b".to_string(), Value::String("foo".to_string())),
        ])
        .validate(&schema));

        assert!(!Value::Record(vec![
            ("b".to_string(), Value::String("foo".to_string())),
            ("a".to_string(), Value::Long(42i64)),
        ])
        .validate(&schema));

        assert!(!Value::Record(vec![
            ("a".to_string(), Value::Boolean(false)),
            ("b".to_string(), Value::String("foo".to_string())),
        ])
        .validate(&schema));

        assert!(!Value::Record(vec![
            ("a".to_string(), Value::Long(42i64)),
            ("c".to_string(), Value::String("foo".to_string())),
        ])
        .validate(&schema));

        assert!(!Value::Record(vec![
            ("a".to_string(), Value::Long(42i64)),
            ("b".to_string(), Value::String("foo".to_string())),
            ("c".to_string(), Value::Null),
        ])
        .validate(&schema));
    }

    #[test]
    fn resolve_bytes_to_bytes_invalid_data() {
        let value = Value::Array(vec![Value::Int(2000), Value::Int(-42)]);
        // Although the value has little to do with Bytes,
        //   the resolution implies Identity-transformation;
        // Hence it's a schema-validation's business to address this invalid input
        assert!(value.resolve(&Schema::Bytes, &Schema::Bytes).is_ok());
    }
}
