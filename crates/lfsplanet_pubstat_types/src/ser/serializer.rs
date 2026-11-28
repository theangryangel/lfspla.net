//! Serde trait implementations and validation of the supported row structure.
use super::{Error, text::Text};
use serde::{
    Serialize,
    ser::{self, Impossible, SerializeSeq, SerializeStruct},
};
use std::{fmt, io::Write};

fn unsupported() -> Error {
    Error::Message("Pubstat text expects a sequence of structs with scalar fields".into())
}

pub(super) struct Serializer<W> {
    text: Text<W>,
    depth: u8,
}
impl<W: Write> Serializer<W> {
    pub(super) fn new(writer: W) -> Self {
        Self {
            text: Text::new(writer),
            depth: 0,
        }
    }

    fn scalar(&mut self, value: impl fmt::Display) -> Result<(), Error> {
        if self.depth != 2 {
            return Err(unsupported());
        }
        self.text.scalar(value)
    }
}
impl<W: Write> ser::Serializer for &mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<(), Error>;
    fn serialize_bool(self, value: bool) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_i8(self, value: i8) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_i16(self, value: i16) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_i32(self, value: i32) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_i64(self, value: i64) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_i128(self, value: i128) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_u8(self, value: u8) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_u16(self, value: u16) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_u32(self, value: u32) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_u64(self, value: u64) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_u128(self, value: u128) -> Result<(), Error> {
        self.scalar(value)
    }
    fn serialize_str(self, value: &str) -> Result<(), Error> {
        if self.depth != 2 {
            return Err(unsupported());
        }
        self.text.string(value)
    }
    fn serialize_char(self, value: char) -> Result<(), Error> {
        self.serialize_str(value.encode_utf8(&mut [0; 4]))
    }
    fn serialize_f32(self, _: f32) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_f64(self, _: f64) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_none(self) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Error> {
        Err(unsupported())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Error> {
        if self.depth != 0 {
            return Err(unsupported());
        }
        self.depth = 1;
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> {
        Err(unsupported())
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(unsupported())
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(unsupported())
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(unsupported())
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        if self.depth != 1 {
            return Err(unsupported());
        }
        self.depth = 2;
        self.text.start_row();
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(unsupported())
    }
}
impl<W: Write> SerializeSeq for &mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if self.depth != 1 {
            return Err(unsupported());
        }
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        self.depth = 0;
        Ok(())
    }
}
impl<W: Write> SerializeStruct for &mut Serializer<W> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        self.text.end_row()?;
        self.depth = 1;
        Ok(())
    }
}
