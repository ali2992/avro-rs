//! Logic for serde-compatible serialization.
use std::collections::HashMap;
use std::iter::once;

use serde::{
    ser::{self, Error as SerdeError},
    Serialize,
};
use std::rc::Rc;

use crate::types::{ToAvro, Value};
use crate::error::Error;

#[derive(Clone, Default)]
pub struct Serializer {}

pub struct SeqSerializer {
    items: Vec<Value>,
}

pub struct MapSerializer {
    indices: HashMap<String, usize>,
    values: Vec<Value>,
}

pub struct StructSerializer {
    fields: Vec<(String, Value)>,
}

pub struct VariantStructSerializer {
    variant_index: usize,
    fields: Vec<(String, Value)>,
}

impl SeqSerializer {
    pub fn new(len: Option<usize>) -> SeqSerializer {
        let items = match len {
            Some(len) => Vec::with_capacity(len),
            None => Vec::new(),
        };

        SeqSerializer { items }
    }
}

impl MapSerializer {
    pub fn new(len: Option<usize>) -> MapSerializer {
        let (indices, values) = match len {
            Some(len) => (HashMap::with_capacity(len), Vec::with_capacity(len)),
            None => (HashMap::new(), Vec::new()),
        };

        MapSerializer { indices, values }
    }
}

impl StructSerializer {
    pub fn new(len: usize) -> StructSerializer {
        StructSerializer {
            fields: Vec::with_capacity(len),
        }
    }
}

impl VariantStructSerializer {
    pub fn new(variant_index: usize, len: usize) -> VariantStructSerializer {
        VariantStructSerializer {
            variant_index,
            fields: Vec::with_capacity(len),
        }
    }
}

pub struct VariantSeqSerializer {
    variant_index: usize,
    items: Vec<Value>,
}
impl VariantSeqSerializer {
    pub fn new(variant_index: usize, len: usize) -> VariantSeqSerializer {
        VariantSeqSerializer {
            variant_index,
            items: Vec::with_capacity(len),
        }
    }
}

impl<'b> ser::Serializer for &'b mut Serializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = VariantSeqSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = VariantStructSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Boolean(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Long(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        if v <= i32::max_value() as u32 {
            self.serialize_i32(v as i32)
        } else {
            self.serialize_i64(i64::from(v))
        }
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        if v <= i64::max_value() as u64 {
            self.serialize_i64(v as i64)
        } else {
            Err(Error::custom("u64 is too large"))
        }
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Double(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&once(v).collect::<String>())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bytes(v.to_owned()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(ToAvro::avro(None::<Self::Ok>))
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let v = value.serialize(&mut Serializer::default())?;
        Ok(ToAvro::avro(Some(v)))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Union(index as usize, Box::new(Value::Null)))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let variant_value = self.serialize_newtype_struct(name, value)?;
        Ok(Value::Union(index as usize, Box::new(variant_value)))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SeqSerializer::new(len))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(VariantSeqSerializer::new(index as usize, len))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer::new(len))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructSerializer::new(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(VariantStructSerializer::new(index as usize, len))
    }
}

impl<'a> ser::SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.items
            .push(value.serialize(&mut Serializer::default())?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Array(self.items))
    }
}

impl<'a> ser::SerializeTuple for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for VariantSeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.items
            .push(value.serialize(&mut Serializer::default())?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let variant_value = Value::Array(self.items);
        Ok(Value::Union(self.variant_index, Box::new(variant_value)))
    }
}

impl ser::SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key = key.serialize(&mut Serializer::default())?;

        if let Value::String(key) = key {
            self.indices.insert(key, self.values.len());
            Ok(())
        } else {
            Err(Error::custom("map key is not a string"))
        }
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.values
            .push(value.serialize(&mut Serializer::default())?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut items = HashMap::new();
        for (key, index) in self.indices {
            if let Some(value) = self.values.get(index) {
                items.insert(key, value.clone());
            }
        }

        Ok(Value::Map(items))
    }
}

impl ser::SerializeStruct for StructSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.fields.push((
            name.to_owned(),
            value.serialize(&mut Serializer::default())?,
        ));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Record(self.fields.iter().map(|f| (Rc::new(f.0.clone()), f.1.clone())).collect()))
    }
}

impl ser::SerializeStructVariant for VariantStructSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.fields.push((
            name.to_owned(),
            value.serialize(&mut Serializer::default())?,
        ));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let variant_value = Value::Record(self.fields.iter().map(|f| (Rc::new(f.0.clone()), f.1.clone())).collect()); //TODO these clones are probably going to be expensive..
        Ok(Value::Union(self.variant_index, Box::new(variant_value)))
    }
}

/// Interpret a serializeable instance as a `Value`.
///
/// This conversion can fail if the value is not valid as per the Avro specification.
/// e.g: HashMap with non-string keys
pub fn to_value<S: Serialize>(value: S) -> Result<Value, Error> {
    let mut serializer = Serializer::default();
    value.serialize(&mut serializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    struct Test {
        a: i64,
        b: String,
    }

    #[test]
    fn test_to_value() {
        let test = Test {
            a: 27,
            b: "foo".to_owned(),
        };
        let expected = Value::Record(vec![
            (Rc::new("a".to_owned()), Value::Long(27)),
            (Rc::new("b".to_owned()), Value::String("foo".to_owned())),
        ]);

        assert_eq!(to_value(test).unwrap(), expected);
    }
}
